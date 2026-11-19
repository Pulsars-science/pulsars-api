//! The admin journal: who did what (`docs/privacy.md`).
//!
//! - `GET /v1/admin/people?q=`: members by e-mail or pseudo, or the
//!   visitor of an exact visitor id.
//! - `GET /v1/admin/people/members/{id}` and `/visitors/{uuid}`: a summary.
//! - `GET /v1/admin/people/members/{id}/events`, `/visitors/{uuid}/events`
//!   and `GET /v1/admin/activity`: timelines, newest first, by keyset
//!   cursor (`?limit=&cursor=&kinds=`).

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::ok;
use crate::auth::member::AdminMember;
use crate::db::journal::{self, After, EventRow, Scope};
use crate::error::{ApiError, ParameterError};
use crate::routes::auth::no_store;
use crate::routes::courses::timestamp;
use crate::routes::cursor;
use crate::routes::params::Params;
use crate::routes::site::not_found;
use crate::state::AppState;

const SEARCH_MIN_CHARS: usize = 2;
const SEARCH_MAX_CHARS: usize = 100;
const DEFAULT_LIMIT: u32 = 50;
const KINDS_MAX_CHARS: usize = 300;

fn at(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(timestamp)
}

#[derive(Debug, Serialize)]
pub struct People {
    pub members: Vec<FoundMember>,
    pub visitors: Vec<Visitor>,
}

#[derive(Debug, Serialize)]
pub struct FoundMember {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    pub created_at: Option<String>,
    /// The latest of their page views and of their sessions' use.
    pub last_seen_at: Option<String>,
    pub verified: bool,
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Visitor {
    pub visitor_id: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub page_views: i64,
    pub member_id: Option<i64>,
}

impl From<journal::VisitorRow> for Visitor {
    fn from(row: journal::VisitorRow) -> Self {
        Self {
            visitor_id: row.visitor_id.to_string(),
            first_seen_at: timestamp(row.first_seen_at),
            last_seen_at: timestamp(row.last_seen_at),
            page_views: row.page_views,
            member_id: row.member_id,
        }
    }
}

/// `GET /v1/admin/people?q=`: at least 2 characters.
pub async fn search(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let q = params.text("q", SEARCH_MAX_CHARS);
    params.finish()?;
    let q = q.map(|q| q.trim().to_owned()).unwrap_or_default();
    if q.chars().count() < SEARCH_MIN_CHARS {
        return Err(ApiError::InvalidParameter(vec![ParameterError {
            parameter: "q".to_owned(),
            detail: format!("must be at least {SEARCH_MIN_CHARS} characters"),
            code: None,
        }]));
    }
    let members = journal::search_members(&state.db, &q)
        .await?
        .into_iter()
        .map(|m| FoundMember {
            id: m.id,
            pseudo: m.pseudo,
            email: m.email,
            created_at: at(m.created_at),
            last_seen_at: at(m.last_seen_at),
            verified: m.verified,
            roles: m.roles,
        })
        .collect();
    let visitors = match Uuid::try_parse(&q) {
        Ok(id) => journal::visitor(&state.db, id)
            .await?
            .into_iter()
            .map(Visitor::from)
            .collect(),
        Err(_) => Vec::new(),
    };
    Ok(ok(People { members, visitors }))
}

#[derive(Debug, Serialize)]
pub struct Consent {
    pub measure: bool,
    pub at: String,
}

#[derive(Debug, Serialize)]
pub struct MemberSummary {
    pub member: MemberInfo,
    pub counts: MemberCounts,
    /// The member's latest answer to the consent banner.
    pub consent: Option<Consent>,
}

#[derive(Debug, Serialize)]
pub struct MemberInfo {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    pub created_at: Option<String>,
    pub verified: bool,
    pub onboarded: bool,
    pub roles: Vec<String>,
    pub xp: i64,
    pub brains: i64,
    pub last_seen_at: Option<String>,
    /// The visitor ids linked to the member, the latest first.
    pub visitor_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MemberCounts {
    pub page_views: i64,
    pub lessons_completed: i64,
    pub exercise_attempts: i64,
    pub games: i64,
    pub forum_posts: i64,
    pub room_messages: i64,
    pub downloads: i64,
    pub logins: i64,
}

fn member_id(raw: &str, path: &str) -> Result<i64, ApiError> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| not_found(format!("No member has the id '{raw}'."), path))
}

