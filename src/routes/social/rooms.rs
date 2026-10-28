//! Rooms: `/v1/rooms/*` and `/v1/invitations/{token}`, as Laravel's
//! `SalonController`, `MessageSalonController`, `InvitationSalonController`
//! and `Salons` service.
//!
//! - A `public` room is listed and readable by every member; one joins it
//!   freely, or by writing in it.
//! - A `prive` (private) room is listed for, and readable by, its members only, and
//!   site moderators. One enters it by an invitation link only.
//! - Its owner (its creator) and site moderators manage it: invitations,
//!   deletion. The owner cannot leave it, only delete it.
//!
//! Others asking for a private room get a 404, as if it did not exist.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{
    ALNUM, Thread, author, cut, data, forbidden, invalid, invite_link, message_body, not_found,
    random_token, timestamp, trimmed, window,
};
use crate::auth::member::{Member, Verified};
use crate::db::rooms::{self, Accepted, Invite, Room, Window};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::routes::auth::empty;
use crate::state::AppState;

/// Length of an invitation token (`Str::random(24)`).
const INVITE_TOKEN_CHARS: usize = 24;
/// Slug suffix on a collision (`Str::lower(Str::random(4))`).
const SLUG_SUFFIX_CHARS: usize = 4;

// --- Shapes ----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Listed {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    /// `public` or `prive`, as Laravel stores it.
    #[serde(rename = "type")]
    pub kind: String,
    pub is_private: bool,
    pub member_count: i64,
    pub message_count: i64,
    /// The reader is a member.
    pub joined: bool,
}

