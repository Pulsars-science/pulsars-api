//! `/v1/moderation/forum/*` and `/v1/moderation/members/{id}/mute`
//! (`openapi.yaml`, tag `forum-moderation`): Laravel's
//! `ModerationForumController`, for moderators (`ModeratorMember`).
//!
//! Everything can be undone: hiding keeps the message behind a notice,
//! deleting is a soft delete that a moderator can restore, and a mute ends
//! on its own. Hiding, locking and pinning toggle, as on the site; each
//! answer says the state reached.

use axum::extract::{OriginalUri, Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::auth::{Data, empty, no_store};
use super::courses::timestamp;
use crate::auth::member::ModeratorMember;
use crate::db::forum::{self, Flag, Target};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::state::AppState;

const REASON_MAX_CHARS: usize = 300;
const MUTE_MAX_DAYS: i64 = 365;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HideBody {
    reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MuteBody {
    days: Option<i64>,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Hidden {
    hidden: bool,
}

#[derive(Debug, Serialize)]
struct Locked {
    locked: bool,
}

#[derive(Debug, Serialize)]
struct Pinned {
    pinned: bool,
}

#[derive(Debug, Serialize)]
struct Muted {
    user_id: String,
    muted_until: String,
    reason: String,
}

// --- Questions --------------------------------------------------------------

/// `POST /v1/moderation/forum/questions/{id}/hide`.
pub async fn hide_question(
    state: State<AppState>,
    moderator: ModeratorMember,
    Path(id): Path<String>,
    uri: OriginalUri,
    Json(body): Json<HideBody>,
) -> Result<Response, ApiError> {
    toggle_hidden(state, moderator, Target::Question, &id, uri, body).await
}

/// `POST /v1/moderation/forum/questions/{id}/lock`: no more answers, or
/// answers again.
pub async fn lock_question(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let locked = toggle(&state, &id, Flag::Locked, uri.path()).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Locked { locked },
        },
    ))
}

/// `POST /v1/moderation/forum/questions/{id}/pin`: first in every list, or
/// back in its place.
pub async fn pin_question(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let pinned = toggle(&state, &id, Flag::Pinned, uri.path()).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Pinned { pinned },
        },
    ))
}

/// `DELETE /v1/moderation/forum/questions/{id}`: a soft delete.
pub async fn delete_question(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    delete(&state, Target::Question, &id, uri.path()).await
}

/// `POST /v1/moderation/forum/questions/{id}/restore`.
pub async fn restore_question(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    restore(&state, Target::Question, &id, uri.path()).await
}

// --- Answers ----------------------------------------------------------------

/// `POST /v1/moderation/forum/answers/{id}/hide`.
pub async fn hide_answer(
    state: State<AppState>,
    moderator: ModeratorMember,
    Path(id): Path<String>,
    uri: OriginalUri,
    Json(body): Json<HideBody>,
) -> Result<Response, ApiError> {
    toggle_hidden(state, moderator, Target::Answer, &id, uri, body).await
}

/// `DELETE /v1/moderation/forum/answers/{id}`: a soft delete.
pub async fn delete_answer(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    delete(&state, Target::Answer, &id, uri.path()).await
}

/// `POST /v1/moderation/forum/answers/{id}/restore`.
pub async fn restore_answer(
    State(state): State<AppState>,
    _: ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    restore(&state, Target::Answer, &id, uri.path()).await
}

// --- Members ----------------------------------------------------------------

/// `PUT /v1/moderation/members/{id}/mute`: the member may read but not
/// post for `days` days.
pub async fn mute(
    State(state): State<AppState>,
    ModeratorMember(moderator): ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<MuteBody>,
) -> Result<Response, ApiError> {
    let user_id = target_member(&state, &moderator, &id, uri.path()).await?;
    let mut errors = Vec::new();
    match body.days {
        None => errors.push(ParameterError::field("days", "required", "is required")),
        Some(days) if !(1..=MUTE_MAX_DAYS).contains(&days) => errors.push(ParameterError::field(
            "days",
            "out_of_range",
            format!("must be between 1 and {MUTE_MAX_DAYS}"),
        )),
        Some(_) => {}
    }
    let reason = body.reason.trim();
    if reason.is_empty() {
        errors.push(ParameterError::field("reason", "required", "is required"));
    } else if reason.chars().count() > REASON_MAX_CHARS {
        errors.push(too_long_reason());
    }
    let days = match body.days.and_then(|d| i32::try_from(d).ok()) {
        Some(days) if errors.is_empty() => days,
        _ => return Err(ApiError::InvalidFields(errors)),
    };
    let until = forum::mute(&state.db, user_id, days, reason, moderator.id()).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Muted {
                user_id: user_id.to_string(),
                muted_until: timestamp(until),
                reason: reason.to_owned(),
            },
        },
    ))
}

