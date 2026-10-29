//! `GET /v1/admin/dashboard` (Laravel's `TableauDeBordController`), the
//! pending badge, the donation goal, and the game and languages statistics
//! for a session (Laravel's `admin/jeu/statistiques` relay).

use axum::extract::{RawQuery, State};
use axum::response::Response;
use chrono::{DateTime, Days, Duration, Utc};
use fred::prelude::{ClientLike as _, Expiration, KeysInterface as _};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{Fields, ok};
use crate::auth::member::AdminMember;
use crate::db::admin_stats::{self as stats, Bounds, DonationGoal, Series};
use crate::db::language_stats;
use crate::error::ApiError;
use crate::extract::Json;
use crate::routes::courses::timestamp;
use crate::state::AppState;
use crate::umami::Umami;

/// Umami's numbers are kept five minutes: each one is an HTTP request, and
/// the dashboard is reloaded often.
const UMAMI_CACHE_SECS: i64 = 300;

#[derive(Debug, Serialize)]
pub struct Period {
    pub days: u32,
    pub choices: [u32; 3],
    pub dates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowCounts {
    #[serde(rename = "24h")]
    pub h24: i64,
    #[serde(rename = "7d")]
    pub d7: i64,
    #[serde(rename = "28d")]
    pub d28: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Measure {
    visitors: WindowCounts,
    page_views: WindowCounts,
    measured_since: String,
}

/// The audience according to Umami.
#[derive(Debug, Serialize)]
pub struct Audience {
    /// False when Umami is not configured or does not answer: the dashboard
    /// says so instead of showing zeros.
    pub available: bool,
    pub configured: bool,
    pub visitors: Option<WindowCounts>,
    pub page_views: Option<WindowCounts>,
    pub measured_since: Option<String>,
    /// Members with a dated activity today (UTC), the dashboard's `dau`:
    /// Laravel counted signed-in page views of the last 24 hours, which are
    /// gone.
    pub connected_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopPage {
    pub path: String,
    pub views: i64,
}

#[derive(Debug, Serialize)]
pub struct Dashboard {
    pub period: Period,
    pub audience: Audience,
    pub accounts: stats::Accounts,
    pub content: stats::Content,
    pub community: stats::Community,
    pub series: stats::AllSeries,
    pub active_members: stats::Active,
    pub funnel: Vec<stats::FunnelStep>,
    pub top_courses: Vec<stats::TopCourse>,
    pub brains: stats::Brains,
    pub top_brains: Vec<stats::TopBrains>,
    pub games: stats::Games,
    /// The most viewed pages of the period, according to Umami.
    pub popular_pages: Vec<TopPage>,
    pub latest_signups: Vec<stats::LatestSignup>,
    pub to_process: stats::ToDo,
    pub pending: stats::Pending,
    /// The raw amounts, to fill the form.
    pub donation_goal: DonationGoal,
    /// The API's requests, 5xx and p95 latency, kept by day.
    pub http: stats::Http,
    /// Published content by subject, with the period's downloads.
    pub catalog: Vec<stats::CatalogSubject>,
    /// The most viewed courses and exams of the period.
    pub top_content: Vec<stats::TopContent>,
}

fn days_param(query: Option<&str>) -> u32 {
    let days = form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .find(|(name, _)| name == "days")
        .map(|(_, value)| value.into_owned());
    stats::period(days.as_deref())
}

/// `GET /v1/admin/stats?days=7|30|90`: any other value is 30 days.
pub async fn dashboard(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let days = days_param(query.as_deref());
    let now = Utc::now();
    let bounds = Bounds::new(now, days);
    let db = &state.db;

    let umami = state
        .config
        .umami
        .as_ref()
        .and_then(|config| Umami::new(config, &state.redis));
    let (audience, visitors, pages) = match &umami {
        Some(umami) => (
            audience(&state, umami, now).await,
            visitors(&state, umami, &bounds, now).await,
            top_pages(&state, umami, days, now).await,
        ),
        None => (
            Audience {
                available: false,
                configured: false,
                visitors: None,
                page_views: None,
                measured_since: None,
                connected_24h: 0,
            },
            None,
            Vec::new(),
        ),
    };

    let active = stats::active(db, &bounds).await?;
    let mut audience = audience;
    audience.connected_24h = active.dau;
    let mut accounts = stats::accounts(db).await?;
    accounts.active_7d = active.wau;
    Ok(ok(Dashboard {
        period: Period {
            days,
            choices: stats::PERIODS,
            dates: bounds
                .dates()
                .iter()
                .map(|d| d.format("%Y-%m-%d").to_string())
                .collect(),
        },
        audience,
        accounts,
        content: stats::content(db).await?,
        community: stats::community(db).await?,
        series: stats::all_series(db, &bounds, visitors).await?,
        active_members: active,
        funnel: stats::funnel(db, &bounds).await?,
        top_courses: stats::top_courses(db, &bounds).await?,
        brains: stats::brains(db, &bounds).await?,
        top_brains: stats::top_brains(db).await?,
        games: stats::games(db, &bounds).await?,
        popular_pages: pages,
        latest_signups: stats::latest_signups(db).await?,
        to_process: stats::to_do(db).await?,
        pending: stats::pending(db).await?,
        donation_goal: stats::donation_goal(db).await?,
        http: stats::http(db, &bounds).await?,
        catalog: stats::catalog(db, &bounds).await?,
        top_content: stats::top_content(db, &bounds).await?,
    }))
}

/// A value kept in Redis for a few minutes; computed again when Redis is
/// down. `None` (Umami failed) is not kept.
async fn cached<T, F>(state: &AppState, key: &str, compute: F) -> Option<T>
where
    T: Serialize + DeserializeOwned,
    F: Future<Output = Option<T>>,
{
    let redis = &state.redis;
    if redis.is_connected()
        && let Ok(Some(json)) = redis.get::<Option<String>, _>(key).await
        && let Ok(value) = serde_json::from_str(&json)
    {
        return Some(value);
    }
    let value = compute.await?;
    if redis.is_connected()
        && let Ok(json) = serde_json::to_string(&value)
    {
        let _: Result<(), _> = redis
            .set(
                key,
                json,
                Some(Expiration::EX(UMAMI_CACHE_SECS)),
                None,
                false,
            )
            .await;
    }
    Some(value)
}

async fn audience(state: &AppState, umami: &Umami<'_>, now: DateTime<Utc>) -> Audience {
    let measure = cached(state, "admin:umami:audience", async {
        let since = umami.measured_since().await?;
        let mut visitors = [0i64; 4];
        let mut views = [0i64; 4];
        let starts = [
            now - Duration::hours(24),
            now - Duration::days(7),
            now - Duration::days(28),
            since,
        ];
        for (i, start) in starts.into_iter().enumerate() {
            let stats = umami.stats(start, now).await?;
            visitors[i] = stats.visitors;
            views[i] = stats.page_views;
        }
        let windows = |v: [i64; 4]| WindowCounts {
            h24: v[0],
            d7: v[1],
            d28: v[2],
            total: v[3],
        };
        Some(Measure {
            visitors: windows(visitors),
            page_views: windows(views),
            measured_since: timestamp(since),
        })
    })
    .await;
    Audience {
        available: measure.is_some(),
        configured: true,
        visitors: measure.as_ref().map(|m| m.visitors.clone()),
        page_views: measure.as_ref().map(|m| m.page_views.clone()),
        measured_since: measure.map(|m| m.measured_since),
        connected_24h: 0,
    }
}

/// Visitors by day over the period and the one before; null when Umami
/// fails (a flat line would read as no visitors).
async fn visitors(
    state: &AppState,
    umami: &Umami<'_>,
    bounds: &Bounds,
    now: DateTime<Utc>,
) -> Option<Series> {
    let key = format!("admin:umami:visitors:{}", bounds.days);
    let from = bounds.previous.and_time(chrono::NaiveTime::MIN).and_utc();
    let days: Vec<(String, i64)> = cached(state, &key, async {
        let days = umami.visitors_by_day(from, now).await?;
        Some(
            days.into_iter()
                .map(|(d, n)| (d.format("%Y-%m-%d").to_string(), n))
                .collect(),
        )
    })
    .await?;
    let rows = days
        .into_iter()
        .filter_map(|(d, n)| Some((d.parse().ok()?, n)))
        .collect();
    Some(bounds.series(&rows))
}

async fn top_pages(
    state: &AppState,
    umami: &Umami<'_>,
    days: u32,
    now: DateTime<Utc>,
) -> Vec<TopPage> {
    let key = format!("admin:umami:pages:{days}");
    let from = now
        .checked_sub_days(Days::new(u64::from(days)))
        .unwrap_or(now);
    cached(state, &key, async {
        let pages = umami.top_pages(from, now, 8).await?;
        Some(
            pages
                .into_iter()
                .map(|(path, views)| TopPage { path, views })
                .collect::<Vec<_>>(),
        )
    })
    .await
    .unwrap_or_default()
}

/// `GET /v1/admin/pending`: the badge of the admin header, cheap enough
/// for every page an admin views.
pub async fn pending(
    State(state): State<AppState>,
    _admin: AdminMember,
) -> Result<Response, ApiError> {
    Ok(ok(stats::pending(&state.db).await?))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GoalBody {
    goal: Option<f64>,
    raised: Option<f64>,
}

/// `GET /v1/admin/settings/donation-goal`: the raw amounts, to fill the
/// form (0 and 0 until set).
pub async fn donation_goal(
    State(state): State<AppState>,
    _admin: AdminMember,
) -> Result<Response, ApiError> {
    Ok(ok(stats::donation_goal(&state.db).await?))
}

/// `PUT /v1/admin/settings/donation-goal`: both amounts, typed in by hand.
/// A goal of 0 takes it off the site.
pub async fn set_donation_goal(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Json(body): Json<GoalBody>,
) -> Result<Response, ApiError> {
    let mut fields = Fields::default();
    let goal = fields.number("goal", body.goal, 0.0, 1_000_000.0);
    let raised = fields.number("raised", body.raised, 0.0, 1_000_000.0);
    fields.finish()?;
    let goal = DonationGoal { goal, raised };
    stats::set_donation_goal(&state.db, goal, admin.id()).await?;
    Ok(ok(goal))
}

#[derive(Debug, Serialize)]
struct GameStats {
    days: u32,
    /// False when the game hub does not answer; the rest of the dashboard
    /// still works.
    available: bool,
    reason: Option<&'static str>,
    stats: Option<crate::game::stats::Stats>,
}

/// `GET /v1/admin/game/stats?days=7|30|90`: what `GET /v1/game/admin/stats`
/// gives a game token with the admin role, for a session. Always 200: a
/// game hub that does not answer is `available: false`.
pub async fn game_stats(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let days = days_param(query.as_deref());
    let Some(live) = state.game.live().await else {
        return Ok(ok(GameStats {
            days,
            available: false,
            reason: Some("unavailable"),
            stats: None,
        }));
    };
    let stats = crate::game::stats::collect(&state.db, Utc::now(), days, &live).await?;
    Ok(ok(GameStats {
        days,
        available: true,
        reason: None,
        stats: Some(stats),
    }))
}

/// `GET /v1/admin/game/leaderboard?subject=&limit=`: the body of
/// `GET /v1/game/leaderboard`, for an admin's session (the site's relay
/// refuses `/v1/game*`). `no-store`, not the public cache.
pub async fn game_leaderboard(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let body = crate::routes::game::leaderboard_body(&state, query.as_deref()).await?;
    Ok(crate::routes::auth::no_store(
        axum::http::StatusCode::OK,
        &body,
    ))
}

/// `GET /v1/admin/languages/stats?days=7|30|90`: what
/// `GET /v1/languages/admin/stats` gives a game token with the admin role.
pub async fn language_stats(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let days = days_param(query.as_deref());
    Ok(ok(
        language_stats::collect(&state.db, Utc::now(), days).await?
    ))
}