#[derive(Debug, Serialize)]
pub struct RoomInfo {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub is_private: bool,
    pub member_count: i64,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub id: i64,
    pub body: String,
    /// The author's pseudo; `null` once the account is deleted.
    pub author: Option<String>,
    pub author_id: Option<String>,
    pub sent_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Viewer {
    pub joined: bool,
    /// A member, or anyone in a public room (writing joins it).
    pub can_write: bool,
    pub can_manage: bool,
}

#[derive(Debug, Serialize)]
pub struct Opened {
    #[serde(flatten)]
    pub room: RoomInfo,
    pub viewer: Viewer,
    /// The latest messages, oldest first.
    pub messages: Vec<Message>,
    /// Older messages exist: `GET …/messages?before={first id}`.
    pub has_older: bool,
}

#[derive(Debug, Serialize)]
pub struct InvitationLine {
    pub token: String,
    /// The page of the link on the site.
    pub url: String,
    /// Pseudo of who made it.
    pub creator: Option<String>,
    pub uses: i32,
    pub max_uses: Option<i32>,
    pub expires_at: Option<String>,
    pub valid: bool,
    /// `revoked`, `expired` or `exhausted`; `null` while valid.
    pub invalid_reason: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Invitations {
    pub room: RoomInfo,
    pub invitations: Vec<InvitationLine>,
}

#[derive(Debug, Serialize)]
pub struct Landing {
    pub valid: bool,
    pub invalid_reason: Option<&'static str>,
    pub invited_by: Option<String>,
    pub room: RoomInfo,
    pub already_member: bool,
}

#[derive(Debug, Serialize)]
pub struct Joined {
    pub room_slug: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CreateBody {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MessageBody {
    body: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InviteBody {
    /// Days the link works; `null`: forever.
    duration_days: Option<i64>,
    /// Members it may let in; `null`: no limit.
    max_uses: Option<i64>,
}

// --- Rights (`Room::peutVoir`, `peutEcrire`, `peutGerer`) -------------------

/// A room and where the reader stands in it.
struct Seen {
    room: Room,
    role: Option<String>,
    moderator: bool,
}

impl Seen {
    async fn load(state: &AppState, me: &Member, slug: &str, path: &str) -> Result<Self, ApiError> {
        let room = rooms::find(&state.db, slug)
            .await?
            .ok_or_else(|| no_room(slug, path))?;
        let role = rooms::role(&state.db, room.id, me.id()).await?;
        Ok(Self {
            room,
            role,
            moderator: me.is_moderator(),
        })
    }

    fn is_member(&self) -> bool {
        self.role.is_some()
    }

    fn can_see(&self) -> bool {
        self.room.is_public() || self.moderator || self.is_member()
    }

    fn is_owner(&self) -> bool {
        self.role.as_deref() == Some(rooms::OWNER)
    }

    fn can_manage(&self) -> bool {
        self.moderator || self.is_owner()
    }

    /// 404 unless the reader may see the room.
    fn visible(self, slug: &str, path: &str) -> Result<Self, ApiError> {
        if self.can_see() {
            Ok(self)
        } else {
            Err(no_room(slug, path))
        }
    }

    /// 403 unless the reader may manage the room.
    fn managed(self, path: &str) -> Result<Self, ApiError> {
        if self.can_manage() {
            Ok(self)
        } else {
            Err(forbidden(
                "Only the room's owner or a moderator may manage it.",
                path,
            ))
        }
    }
}

fn no_room(slug: &str, path: &str) -> ApiError {
    not_found(format!("No room has the slug {slug:?}."), path)
}

async fn info(state: &AppState, room: &Room) -> Result<RoomInfo, ApiError> {
    Ok(RoomInfo {
        name: room.name.clone(),
        slug: room.slug.clone(),
        description: room.description.clone(),
        is_private: !room.is_public(),
        kind: room.kind.clone(),
        member_count: rooms::member_count(&state.db, room.id).await?,
    })
}

fn message(row: rooms::Message) -> Message {
    let (author, author_id) = author(row.author_id, row.author_pseudo);
    Message {
        id: row.id,
        body: row.body,
        author,
        author_id,
        sent_at: timestamp(row.created_at),
    }
}

// --- Rooms -------------------------------------------------------------------

/// `GET /v1/rooms`: the public rooms and the private ones the member
/// belongs to, busiest first.
pub async fn index(
    State(state): State<AppState>,
    Verified(me): Verified,
) -> Result<Response, ApiError> {
    let rows = rooms::visible_to(&state.db, me.id()).await?;
    let list: Vec<Listed> = rows
        .into_iter()
        .map(|r| Listed {
            is_private: r.kind != rooms::PUBLIC,
            kind: r.kind,
            name: r.name,
            slug: r.slug,
            description: r.description,
            member_count: r.member_count,
            message_count: r.message_count,
            joined: r.joined,
        })
        .collect();
    Ok(data(StatusCode::OK, list))
}

/// `POST /v1/rooms`: creates a room; its creator owns it.
pub async fn create(
    State(state): State<AppState>,
    Verified(me): Verified,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let name = trimmed(body.name);
    let description = trimmed(body.description);
    let kind = trimmed(body.kind);
    let mut errors = Vec::new();
    match &name {
        None => errors.push(ParameterError::field("name", "required", "is required")),
        Some(n) if n.chars().count() < 2 => errors.push(ParameterError::field(
            "name",
            "too_short",
            "must be at least 2 characters",
        )),
        Some(n) if n.chars().count() > 50 => errors.push(ParameterError::field(
            "name",
            "too_long",
            "must be at most 50 characters",
        )),
        Some(_) => {}
    }
    if description
        .as_ref()
        .is_some_and(|d| d.chars().count() > 280)
    {
        errors.push(ParameterError::field(
            "description",
            "too_long",
            "must be at most 280 characters",
        ));
    }
    let stored_kind = match kind.as_deref() {
        None => {
            errors.push(ParameterError::field("type", "required", "is required"));
            None
        }
        Some("public") => Some(rooms::PUBLIC),
        Some("prive") => Some(rooms::PRIVATE),
        Some(_) => {
            errors.push(ParameterError::field(
                "type",
                "invalid",
                "must be public or prive",
            ));
            None
        }
    };
    let (Some(name), Some(stored_kind), true) = (name, stored_kind, errors.is_empty()) else {
        return Err(ApiError::InvalidFields(errors));
    };

    // A readable slug; on a collision, a short random suffix rather than a
    // counter (`Salons::slugUnique`).
    let base = rooms::base_slug(&state.db, &name).await?;
    let mut slug = base.clone();
    let room = loop {
        if let Some(room) = rooms::create(
            &state.db,
            me.id(),
            &name,
            &slug,
            description.as_deref(),
            stored_kind,
        )
        .await?
        {
            break room;
        }
        slug = format!(
            "{base}-{}",
            random_token(SLUG_SUFFIX_CHARS, super::LOWER_ALNUM)?
        );
    };
    let opened = opened(&state, &me, room, Some(rooms::OWNER.to_owned())).await?;
    Ok(data(StatusCode::CREATED, opened))
}

/// `GET /v1/rooms/{slug}`: the room, its latest messages and the reader's
/// rights. Opening it marks it read for a member.
pub async fn show(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .visible(&slug, uri.path())?;
    if seen.is_member() {
        rooms::mark_read(&state.db, seen.room.id, me.id()).await?;
    }
    let opened = opened(&state, &me, seen.room, seen.role).await?;
    Ok(data(StatusCode::OK, opened))
}

async fn opened(
    state: &AppState,
    me: &Member,
    room: Room,
    role: Option<String>,
) -> Result<Opened, ApiError> {
    let limit = i64::from(super::THREAD_LIMIT);
    let rows = rooms::messages(&state.db, room.id, Window::Latest, limit + 1).await?;
    let (rows, has_older) = cut(rows, Window::Latest, limit);
    let joined = role.is_some();
    let can_manage = me.is_moderator() || role.as_deref() == Some(rooms::OWNER);
    Ok(Opened {
        room: info(state, &room).await?,
        viewer: Viewer {
            joined,
            // Writing in a public room joins it (`MessageSalonController`).
            can_write: joined || room.is_public(),
            can_manage,
        },
        messages: rows.into_iter().map(message).collect(),
        has_older,
    })
}

/// `DELETE /v1/rooms/{slug}`: the owner or a moderator deletes the room,
/// with its messages and invitations.
pub async fn destroy(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .managed(uri.path())?;
    rooms::delete(&state.db, seen.room.id).await?;
    Ok(empty())
}

/// `POST /v1/rooms/{slug}/members`: joins; public rooms only; a private one is
/// entered by an invitation link.
pub async fn join(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let room = rooms::find(&state.db, &slug)
        .await?
        .ok_or_else(|| no_room(&slug, uri.path()))?;
    if !room.is_public() {
        return Err(forbidden(
            "A private room is entered by an invitation link only.",
            uri.path(),
        ));
    }
    rooms::join(&state.db, room.id, me.id()).await?;
    Ok(empty())
}

/// `DELETE /v1/rooms/{slug}/members/me`: leaves. Its owner cannot leave a room: they
/// delete it instead.
pub async fn leave(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path()).await?;
    if seen.is_owner() {
        return Err(invalid(
            "room",
            "owner",
            "The owner cannot leave their room; delete it instead.",
        ));
    }
    rooms::leave(&state.db, seen.room.id, me.id()).await?;
    Ok(empty())
}

/// `POST /v1/rooms/{slug}/read`: clears the member's unread count, as
/// opening the room, reading or writing in it does.
pub async fn read(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .visible(&slug, uri.path())?;
    rooms::mark_read(&state.db, seen.room.id, me.id()).await?;
    Ok(empty())
}

// --- Messages ----------------------------------------------------------------

/// `GET /v1/rooms/{slug}/messages?after=…|before=…&limit=…`: the thread,
/// oldest first. `after` is for polling: what came after the last message
/// shown. Reading marks the room read for a member.
pub async fn messages(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let (window, limit) = window(query.as_deref())?;
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .visible(&slug, uri.path())?;
    let rows = rooms::messages(&state.db, seen.room.id, window, limit + 1).await?;
    let (rows, has_more) = cut(rows, window, limit);
    if seen.is_member() {
        rooms::mark_read(&state.db, seen.room.id, me.id()).await?;
    }
    Ok(crate::routes::auth::no_store(
        StatusCode::OK,
        &Thread {
            data: rows.into_iter().map(message).collect::<Vec<_>>(),
            has_more,
        },
    ))
}

/// `POST /v1/rooms/{slug}/messages`. Writing in a public room joins it; in
/// a private one, only its members write. Muted members cannot post.
pub async fn post(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<MessageBody>,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .visible(&slug, uri.path())?;
    if !seen.is_member() && !seen.room.is_public() {
        return Err(forbidden(
            "Only the members of a private room write in it.",
            uri.path(),
        ));
    }
    me.require_unmuted(uri.path())?;
    let body = message_body(body.body)?;
    if !seen.is_member() {
        rooms::join(&state.db, seen.room.id, me.id()).await?;
    }
    let row = rooms::post(&state.db, seen.room.id, me.id(), &body).await?;
    rooms::mark_read(&state.db, seen.room.id, me.id()).await?;
    Ok(data(StatusCode::CREATED, message(row)))
}

// --- Invitations -------------------------------------------------------------

fn line(invite: Invite) -> InvitationLine {
    let reason = invite.invalid(Utc::now());
    InvitationLine {
        url: invite_link(&invite.token),
        token: invite.token,
        creator: invite.created_by,
        uses: invite.uses,
        max_uses: invite.max_uses,
        expires_at: timestamp(invite.expires_at),
        valid: reason.is_none(),
        invalid_reason: reason.map(|r| r.code()),
    }
}

/// `GET /v1/rooms/{slug}/invitations`: the links of the room, newest first.
pub async fn invitations(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .managed(uri.path())?;
    let invites = rooms::invites(&state.db, seen.room.id).await?;
    Ok(data(
        StatusCode::OK,
        Invitations {
            room: info(&state, &seen.room).await?,
            invitations: invites.into_iter().map(line).collect(),
        },
    ))
}

/// `POST /v1/rooms/{slug}/invitations`: a new link, for public and private
/// rooms alike.
pub async fn invite(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<InviteBody>,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .managed(uri.path())?;
    let mut errors = Vec::new();
    let mut bounded = |field: &str, value: Option<i64>, max: i64| {
        let value = value?;
        if (1..=max).contains(&value) {
            i32::try_from(value).ok()
        } else {
            errors.push(ParameterError::field(
                field,
                "out_of_range",
                format!("must be between 1 and {max}, or null"),
            ));
            None
        }
    };
    let days = bounded("duration_days", body.duration_days, 365);
    let max_uses = bounded("max_uses", body.max_uses, 10_000);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let invite = loop {
        let token = random_token(INVITE_TOKEN_CHARS, ALNUM)?;
        if let Some(invite) =
            rooms::create_invite(&state.db, seen.room.id, me.id(), &token, days, max_uses).await?
        {
            break invite;
        }
    };
    Ok(data(StatusCode::CREATED, line(invite)))
}

/// `DELETE /v1/rooms/{slug}/invitations/{token}`: revokes a link of this
/// room. A link of another room is not found here.
pub async fn revoke(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path((slug, token)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let seen = Seen::load(&state, &me, &slug, uri.path())
        .await?
        .managed(uri.path())?;
    let invite = rooms::invite(&state.db, &token)
        .await?
        .filter(|i| i.room_id == seen.room.id)
        .ok_or_else(|| no_invite(uri.path()))?;
    rooms::revoke_invite(&state.db, invite.id).await?;
    Ok(empty())
}

fn no_invite(path: &str) -> ApiError {
    not_found("No such invitation link.", path)
}

/// `GET /v1/invitations/{token}`: the landing page of a link. It shows
/// the room without joining it, and counts no use.
pub async fn invitation(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(token): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let invite = rooms::invite(&state.db, &token)
        .await?
        .ok_or_else(|| no_invite(uri.path()))?;
    let room = rooms::room_by_id(&state.db, invite.room_id)
        .await?
        .ok_or_else(|| no_invite(uri.path()))?;
    let already_member = rooms::role(&state.db, room.id, me.id()).await?.is_some();
    let reason = invite.invalid(Utc::now());
    Ok(data(
        StatusCode::OK,
        Landing {
            valid: reason.is_none(),
            invalid_reason: reason.map(|r| r.code()),
            invited_by: invite.created_by,
            room: info(&state, &room).await?,
            already_member,
        },
    ))
}

/// `POST /v1/invitations/{token}/accept`: joins the room of a valid link. A
/// use is counted only for a member who was not in it yet.
pub async fn accept(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(token): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let invite = rooms::invite(&state.db, &token)
        .await?
        .ok_or_else(|| no_invite(uri.path()))?;
    let room = rooms::room_by_id(&state.db, invite.room_id)
        .await?
        .ok_or_else(|| no_invite(uri.path()))?;
    match rooms::accept_invite(&state.db, invite.id, me.id()).await? {
        Accepted::Invalid(reason) => Err(invalid("invitation", reason.code(), reason.detail())),
        Accepted::Joined | Accepted::AlreadyMember => Ok(data(
            StatusCode::OK,
            Joined {
                room_slug: room.slug,
            },
        )),
    }
}
