//! Private messages: `/v1/conversations/*`, as Laravel's
//! `DiscussionController` and `MessagePriveController`.
//!
//! A conversation is between two friends; only they see it (anyone else
//! gets a 404). Removing a friend keeps the conversation readable but closes
//! it to new messages. Bodies are sealed at rest with `MESSAGES_KEY`
//! (`crate::crypto`); without the key, every route here answers 503.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::members::member_id;
use super::{
    Thread, author, avatar_url, cut, data, forbidden, invalid, message_body, natural_cmp,
    not_found, path_id, timestamp, window,
};
use crate::auth::member::{Member, Verified};
use crate::config::MessagesKey;
use crate::crypto;
use crate::db::conversations::{self, Conversation};
use crate::db::friends;
use crate::db::rooms::Window;
use crate::error::ApiError;
use crate::extract::Json;
use crate::routes::auth::{limit, no_store};
use crate::state::AppState;

/// Private messages per minute per member (`throttle:60,1`).
const POST_LIMIT: u32 = 60;

#[derive(Debug, Serialize)]
pub struct Correspondent {
    pub pseudo: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Line {
    pub id: i64,
    pub with: Correspondent,
    /// The last message, in clear; `null` before the first one.
    pub last_message: Option<String>,
    pub last_message_from_me: bool,
    pub last_message_at: Option<String>,
    /// Messages received and not read yet.
    pub unread_count: i64,
}

#[derive(Debug, Serialize)]
pub struct Index {
    pub conversations: Vec<Line>,
    /// Friends one can start a conversation with, by pseudo.
    pub friends: Vec<Correspondent>,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub id: i64,
    pub body: String,
    /// The author's pseudo; `null` once the account is deleted.
    pub author: Option<String>,
    pub author_id: Option<String>,
    pub sent_at: Option<String>,
    /// The recipient opened it.
    pub read: bool,
}

#[derive(Debug, Serialize)]
pub struct Info {
    pub id: i64,
    pub with: Correspondent,
    /// Still friends: new messages are possible.
    pub is_friend: bool,
}

#[derive(Debug, Serialize)]
pub struct Opened {
    #[serde(flatten)]
    pub conversation: Info,
    pub messages: Vec<Message>,
    pub has_older: bool,
}

#[derive(Debug, Serialize)]
pub struct Started {
    pub id: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenBody {
    pseudo: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MessageBody {
    body: Option<String>,
}

fn key(state: &AppState) -> Result<&MessagesKey, ApiError> {
    state
        .config
        .messages_key
        .as_ref()
        .ok_or(ApiError::MessagesUnavailable)
}

/// Opens a stored body. One that does not open (a key that changed) is a
/// server fault, logged; the reader gets a 500 rather than a lie.
fn reveal(
    key: &MessagesKey,
    conversation_id: i64,
    sealed: &str,
    version: i16,
) -> Result<String, ApiError> {
    crypto::open(key, conversation_id, sealed, version).map_err(|e| {
        ApiError::Internal(format!(
            "private message of conversation {conversation_id}: {e}"
        ))
    })
}

fn message(
    key: &MessagesKey,
    conversation_id: i64,
    row: conversations::Message,
) -> Result<Message, ApiError> {
    let (author, author_id) = author(row.sender_id, row.sender_pseudo);
    Ok(Message {
        body: reveal(key, conversation_id, &row.body, row.key_version)?,
        id: row.id,
        author,
        author_id,
        sent_at: timestamp(row.created_at),
        read: row.read,
    })
}

/// `GET /v1/conversations`: the member's conversations, latest first, and
/// the friends they may write to.
pub async fn index(
    State(state): State<AppState>,
    Verified(me): Verified,
) -> Result<Response, ApiError> {
    let key = key(&state)?;
    let (rows, friends) = tokio::try_join!(
        conversations::of(&state.db, me.id()),
        conversations::friends(&state.db, me.id()),
    )?;
    let storage = &state.config.public_storage_url;
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let last_message = match (&row.last_body, row.last_key_version) {
            (Some(body), Some(version)) => Some(reveal(key, row.id, body, version)?),
            _ => None,
        };
        lines.push(Line {
            id: row.id,
            with: Correspondent {
                avatar_url: avatar_url(storage, row.avatar_path),
                pseudo: row.pseudo,
            },
            last_message_from_me: last_message.is_some() && row.last_sender_id == Some(me.id()),
            last_message,
            last_message_at: timestamp(row.last_at),
            unread_count: row.unread,
        });
    }
    let mut friends: Vec<Correspondent> = friends
        .into_iter()
        .map(|(pseudo, avatar)| Correspondent {
            avatar_url: avatar_url(storage, avatar),
            pseudo,
        })
        .collect();
    friends.sort_by(|a, b| natural_cmp(&a.pseudo, &b.pseudo));
    Ok(data(
        StatusCode::OK,
        Index {
            conversations: lines,
            friends,
        },
    ))
}

/// `POST /v1/conversations` `{"pseudo": …}`: the conversation with a
/// friend, started if needed (201) or found (200).
pub async fn open(
    State(state): State<AppState>,
    Verified(me): Verified,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<OpenBody>,
) -> Result<Response, ApiError> {
    key(&state)?;
    let pseudo = body.pseudo.trim();
    if pseudo.is_empty() {
        return Err(invalid("pseudo", "required", "is required"));
    }
    let other = member_id(&state, pseudo, uri.path()).await?;
    if other == me.id() {
        return Err(invalid(
            "pseudo",
            "self",
            "You cannot start a conversation with yourself.",
        ));
    }
    if !friends::are_friends(&state.db, me.id(), other).await? {
        return Err(forbidden(
            "You must be friends to start a conversation.",
            uri.path(),
        ));
    }
    let (id, created) = conversations::open(&state.db, me.id(), other).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok(data(status, Started { id }))
}

/// The conversation `raw_id` if `me` is in it; 404 otherwise.
async fn mine(
    state: &AppState,
    me: &Member,
    raw_id: &str,
    path: &str,
) -> Result<Conversation, ApiError> {
    let gone = || not_found("No such conversation of yours.", path);
    let id = path_id(raw_id).ok_or_else(gone)?;
    conversations::find(&state.db, id)
        .await?
        .filter(|c| c.contains(me.id()))
        .ok_or_else(gone)
}

/// `GET /v1/conversations/{id}`: the latest messages, oldest first. What
/// the other member sent is marked read.
pub async fn show(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let key = key(&state)?;
    let conversation = mine(&state, &me, &id, uri.path()).await?;
    conversations::mark_read(&state.db, conversation.id, me.id()).await?;
    let other = conversation.other_than(me.id());
    let limit = i64::from(super::THREAD_LIMIT);
    let (rows, is_friend, profile) = tokio::try_join!(
        conversations::messages(&state.db, conversation.id, Window::Latest, limit + 1),
        friends::are_friends(&state.db, me.id(), other),
        crate::db::members::find_by_id_brief(&state.db, other),
    )?;
    let (rows, has_older) = cut(rows, Window::Latest, limit);
    let (pseudo, avatar) = profile.unwrap_or_default();
    let messages = rows
        .into_iter()
        .map(|row| message(key, conversation.id, row))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(data(
        StatusCode::OK,
        Opened {
            conversation: Info {
                id: conversation.id,
                with: Correspondent {
                    avatar_url: avatar_url(&state.config.public_storage_url, avatar),
                    pseudo,
                },
                is_friend,
            },
            messages,
            has_older,
        },
    ))
}

/// `POST /v1/conversations/{id}/read`: marks read what the other member
/// sent, as opening or reading the conversation does.
pub async fn read(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    key(&state)?;
    let conversation = mine(&state, &me, &id, uri.path()).await?;
    conversations::mark_read(&state.db, conversation.id, me.id()).await?;
    Ok(crate::routes::auth::empty())
}

/// `GET /v1/conversations/{id}/messages?after=…|before=…&limit=…`: the
/// thread, oldest first; `after` is for polling. Marks read as opening does.
pub async fn messages(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let key = key(&state)?;
    let (window, limit) = window(query.as_deref())?;
    let conversation = mine(&state, &me, &id, uri.path()).await?;
    conversations::mark_read(&state.db, conversation.id, me.id()).await?;
    let rows = conversations::messages(&state.db, conversation.id, window, limit + 1).await?;
    let (rows, has_more) = cut(rows, window, limit);
    let data = rows
        .into_iter()
        .map(|row| message(key, conversation.id, row))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(no_store(StatusCode::OK, &Thread { data, has_more }))
}

/// `POST /v1/conversations/{id}/messages`: friends only, muted members
/// excepted.
pub async fn post(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<MessageBody>,
) -> Result<Response, ApiError> {
    let key = key(&state)?;
    limit(
        &state,
        "direct-message",
        &me.id().to_string(),
        POST_LIMIT,
        crate::rate_limit::WINDOW_SECS,
    )
    .await?;
    let conversation = mine(&state, &me, &id, uri.path()).await?;
    let other = conversation.other_than(me.id());
    if !friends::are_friends(&state.db, me.id(), other).await? {
        return Err(forbidden(
            "You are no longer friends: this conversation is closed.",
            uri.path(),
        ));
    }
    me.require_unmuted(uri.path())?;
    let body = message_body(body.body)?;
    let (sealed, version) = crypto::seal(key, conversation.id, &body)
        .map_err(|e| ApiError::Internal(format!("cannot seal a private message: {e}")))?;
    let row = conversations::post(&state.db, conversation.id, me.id(), &sealed, version).await?;
    Ok(data(
        StatusCode::CREATED,
        message(key, conversation.id, row)?,
    ))
}
