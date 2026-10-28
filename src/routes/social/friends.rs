//! Friends: `GET /v1/friends`, `POST /v1/members/{pseudo}/friend-requests`,
//! `POST /v1/friends/{id}/accept`, `DELETE /v1/friends/{id}`, as Laravel's
//! `AmitieController`; and the QR code of `AmiQrController`:
//! `GET /v1/me/friend-qr`, `POST /v1/me/friend-qr/regenerate`,
//! `POST /v1/friends/scan`.
//!
//! Laravel drew the QR code as an SVG on the server. The API gives the link
//! it encodes (`https://pulsars.science/ami/{token}`); the site draws it.
//! Scanning it opens that page, which posts the token to `/v1/friends/scan`:
//! both members are friends at once, without a request to accept.

use axum::extract::{OriginalUri, Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::members::{friendship_state, member_id};
use super::{
    LOWER_ALNUM, MemberRef, avatar_url, data, forbidden, friend_link, invalid, not_found, path_id,
    random_token, timestamp,
};
use crate::auth::member::Verified;
use crate::db::friends::{self, Friendship};
use crate::error::ApiError;
use crate::extract::Json;
use crate::routes::auth::{empty, limit};
use crate::state::AppState;

/// Friend requests per minute per member (`throttle:20,1`).
const REQUEST_LIMIT: u32 = 20;
/// QR scans per minute per member (`throttle:30,1`).
const SCAN_LIMIT: u32 = 30;
/// Length of a friend token (`Str::lower(Str::random(24))`).
const TOKEN_CHARS: usize = 24;

#[derive(Debug, Serialize)]
pub struct Friend {
    pub id: String,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Line {
    pub friendship_id: i64,
    pub member: Friend,
    /// When the friendship, or the request, was made.
    pub since: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Lists {
    pub friends: Vec<Line>,
    /// Requests the member received, to accept.
    pub received: Vec<Line>,
    /// Requests the member sent, still pending.
    pub sent: Vec<Line>,
}

#[derive(Debug, Serialize)]
pub struct Qr {
    pub token: String,
    /// What the QR code encodes.
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct Scanned {
    /// `friends`, or `already_friends` when they were before this scan.
    pub state: &'static str,
    pub friendship_id: i64,
    pub member: MemberRef,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScanBody {
    token: String,
}

/// `GET /v1/friends`: friends, requests received and requests sent.
pub async fn index(
    State(state): State<AppState>,
    Verified(me): Verified,
) -> Result<Response, ApiError> {
    let rows = friends::of(&state.db, me.id()).await?;
    let storage = &state.config.public_storage_url;
    let mut lists = Lists {
        friends: Vec::new(),
        received: Vec::new(),
        sent: Vec::new(),
    };
    for row in rows {
        let list = if row.accepted {
            &mut lists.friends
        } else if row.requested_by_id == me.id() {
            &mut lists.sent
        } else {
            &mut lists.received
        };
        list.push(Line {
            friendship_id: row.id,
            member: Friend {
                id: row.other_id.to_string(),
                avatar_url: avatar_url(storage, row.avatar_path),
                pseudo: row.pseudo,
                bio: row.bio,
            },
            since: timestamp(row.created_at),
        });
    }
    Ok(data(StatusCode::OK, lists))
}

/// `POST /v1/members/{pseudo}/friend-requests`: a friend request. 201 when
/// sent; 200 when they are friends already (Laravel's « Vous êtes déjà
/// amis. », a success); 409 `request_pending` when a request between them,
/// of either side, waits (Laravel's error).
pub async fn request(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(pseudo): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    limit(
        &state,
        "friend-request",
        &me.id().to_string(),
        REQUEST_LIMIT,
        crate::rate_limit::WINDOW_SECS,
    )
    .await?;
    let target = member_id(&state, &pseudo, uri.path()).await?;
    if target == me.id() {
        return Err(invalid("pseudo", "self", "You cannot add yourself."));
    }
    let (friendship, created) = friends::request(&state.db, me.id(), target).await?;
    if created {
        return Ok(data(
            StatusCode::CREATED,
            friendship_state(Some(&friendship), me.id()),
        ));
    }
    if friendship.is_accepted() {
        return Ok(data(
            StatusCode::OK,
            friendship_state(Some(&friendship), me.id()),
        ));
    }
    Err(ApiError::Conflict {
        reason: "request_pending",
        detail: "A friend request between you is already pending.".to_owned(),
    })
}

/// `POST /v1/friends/{id}/accept`: only the member who received the request.
pub async fn accept(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let friendship = concerning(&state, &id, me.id(), uri.path()).await?;
    if friendship.requested_by_id == me.id() {
        return Err(forbidden(
            "Only the member who received the request may accept it.",
            uri.path(),
        ));
    }
    let friendship = friends::accept(&state.db, friendship.id)
        .await?
        .ok_or_else(|| no_friendship(uri.path()))?;
    Ok(data(
        StatusCode::OK,
        friendship_state(Some(&friendship), me.id()),
    ))
}

/// `DELETE /v1/friends/{id}`: removes a friend, or a request (sent or
/// received).
pub async fn remove(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let friendship = concerning(&state, &id, me.id(), uri.path()).await?;
    friends::delete(&state.db, friendship.id).await?;
    Ok(empty())
}

/// The friendship `raw_id` if `user_id` is one of its two members; 404
/// otherwise, so others learn nothing of it.
async fn concerning(
    state: &AppState,
    raw_id: &str,
    user_id: i64,
    path: &str,
) -> Result<Friendship, ApiError> {
    let Some(id) = path_id(raw_id) else {
        return Err(no_friendship(path));
    };
    friends::find(&state.db, id)
        .await?
        .filter(|f| f.concerns(user_id))
        .ok_or_else(|| no_friendship(path))
}

fn no_friendship(path: &str) -> ApiError {
    not_found("No such friendship of yours.", path)
}

/// `GET /v1/me/friend-qr`: the link the member's QR code encodes. The token is
/// made the first time.
pub async fn qr(
    State(state): State<AppState>,
    Verified(me): Verified,
) -> Result<Response, ApiError> {
    let fresh = random_token(TOKEN_CHARS, LOWER_ALNUM)?;
    let token = friends::token(&state.db, me.id(), &fresh).await?;
    Ok(data(StatusCode::OK, qr_of(token)))
}

/// `POST /v1/me/friend-qr/regenerate`: a new token; the previous QR code stops
/// working at once.
pub async fn renew_qr(
    State(state): State<AppState>,
    Verified(me): Verified,
) -> Result<Response, ApiError> {
    let token = random_token(TOKEN_CHARS, LOWER_ALNUM)?;
    friends::renew_token(&state.db, me.id(), &token).await?;
    Ok(data(StatusCode::OK, qr_of(token)))
}

fn qr_of(token: String) -> Qr {
    Qr {
        url: friend_link(&token),
        token,
    }
}

/// `POST /v1/friends/scan`: the member scanned someone's QR code. They are
/// friends at once, a pending request of either side included.
pub async fn scan(
    State(state): State<AppState>,
    Verified(me): Verified,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<ScanBody>,
) -> Result<Response, ApiError> {
    limit(
        &state,
        "friend-scan",
        &me.id().to_string(),
        SCAN_LIMIT,
        crate::rate_limit::WINDOW_SECS,
    )
    .await?;
    let token = body.token.trim();
    let owner = if token.is_empty() || token.len() > 32 {
        None
    } else {
        friends::owner_of_token(&state.db, token).await?
    };
    let Some((owner_id, owner_pseudo, owner_avatar)) = owner else {
        return Err(not_found("This QR code is no longer valid.", uri.path()));
    };
    if owner_id == me.id() {
        return Err(invalid("token", "self", "This is your own QR code."));
    }
    let (friendship, already) = friends::befriend(&state.db, me.id(), owner_id).await?;
    Ok(data(
        StatusCode::OK,
        Scanned {
            state: if already {
                "already_friends"
            } else {
                "friends"
            },
            friendship_id: friendship.id,
            member: MemberRef {
                id: owner_id.to_string(),
                avatar_url: avatar_url(&state.config.public_storage_url, owner_avatar),
                pseudo: owner_pseudo,
            },
        },
    ))
}
