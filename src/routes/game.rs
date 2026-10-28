//! `/v1/game`: the match WebSocket, the leaderboard, the player's own
//! ratings and matches, and the admin statistics (`docs/game-protocol.md`
//! §3 and §9, `openapi.yaml` tag `game`).

use axum::Json;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::{OriginalUri, RawQuery, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use chrono::SecondsFormat;
use serde::Serialize;
use serde_json::Value;

use super::params::Params;
use crate::auth::{self, Admin, AuthUser};
use crate::db::members;
use crate::error::ApiError;
use crate::game::glicko::PROVISIONAL_RD;
use crate::game::rules::{SUBJECT_KEYS, SubjectKey};
use crate::game::{session, stats, store};
use crate::links::encode;
use crate::state::AppState;

/// `Cache-Control` of the leaderboard.
pub const CACHE_LEADERBOARD: &str = "public, max-age=30";

/// Largest frame a client may send: every client message is small.
const MAX_MESSAGE_BYTES: usize = 4096;

/// Leaderboard size when `limit` is absent.
const LEADERBOARD_DEFAULT_LIMIT: u32 = 50;

/// `GET /v1/game/ws`: the match WebSocket. The `Origin` must be one of
/// `CORS_ALLOWED_ORIGINS`, checked before the upgrade.
pub async fn ws(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !session::origin_allowed(
        headers.get(header::ORIGIN),
        &state.config.cors_allowed_origins,
    ) {
        return ApiError::Forbidden {
            detail: "This origin may not open a game session.".to_owned(),
            instance: Some(uri.path().to_owned()),
        }
        .into_response();
    }
    let upgrade = match upgrade {
        Ok(upgrade) => upgrade,
        Err(_) => return ApiError::UpgradeRequired.into_response(),
    };
    upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| session::run(socket, state))
}

#[derive(Debug, Serialize)]
pub struct Leaderboard {
    pub subject: SubjectKey,
    pub data: Vec<Leader>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Leader {
    pub rank: i64,
    pub user_id: String,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub rating: i64,
    pub provisional: bool,
    pub games: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
}

/// `GET /v1/game/leaderboard?subject=maths&limit=50`.
pub async fn leaderboard(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let subject = params.one_of("subject", SUBJECT_KEYS);
    let limit = params.limit_or(LEADERBOARD_DEFAULT_LIMIT);
    params.finish()?;
    let subject = subject
        .and_then(SubjectKey::parse)
        .unwrap_or(SubjectKey(None));

    let rows = store::leaderboard(&state.db, subject, i64::from(limit)).await?;
    let storage = &state.config.public_storage_url;
    let body = Leaderboard {
        subject,
        data: rows
            .into_iter()
            .map(|row| Leader {
                rank: row.rank,
                user_id: row.user_id.to_string(),
                pseudo: row.pseudo,
                avatar_url: avatar_url(row.avatar_path, storage),
                rating: row.rating.round() as i64,
                provisional: row.rd > PROVISIONAL_RD,
                games: row.games,
                wins: row.wins,
                losses: row.losses,
                draws: row.draws,
            })
            .collect(),
    };
    let mut response = Json(body).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_LEADERBOARD),
    );
    Ok(response)
}

fn avatar_url(path: Option<String>, storage: &str) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| format!("{storage}/{}", encode(p.trim_start_matches('/'), true)))
}

#[derive(Debug, Serialize)]
pub struct MyGame {
    /// Brains balance of the token's owner (`brains.md` §4).
    pub brains: i64,
    pub ratings: Vec<MyRating>,
    pub recent: Vec<Recent>,
}

#[derive(Debug, Serialize)]
pub struct MyRating {
    pub subject: String,
    pub rating: i64,
    pub provisional: bool,
    pub games: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
    pub rank: i64,
}

#[derive(Debug, Serialize)]
pub struct Recent {
    pub match_id: String,
    pub mode: String,
    pub ranked: bool,
    pub subject: String,
    pub rules: Value,
    pub ended_at: String,
    pub end_reason: String,
    pub outcome: String,
    pub score: i32,
    pub rating_before: Option<i32>,
    pub rating_after: Option<i32>,
    pub opponents: Vec<String>,
}

/// `GET /v1/game/me`: the token's brains, ratings and last 20 matches.
pub async fn me(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let user_id = auth::user_id(&claims.sub).ok_or_else(|| ApiError::NotFound {
        detail: "The account of this token no longer exists.".to_owned(),
        instance: Some(uri.path().to_owned()),
    })?;
    let ratings = store::my_ratings(&state.db, user_id).await?;
    let recent = store::recent(&state.db, user_id).await?;
    let brains = members::brains(&state.db, user_id).await?;
    let body = MyGame {
        brains: i64::from(brains),
        ratings: ratings
            .into_iter()
            .map(|r| MyRating {
                subject: r.subject,
                rating: r.rating.round() as i64,
                provisional: r.rd > PROVISIONAL_RD,
                games: r.games,
                wins: r.wins,
                losses: r.losses,
                draws: r.draws,
                rank: r.rank,
            })
            .collect(),
        recent: recent
            .into_iter()
            .map(|m| Recent {
                match_id: m.match_id.to_string(),
                mode: m.mode,
                ranked: m.ranked,
                subject: m.subject,
                rules: m.rules.0,
                ended_at: m.ended_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                end_reason: m.end_reason,
                outcome: m.outcome,
                score: m.score,
                rating_before: m.rating_before,
                rating_after: m.rating_after,
                opponents: m.opponents,
            })
            .collect(),
    };
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.append(header::VARY, HeaderValue::from_static("Authorization"));
    Ok(response)
}

/// `GET /v1/game/admin/stats?days=30`: live counters and the activity of
/// the last `days` UTC days, for a game token with the admin role. The
/// site's dashboard reads them with a session: `GET /v1/admin/game/stats`.
pub async fn admin_stats(
    State(state): State<AppState>,
    _admin: Admin,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let days = params.integer("days", 1, stats::MAX_DAYS, stats::DEFAULT_DAYS);
    params.finish()?;

    // The live snapshot first: see `game::stats`.
    let live = state
        .game
        .live()
        .await
        .ok_or_else(|| ApiError::Unavailable("the game hub does not answer".to_owned()))?;
    let body = stats::collect(&state.db, chrono::Utc::now(), days, &live).await?;
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.append(header::VARY, HeaderValue::from_static("Authorization"));
    Ok(response)
}