fn visitor_id(raw: &str, path: &str) -> Result<Uuid, ApiError> {
    Uuid::try_parse(raw).map_err(|_| not_found(format!("No visitor has the id '{raw}'."), path))
}

/// `GET /v1/admin/people/members/{id}`.
pub async fn member(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = member_id(&id, uri.path())?;
    let row = journal::member(&state.db, id)
        .await?
        .ok_or_else(|| not_found(format!("No member has the id '{id}'."), uri.path()))?;
    let consent = journal::latest_consent(&state.db, Some(id), None).await?;
    Ok(ok(MemberSummary {
        member: MemberInfo {
            id: row.id,
            pseudo: row.pseudo,
            email: row.email,
            created_at: at(row.created_at),
            verified: row.verified,
            onboarded: row.onboarded,
            roles: row.roles,
            xp: row.xp,
            brains: row.brains,
            last_seen_at: at(row.last_seen_at),
            visitor_ids: row.visitor_ids.iter().map(Uuid::to_string).collect(),
        },
        counts: MemberCounts {
            page_views: row.page_views,
            lessons_completed: row.lessons_completed,
            exercise_attempts: row.exercise_attempts,
            games: row.games,
            forum_posts: row.forum_posts,
            room_messages: row.room_messages,
            downloads: row.downloads,
            logins: row.logins,
        },
        consent: consent.map(|(measure, at)| Consent {
            measure,
            at: timestamp(at),
        }),
    }))
}

#[derive(Debug, Serialize)]
pub struct VisitorSummary {
    pub visitor: VisitorInfo,
    pub counts: VisitorCounts,
    pub consent: Option<Consent>,
}