/// `DELETE /v1/moderation/members/{id}/mute`.
pub async fn unmute(
    State(state): State<AppState>,
    ModeratorMember(moderator): ModeratorMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let user_id = target_member(&state, &moderator, &id, uri.path()).await?;
    if !forum::unmute(&state.db, user_id).await? {
        return Err(member_not_found(uri.path()));
    }
    Ok(empty())
}

/// The member a moderator acts on. Nobody mutes themselves, nor a member
/// whose permission level is as high as theirs: a moderator mutes members,
/// an admin also moderators, a super admin also admins.
async fn target_member(
    state: &AppState,
    moderator: &crate::auth::member::Member,
    id: &str,
    path: &str,
) -> Result<i64, ApiError> {
    let user_id = crate::auth::user_id(id).ok_or_else(|| member_not_found(path))?;
    let level = forum::permission_level(&state.db, user_id)
        .await?
        .ok_or_else(|| member_not_found(path))?;
    if user_id == moderator.id() || level >= moderator.level {
        return Err(ApiError::Forbidden {
            detail: "A moderator only mutes members of a lower permission level.".to_owned(),
            instance: Some(path.to_owned()),
        });
    }
    Ok(user_id)
}

// --- Shared -----------------------------------------------------------------

/// Hides the message with an optional reason, or shows it again if it is
/// hidden already (the reason is then ignored, as in Laravel).
async fn toggle_hidden(
    State(state): State<AppState>,
    ModeratorMember(moderator): ModeratorMember,
    target: Target,
    id: &str,
    OriginalUri(uri): OriginalUri,
    body: HideBody,
) -> Result<Response, ApiError> {
    let missing = || not_found(target, uri.path());
    let id = crate::auth::user_id(id).ok_or_else(missing)?;
    let hidden = forum::is_hidden(&state.db, target, id)
        .await?
        .ok_or_else(missing)?;
    let done = if hidden {
        forum::unhide(&state.db, target, id).await?
    } else {
        let reason = body
            .reason
            .map(|r| r.trim().to_owned())
            .filter(|r| !r.is_empty());
        if reason
            .as_deref()
            .is_some_and(|r| r.chars().count() > REASON_MAX_CHARS)
        {
            return Err(ApiError::InvalidFields(vec![too_long_reason()]));
        }
        forum::hide(&state.db, target, id, moderator.id(), reason.as_deref()).await?
    };
    if !done {
        return Err(missing());
    }
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Hidden { hidden: !hidden },
        },
    ))
}

async fn toggle(state: &AppState, id: &str, flag: Flag, path: &str) -> Result<bool, ApiError> {
    let missing = || not_found(Target::Question, path);
    let id = crate::auth::user_id(id).ok_or_else(missing)?;
    forum::toggle_flag(&state.db, id, flag)
        .await?
        .ok_or_else(missing)
}

async fn delete(
    state: &AppState,
    target: Target,
    id: &str,
    path: &str,
) -> Result<Response, ApiError> {
    let missing = || not_found(target, path);
    let id = crate::auth::user_id(id).ok_or_else(missing)?;
    if !forum::delete(&state.db, target, id).await? {
        return Err(missing());
    }
    Ok(empty())
}

async fn restore(
    state: &AppState,
    target: Target,
    id: &str,
    path: &str,
) -> Result<Response, ApiError> {
    let missing = || not_found(target, path);
    let id = crate::auth::user_id(id).ok_or_else(missing)?;
    if !forum::restore(&state.db, target, id).await? {
        return Err(missing());
    }
    Ok(empty())
}

fn too_long_reason() -> ParameterError {
    ParameterError::field(
        "reason",
        "too_long",
        format!("must be at most {REASON_MAX_CHARS} characters"),
    )
}

fn not_found(target: Target, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: format!("No such {}.", target.morph()),
        instance: Some(path.to_owned()),
    }
}

fn member_not_found(path: &str) -> ApiError {
    ApiError::NotFound {
        detail: "No such member.".to_owned(),
        instance: Some(path.to_owned()),
    }
}
