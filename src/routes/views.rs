//! Page views of courses and exams: their `vues` counters and the
//! dashboard's views by day (`site.content_views`).
//!
//! An exam's view is counted by `GET /v1/exams/{slug}/corrections`, which
//! its page calls on every display; a course has its own beacon,
//! `POST /v1/courses/{slug}/view`, that the course page calls once from the
//! browser (`GET /v1/courses/{slug}` is also read by lesson and exercise
//! pages, so it counts nothing). Either way the same reader is counted once
//! within [`VIEW_DEDUP_SECS`], as Laravel's `CompteurVues`.

use axum::extract::{OriginalUri, Path, State};
use axum::response::Response;
use fred::prelude::{ClientLike as _, KeysInterface as _};
use fred::types::{Expiration, SetOptions};

use super::auth::{Caller, empty};
use super::site::{self, not_found};
use crate::auth::member::MaybeMember;
use crate::db::courses;
use crate::error::ApiError;
use crate::state::AppState;

/// The same reader again within this delay is the same view (a reload).
pub const VIEW_DEDUP_SECS: i64 = 20;

/// Who reads, for the dedup: the member, or the visitor's fingerprint.
pub(crate) async fn reader(
    state: &AppState,
    member: Option<i64>,
    caller: &Caller,
) -> Result<String, ApiError> {
    Ok(match member {
        Some(id) => format!("membre:{id}"),
        None => format!(
            "invite:{}",
            site::visitor_hash(state, caller.ip.as_deref(), caller.user_agent.as_deref()).await?
        ),
    })
}

/// Whether this is the first view of `who` of the `kind` (`courses`,
/// `exams`) `id` in the delay (`SET NX EX`). Redis down: counted, as
/// Laravel's cache would have.
pub(crate) async fn first_view(state: &AppState, kind: &str, id: i64, who: &str) -> bool {
    if !state.redis.is_connected() {
        return true;
    }
    let key = format!("pulsars-api:views:{kind}:{id}:{who}");
    let set: Result<Option<String>, _> = state
        .redis
        .set(
            key,
            "1",
            Some(Expiration::EX(VIEW_DEDUP_SECS)),
            Some(SetOptions::NX),
            false,
        )
        .await;
    !matches!(set, Ok(None))
}

/// `POST /v1/courses/{slug}/view`: the course page was displayed. 204,
/// counted or not (a robot, a reload); 404 for a course that is not
/// published.
pub async fn course(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = courses::published_id(&state.db, &slug)
        .await?
        .ok_or_else(|| {
            not_found(
                format!("No published course has the slug '{slug}'."),
                uri.path(),
            )
        })?;
    if site::is_robot(caller.user_agent.as_deref()) {
        return Ok(empty());
    }
    let who = reader(&state, member.as_ref().map(|m| m.id()), &caller).await?;
    if first_view(&state, "courses", id, &who).await {
        courses::add_view(&state.db, id).await?;
    }
    Ok(empty())
}