#[derive(Debug, Serialize)]
pub struct VisitorInfo {
    pub visitor_id: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub member_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct VisitorCounts {
    pub page_views: i64,
}

/// `GET /v1/admin/people/visitors/{uuid}`.
pub async fn visitor(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = visitor_id(&id, uri.path())?;
    let row = journal::visitor(&state.db, id)
        .await?
        .ok_or_else(|| not_found(format!("No visitor has the id '{id}'."), uri.path()))?;
    let consent = journal::latest_consent(&state.db, None, Some(id)).await?;
    Ok(ok(VisitorSummary {
        visitor: VisitorInfo {
            visitor_id: row.visitor_id.to_string(),
            first_seen_at: timestamp(row.first_seen_at),
            last_seen_at: timestamp(row.last_seen_at),
            member_id: row.member_id,
        },
        counts: VisitorCounts {
            page_views: row.page_views,
        },
        consent: consent.map(|(measure, at)| Consent {
            measure,
            at: timestamp(at),
        }),
    }))
}

#[derive(Debug, Serialize)]
pub struct Item {
    /// `<kind>:<row id>`.
    pub id: String,
    pub at: String,
    pub kind: String,
    /// On `/v1/admin/activity` only: `{member_id, pseudo}`, `{visitor_id}`,
    /// or null (a failed login on an unknown account).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub who: Option<Value>,
    pub data: Value,
    pub link: Option<String>,
}

#[derive(Debug, Serialize)]
struct Page {
    data: Vec<Item>,
    next_cursor: Option<String>,
}

fn item(row: EventRow, with_who: bool) -> Item {
    let who = with_who.then(|| match (row.user_id, row.visitor_id) {
        (Some(member_id), _) => json!({"member_id": member_id, "pseudo": row.pseudo}),
        (None, Some(visitor_id)) => json!({"visitor_id": visitor_id.to_string()}),
        (None, None) => Value::Null,
    });
    Item {
        id: format!("{}:{}", row.kind, row.id),
        at: timestamp_micros(row.at),
        kind: row.kind,
        who,
        data: row.data.0,
        link: row.link,
    }
}

/// Seconds are not enough to tell items apart: page views keep their
/// microseconds.
fn timestamp_micros(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

/// `kinds=a,b`: every one must be known.
fn parse_kinds(raw: Option<&str>) -> Result<Vec<&'static str>, ApiError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let known = journal::kinds();
    let mut kinds = Vec::new();
    for name in raw.split(',').map(str::trim).filter(|k| !k.is_empty()) {
        match known.iter().find(|k| **k == name) {
            Some(kind) if !kinds.contains(kind) => kinds.push(*kind),
            Some(_) => {}
            None => {
                return Err(ApiError::InvalidParameter(vec![ParameterError {
                    parameter: "kinds".to_owned(),
                    detail: format!("unknown kind `{name}`; known: {}", known.join(", ")),
                    code: None,
                }]));
            }
        }
    }
    kinds.sort_unstable();
    Ok(kinds)
}

fn decode_after(cursor: &str, fingerprint: &str) -> Result<After, ApiError> {
    match cursor::decode(cursor, fingerprint)?.as_slice() {
        [micros, Value::String(kind), id] => Ok(After {
            at: micros
                .as_i64()
                .and_then(DateTime::from_timestamp_micros)
                .ok_or(ApiError::InvalidCursor)?,
            kind: kind.clone(),
            id: id.as_i64().ok_or(ApiError::InvalidCursor)?,
        }),
        _ => Err(ApiError::InvalidCursor),
    }
}

async fn timeline(
    state: &AppState,
    scope: Scope,
    query: Option<&str>,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query);
    let limit = params.limit_or(DEFAULT_LIMIT);
    let cursor = params.cursor();
    let kinds = params.text("kinds", KINDS_MAX_CHARS);
    params.finish()?;
    let kinds = parse_kinds(kinds.as_deref())?;
    let scope_key = match scope {
        Scope::Member(id) => format!("member:{id}"),
        Scope::Visitor(id) => format!("visitor:{id}"),
        Scope::Everybody => "all".to_owned(),
    };
    let fingerprint = cursor::fingerprint(&format!(
        "admin-journal\u{0}{scope_key}\u{0}{}",
        kinds.join(",")
    ));
    let after = cursor.map(|c| decode_after(&c, &fingerprint)).transpose()?;
    let rows = journal::timeline(
        &state.db,
        scope,
        &kinds,
        after.as_ref(),
        i64::from(limit) + 1,
    )
    .await?;
    let more = rows.len() > limit as usize;
    let mut rows = rows;
    rows.truncate(limit as usize);
    let next_cursor = rows.last().filter(|_| more).map(|row| {
        cursor::encode(
            vec![
                Value::from(row.at.timestamp_micros()),
                Value::from(row.kind.clone()),
                Value::from(row.id),
            ],
            &fingerprint,
        )
    });
    let with_who = scope == Scope::Everybody;
    Ok(no_store(
        StatusCode::OK,
        &Page {
            data: rows.into_iter().map(|r| item(r, with_who)).collect(),
            next_cursor,
        },
    ))
}

/// `GET /v1/admin/people/members/{id}/events`.
pub async fn member_events(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let id = member_id(&id, uri.path())?;
    if !journal::member_exists(&state.db, id).await? {
        return Err(not_found(
            format!("No member has the id '{id}'."),
            uri.path(),
        ));
    }
    timeline(&state, Scope::Member(id), query.as_deref()).await
}

/// `GET /v1/admin/people/visitors/{uuid}/events`: page views and consents.
pub async fn visitor_events(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let id = visitor_id(&id, uri.path())?;
    timeline(&state, Scope::Visitor(id), query.as_deref()).await
}

/// `GET /v1/admin/activity`: everybody's.
pub async fn activity(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    timeline(&state, Scope::Everybody, query.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_must_be_known() {
        assert_eq!(parse_kinds(None).unwrap(), Vec::<&str>::new());
        assert_eq!(
            parse_kinds(Some("security, page_view,security")).unwrap(),
            ["page_view", "security"]
        );
        assert!(matches!(
            parse_kinds(Some("page_view,nope")),
            Err(ApiError::InvalidParameter(_))
        ));
    }
}
