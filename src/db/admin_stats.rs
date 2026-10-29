//! The numbers of the admin dashboard (Laravel's `StatistiquesAdmin`).
//!
//! The fixed windows are the same everywhere (24 h, 7 days, 28 days, total):
//! comparing an audience over 7 days with sign-ups over 30 says nothing.
//! Everything else follows the chosen period of N days, which ends today
//! (included, so partial) and starts at midnight UTC N-1 days ago; each
//! series also carries the total of the N days before, for the trend.
//!
//! The old site's `visits` log is not kept: signed-in page
//! views no longer count as activity, and the "signed in in the last 24 h"
//! and "active over 7 days" counters are not kept.

use std::collections::HashMap;

use chrono::{DateTime, Days, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

/// `format!` for a query built from this module's constants only (never
/// from input), which sqlx asks to assert.
macro_rules! sql {
    ($($arg:tt)*) => {
        sqlx::AssertSqlSafe(format!($($arg)*))
    };
}

/// Periods offered by the dashboard, in days.
pub const PERIODS: [u32; 3] = [7, 30, 90];
pub const DEFAULT_PERIOD: u32 = 30;

/// A requested period, brought back to one of [`PERIODS`] (30 otherwise).
pub fn period(days: Option<&str>) -> u32 {
    days.and_then(|d| d.trim().parse::<u32>().ok())
        .filter(|d| PERIODS.contains(d))
        .unwrap_or(DEFAULT_PERIOD)
}

/// Day by day values of a period, their total and that of the period before.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Series {
    pub values: Vec<i64>,
    pub total: i64,
    pub previous: i64,
}

/// The bounds of a period of `days` days ending on `now`'s day.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub days: u32,
    /// Midnight UTC of the period's first day.
    pub start: NaiveDate,
    /// Midnight UTC of the previous period's first day.
    pub previous: NaiveDate,
    pub today: NaiveDate,
}

impl Bounds {
    pub fn new(now: DateTime<Utc>, days: u32) -> Self {
        let today = now.date_naive();
        let start = today - Days::new(u64::from(days.saturating_sub(1)));
        let previous = start - Days::new(u64::from(days));
        Self {
            days,
            start,
            previous,
            today,
        }
    }

    /// The days of the period, oldest first.
    pub fn dates(&self) -> Vec<NaiveDate> {
        (0..self.days)
            .map(|i| self.start + Days::new(u64::from(i)))
            .collect()
    }

    /// Fills a series from daily rows; days without a row are zeros.
    pub fn series(&self, rows: &HashMap<NaiveDate, i64>) -> Series {
        let values: Vec<i64> = self
            .dates()
            .iter()
            .map(|d| rows.get(d).copied().unwrap_or(0))
            .collect();
        let previous = (0..self.days)
            .map(|i| self.previous + Days::new(u64::from(i)))
            .map(|d| rows.get(&d).copied().unwrap_or(0))
            .sum();
        Series {
            total: values.iter().sum(),
            values,
            previous,
        }
    }
}

fn midnight(day: NaiveDate) -> NaiveDateTime {
    day.and_time(chrono::NaiveTime::MIN)
}

/// Daily `aggregate` of `table` over the period and the one before, by the
/// day of `column`. Every argument is a constant of this module.
async fn by_day(
    pool: &PgPool,
    bounds: &Bounds,
    table: &str,
    column: &str,
    aggregate: &str,
    filter: &str,
) -> Result<HashMap<NaiveDate, i64>, sqlx::Error> {
    let rows: Vec<(NaiveDate, i64)> = sqlx::query_as(sql!(
        "SELECT {column}::date, ({aggregate})::bigint FROM {table}
          WHERE {column} >= $1 {filter} GROUP BY 1"
    ))
    .bind(midnight(bounds.previous))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

async fn series(
    pool: &PgPool,
    bounds: &Bounds,
    table: &str,
    column: &str,
    aggregate: &str,
    filter: &str,
) -> Result<Series, sqlx::Error> {
    Ok(bounds.series(&by_day(pool, bounds, table, column, aggregate, filter).await?))
}

#[derive(Debug, Serialize)]
pub struct Windows {
    pub total: i64,
    #[serde(rename = "24h")]
    pub h24: i64,
    #[serde(rename = "7d")]
    pub d7: i64,
    #[serde(rename = "28d")]
    pub d28: i64,
}

#[derive(Debug, Serialize)]
pub struct Accounts {
    pub signups: Windows,
    pub verified: i64,
    pub onboarded: i64,
    pub with_role: i64,
    /// Members with a dated activity over the last 7 days (the dashboard's
    /// `wau`): Laravel counted signed-in page views, which are gone.
    pub active_7d: i64,
}

pub async fn accounts(pool: &PgPool) -> Result<Accounts, sqlx::Error> {
    let (total, h24, d7, d28, verified, onboarded, with_role): (i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT count(*),
                    count(*) FILTER (WHERE created_at >= now() AT TIME ZONE 'UTC' - interval '24 hours'),
                    count(*) FILTER (WHERE created_at >= now() AT TIME ZONE 'UTC' - interval '7 days'),
                    count(*) FILTER (WHERE created_at >= now() AT TIME ZONE 'UTC' - interval '28 days'),
                    count(*) FILTER (WHERE email_verified_at IS NOT NULL),
                    count(*) FILTER (WHERE onboarding_at IS NOT NULL),
                    count(*) FILTER (WHERE EXISTS (SELECT 1 FROM site.role_user r WHERE r.user_id = u.id))
               FROM site.users u",
        )
        .fetch_one(pool)
        .await?;
    Ok(Accounts {
        signups: Windows {
            total,
            h24,
            d7,
            d28,
        },
        verified,
        onboarded,
        with_role,
        active_7d: 0,
    })
}

#[derive(Debug, Serialize)]
pub struct CourseCounts {
    pub total: i64,
    pub published: i64,
    pub drafts: i64,
    pub archived: i64,
}

#[derive(Debug, Serialize)]
pub struct ExamCounts {
    pub total: i64,
    pub published: i64,
    pub drafts: i64,
    pub pending_import: i64,
}

#[derive(Debug, Serialize)]
pub struct Content {
    pub courses: CourseCounts,
    pub lessons: i64,
    pub exercises: i64,
    pub exams: ExamCounts,
    pub exercise_attempts: i64,
}

pub async fn content(pool: &PgPool) -> Result<Content, sqlx::Error> {
    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM contenu.courses),
                (SELECT count(*) FROM contenu.courses WHERE statut = 'publie'),
                (SELECT count(*) FROM contenu.courses WHERE statut = 'brouillon'),
                (SELECT count(*) FROM contenu.courses WHERE statut = 'archive'),
                (SELECT count(*) FROM contenu.lessons),
                (SELECT count(*) FROM contenu.exercises),
                (SELECT count(*) FROM contenu.exams),
                (SELECT count(*) FROM contenu.exams WHERE statut = 'publie'),
                (SELECT count(*) FROM contenu.exams WHERE statut = 'brouillon'),
                (SELECT count(*) FROM contenu.exams WHERE statut = 'attente_import'),
                (SELECT count(*) FROM site.exercise_attempts)",
    )
    .fetch_one(pool)
    .await?;
    Ok(Content {
        courses: CourseCounts {
            total: row.0,
            published: row.1,
            drafts: row.2,
            archived: row.3,
        },
        lessons: row.4,
        exercises: row.5,
        exams: ExamCounts {
            total: row.6,
            published: row.7,
            drafts: row.8,
            pending_import: row.9,
        },
        exercise_attempts: row.10,
    })
}

#[derive(Debug, Serialize)]
pub struct Community {
    pub questions: i64,
    pub answers: i64,
    pub unanswered_questions: i64,
    pub rooms: i64,
    pub room_messages: i64,
    pub questions_7d: i64,
}

/// Questions without a live answer (soft-deleted rows do not count).
const UNANSWERED: &str = "(SELECT count(*) FROM site.questions q
   WHERE q.deleted_at IS NULL
     AND NOT EXISTS (SELECT 1 FROM site.answers a
                      WHERE a.question_id = q.id AND a.deleted_at IS NULL))";

pub async fn community(pool: &PgPool) -> Result<Community, sqlx::Error> {
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(sql!(
        "SELECT (SELECT count(*) FROM site.questions WHERE deleted_at IS NULL),
                (SELECT count(*) FROM site.answers WHERE deleted_at IS NULL),
                {UNANSWERED},
                (SELECT count(*) FROM site.rooms),
                (SELECT count(*) FROM site.room_messages),
                (SELECT count(*) FROM site.questions
                  WHERE deleted_at IS NULL
                    AND created_at >= now() AT TIME ZONE 'UTC' - interval '7 days')"
    ))
    .fetch_one(pool)
    .await?;
    Ok(Community {
        questions: row.0,
        answers: row.1,
        unanswered_questions: row.2,
        rooms: row.3,
        room_messages: row.4,
        questions_7d: row.5,
    })
}

#[derive(Debug, Serialize)]
pub struct AllSeries {
    pub signups: Series,
    /// Distinct devices according to Umami; null without Umami.
    pub visitors: Option<Series>,
    /// Lessons completed.
    pub lessons: Series,
    /// Answers sent to an exercise; `attempts_correct` those judged right,
    /// `attempts_graded` those judged at all (free answers are not).
    pub attempts: Series,
    pub attempts_solved: Series,
    pub attempts_graded: Series,
    pub course_enrollments: Series,
    pub courses_completed: Series,
    pub questions: Series,
    pub answers: Series,
    pub room_messages: Series,
    /// Brains earned (positive movements).
    pub brains: Series,
    /// Finished matches with at least one member.
    pub games: Series,
    /// Duels played to the end (not void), guests' included, by the day
    /// they started.
    pub duels: Series,
    /// First downloads of an exam's PDF by a visitor
    /// (`contenu.telechargements`).
    pub pdf_downloads: Series,
}

pub async fn all_series(
    pool: &PgPool,
    bounds: &Bounds,
    visitors: Option<Series>,
) -> Result<AllSeries, sqlx::Error> {
    let attempts = "site.exercise_attempts";
    Ok(AllSeries {
        signups: series(pool, bounds, "site.users", "created_at", "count(*)", "").await?,
        visitors,
        lessons: series(
            pool,
            bounds,
            "site.lesson_progress",
            "completed_at",
            "count(*)",
            "",
        )
        .await?,
        attempts: series(pool, bounds, attempts, "created_at", "count(*)", "").await?,
        attempts_solved: series(
            pool,
            bounds,
            attempts,
            "created_at",
            "sum(CASE WHEN correct THEN 1 ELSE 0 END)",
            "",
        )
        .await?,
        attempts_graded: series(
            pool,
            bounds,
            attempts,
            "created_at",
            "sum(CASE WHEN correct IS NOT NULL THEN 1 ELSE 0 END)",
            "",
        )
        .await?,
        course_enrollments: series(
            pool,
            bounds,
            "site.enrollments",
            "started_at",
            "count(*)",
            "",
        )
        .await?,
        courses_completed: series(
            pool,
            bounds,
            "site.enrollments",
            "completed_at",
            "count(*)",
            "",
        )
        .await?,
        questions: series(pool, bounds, "site.questions", "created_at", "count(*)", "").await?,
        answers: series(pool, bounds, "site.answers", "created_at", "count(*)", "").await?,
        room_messages: series(
            pool,
            bounds,
            "site.room_messages",
            "created_at",
            "count(*)",
            "",
        )
        .await?,
        brains: series(
            pool,
            bounds,
            "site.cerveaux_mouvements",
            "created_at",
            "sum(montant)",
            "AND montant > 0",
        )
        .await?,
        games: series(
            pool,
            bounds,
            "site.game_results",
            "ended_at",
            "count(DISTINCT match_id)",
            "",
        )
        .await?,
        duels: series(
            pool,
            bounds,
            "game.matches",
            "(started_at AT TIME ZONE 'UTC')",
            "count(*)",
            "AND mode = 'duel' AND end_reason IS NOT NULL AND end_reason <> 'void'",
        )
        .await?,
        pdf_downloads: series(
            pool,
            bounds,
            "contenu.telechargements",
            "created_at",
            "count(*)",
            "AND telechargeable_type = 'exam'",
        )
        .await?,
    })
}

#[derive(Debug, Serialize)]
pub struct Active {
    #[serde(flatten)]
    pub series: Series,
    pub dau: i64,
    pub wau: i64,
    pub mau: i64,
    /// `dau / mau`, to three decimals; null without monthly actives.
    pub stickiness: Option<f64>,
}

/// Members active on a day: a dated trace that day (a lesson completed, an
/// exercise attempt, a match finished, a forum question or answer).
/// `total` and `previous` count distinct members over the whole period (not
/// the sum of the days); `dau`, `wau` and `mau` are over today, 7 and 30
/// days whatever the period.
pub async fn active(pool: &PgPool, bounds: &Bounds) -> Result<Active, sqlx::Error> {
    let since = bounds.previous.min(bounds.today - Days::new(29));
    // UNION, not UNION ALL: a (member, day) pair appears once.
    let pairs = "SELECT user_id, completed_at::date AS day FROM site.lesson_progress WHERE completed_at >= $1
                 UNION SELECT user_id, created_at::date FROM site.exercise_attempts WHERE created_at >= $1
                 UNION SELECT user_id, ended_at::date FROM site.game_results WHERE ended_at >= $1
                 UNION SELECT user_id, created_at::date FROM site.questions
                        WHERE user_id IS NOT NULL AND created_at >= $1
                 UNION SELECT user_id, created_at::date FROM site.answers
                        WHERE user_id IS NOT NULL AND created_at >= $1";
    let rows: Vec<(NaiveDate, i64)> =
        sqlx::query_as(sql!("SELECT day, count(*) FROM ({pairs}) a GROUP BY day"))
            .bind(midnight(since))
            .fetch_all(pool)
            .await?;
    let (period, previous, dau, wau, mau): (i64, i64, i64, i64, i64) = sqlx::query_as(sql!(
        "SELECT count(DISTINCT user_id) FILTER (WHERE day >= $2),
                count(DISTINCT user_id) FILTER (WHERE day >= $3 AND day < $2),
                count(DISTINCT user_id) FILTER (WHERE day >= $4),
                count(DISTINCT user_id) FILTER (WHERE day >= $5),
                count(DISTINCT user_id) FILTER (WHERE day >= $6)
           FROM ({pairs}) a"
    ))
    .bind(midnight(since))
    .bind(bounds.start)
    .bind(bounds.previous)
    .bind(bounds.today)
    .bind(bounds.today - Days::new(6))
    .bind(bounds.today - Days::new(29))
    .fetch_one(pool)
    .await?;
    let daily = bounds.series(&rows.into_iter().collect());
    Ok(Active {
        series: Series {
            values: daily.values,
            total: period,
            previous,
        },
        dau,
        wau,
        mau,
        stickiness: (mau > 0).then(|| (dau as f64 / mau as f64 * 1000.0).round() / 1000.0),
    })
}

#[derive(Debug, Serialize)]
pub struct FunnelStep {
    pub step: &'static str,
    pub n: i64,
}

/// The accounts created in the period, and how many of them went through
/// each welcome step, whatever the order.
pub async fn funnel(pool: &PgPool, bounds: &Bounds) -> Result<Vec<FunnelStep>, sqlx::Error> {
    let (signed_up, verified, onboarded, lesson, exercise): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT count(*),
                    count(*) FILTER (WHERE email_verified_at IS NOT NULL),
                    count(*) FILTER (WHERE onboarding_at IS NOT NULL),
                    count(*) FILTER (WHERE EXISTS (SELECT 1 FROM site.lesson_progress p WHERE p.user_id = u.id)),
                    count(*) FILTER (WHERE EXISTS (SELECT 1 FROM site.exercise_attempts e WHERE e.user_id = u.id))
               FROM site.users u WHERE created_at >= $1",
        )
        .bind(midnight(bounds.start))
        .fetch_one(pool)
        .await?;
    Ok(vec![
        FunnelStep {
            step: "signed_up",
            n: signed_up,
        },
        FunnelStep {
            step: "verified",
            n: verified,
        },
        FunnelStep {
            step: "onboarded",
            n: onboarded,
        },
        FunnelStep {
            step: "first_lesson",
            n: lesson,
        },
        FunnelStep {
            step: "first_exercise",
            n: exercise,
        },
    ])
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopCourse {
    pub title: String,
    pub slug: String,
    pub completed: i64,
    pub enrolled: i64,
}

/// The courses most completed over the period, with their new enrollments.
pub async fn top_courses(pool: &PgPool, bounds: &Bounds) -> Result<Vec<TopCourse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.titre AS title, c.slug,
                count(*) FILTER (WHERE e.completed_at >= $1) AS completed,
                count(*) FILTER (WHERE e.started_at >= $1) AS enrolled
           FROM site.enrollments e
           JOIN contenu.courses c ON c.id = e.course_id
          WHERE e.completed_at >= $1 OR e.started_at >= $1
          GROUP BY c.id, c.titre, c.slug
          ORDER BY completed DESC, enrolled DESC, c.titre
          LIMIT 8",
    )
    .bind(midnight(bounds.start))
    .fetch_all(pool)
    .await
}

#[derive(Debug)]
pub struct ReasonSeries {
    pub reason: String,
    pub series: Series,
}

/// Series by reason, serialized as an object whose keys keep their order.
#[derive(Debug)]
pub struct ByReason(pub Vec<ReasonSeries>);

impl Serialize for ByReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for entry in &self.0 {
            map.serialize_entry(&entry.reason, &entry.series)?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
pub struct Brains {
    /// Brains earned over the period by reason, the most rewarding first.
    pub by_reason: ByReason,
    pub in_circulation: i64,
    pub holders: i64,
}

pub async fn brains(pool: &PgPool, bounds: &Bounds) -> Result<Brains, sqlx::Error> {
    let rows: Vec<(NaiveDate, String, i64)> = sqlx::query_as(
        "SELECT created_at::date, raison::text, sum(montant)::bigint
           FROM site.cerveaux_mouvements
          WHERE created_at >= $1 AND montant > 0
          GROUP BY 1, 2",
    )
    .bind(midnight(bounds.previous))
    .fetch_all(pool)
    .await?;
    let mut by_reason: HashMap<String, HashMap<NaiveDate, i64>> = HashMap::new();
    for (day, reason, n) in rows {
        by_reason.entry(reason).or_default().insert(day, n);
    }
    let mut by_reason: Vec<ReasonSeries> = by_reason
        .into_iter()
        .map(|(reason, days)| ReasonSeries {
            reason,
            series: bounds.series(&days),
        })
        .collect();
    // The order of the colours: it must not change from one day to the next.
    by_reason.sort_by(|a, b| {
        b.series
            .total
            .cmp(&a.series.total)
            .then_with(|| a.reason.cmp(&b.reason))
    });
    let (in_circulation, holders): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(sum(montant), 0)::bigint, count(DISTINCT user_id) FROM site.cerveaux_mouvements",
    )
    .fetch_one(pool)
    .await?;
    Ok(Brains {
        by_reason: ByReason(by_reason),
        in_circulation,
        holders,
    })
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopBrains {
    pub pseudo: String,
    pub balance: i64,
}

/// The richest members in brains (balance, ever).
pub async fn top_brains(pool: &PgPool) -> Result<Vec<TopBrains>, sqlx::Error> {
    sqlx::query_as(
        "SELECT u.pseudo, sum(m.montant)::bigint AS balance
           FROM site.cerveaux_mouvements m JOIN site.users u ON u.id = m.user_id
          GROUP BY u.id, u.pseudo
          ORDER BY balance DESC, u.pseudo
          LIMIT 8",
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Serialize)]
pub struct ByMode {
    pub duel: Series,
    pub room: Series,
    pub ghost: Series,
    /// Modes the dashboard does not know; absent when there were none.
    #[serde(rename = "autre", skip_serializing_if = "Option::is_none")]
    pub other: Option<Series>,
}

#[derive(Debug, Serialize)]
pub struct Outcomes {
    pub win: i64,
    pub loss: i64,
    pub draw: i64,
    pub forfeit: i64,
}

#[derive(Debug, Serialize)]
pub struct Games {
    pub by_mode: ByMode,
    /// Ranked matches over the period.
    pub ranked: i64,
    /// Members' results (one per member and match).
    pub outcomes: Outcomes,
    /// Distinct members who finished a match.
    pub players: i64,
}

/// The finished matches of `site.game_results` (members only: matches
/// between guests are not there).
pub async fn games(pool: &PgPool, bounds: &Bounds) -> Result<Games, sqlx::Error> {
    let rows: Vec<(NaiveDate, String, i64)> = sqlx::query_as(
        "SELECT ended_at::date,
                CASE mode WHEN 'flash-duel' THEN 'duel' WHEN 'flash-room' THEN 'room'
                          WHEN 'flash-ghost' THEN 'ghost' ELSE 'other' END,
                count(DISTINCT match_id)::bigint
           FROM site.game_results WHERE ended_at >= $1 GROUP BY 1, 2",
    )
    .bind(midnight(bounds.previous))
    .fetch_all(pool)
    .await?;
    let mut modes: HashMap<String, HashMap<NaiveDate, i64>> = HashMap::new();
    for (day, mode, n) in rows {
        *modes.entry(mode).or_default().entry(day).or_default() += n;
    }
    let mut take = |mode: &str| bounds.series(&modes.remove(mode).unwrap_or_default());
    let other = take("other");
    let by_mode = ByMode {
        duel: take("duel"),
        room: take("room"),
        ghost: take("ghost"),
        other: (other.total > 0 || other.previous > 0).then_some(other),
    };
    let (ranked, win, loss, draw, forfeit, players): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT count(DISTINCT match_id) FILTER (WHERE ranked),
                    count(*) FILTER (WHERE outcome = 'win'),
                    count(*) FILTER (WHERE outcome = 'loss'),
                    count(*) FILTER (WHERE outcome = 'draw'),
                    count(*) FILTER (WHERE outcome = 'forfeit'),
                    count(DISTINCT user_id)
               FROM site.game_results WHERE ended_at >= $1",
        )
        .bind(midnight(bounds.start))
        .fetch_one(pool)
        .await?;
    Ok(Games {
        by_mode,
        ranked,
        outcomes: Outcomes {
            win,
            loss,
            draw,
            forfeit,
        },
        players,
    })
}

#[derive(Debug, Serialize)]
pub struct LatestSignup {
    pub pseudo: String,
    pub verified: bool,
    /// When the account was created.
    pub date: Option<String>,
}

pub async fn latest_signups(pool: &PgPool) -> Result<Vec<LatestSignup>, sqlx::Error> {
    let rows: Vec<(String, bool, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT pseudo, email_verified_at IS NOT NULL, created_at AT TIME ZONE 'UTC'
           FROM site.users ORDER BY created_at DESC NULLS LAST, id DESC LIMIT 8",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(pseudo, verified, at)| LatestSignup {
            pseudo,
            verified,
            date: at.map(crate::routes::courses::timestamp),
        })
        .collect())
}

/// What waits for a decision: the first reason to open the dashboard.
#[derive(Debug, Serialize)]
pub struct ToDo {
    pub reports: i64,
    pub suggestions: i64,
    pub resource_requests: i64,
    pub draft_courses: i64,
    pub pending_exams: i64,
    pub draft_exams: i64,
    pub unanswered_questions: i64,
    pub unverified_accounts: i64,
}

/// Open reports, suggestions and resource requests (`aTraiter()` scopes).
const OPEN_REPORTS: &str =
    "(SELECT count(*) FROM site.signalements WHERE statut IN ('nouveau', 'retenu'))";
const OPEN_SUGGESTIONS: &str =
    "(SELECT count(*) FROM site.suggestions WHERE statut IN ('nouvelle', 'retenue'))";
const OPEN_REQUESTS: &str =
    "(SELECT count(*) FROM site.demandes_ressources WHERE statut = 'en_attente')";

pub async fn to_do(pool: &PgPool) -> Result<ToDo, sqlx::Error> {
    let row: (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(sql!(
        "SELECT {OPEN_REPORTS}, {OPEN_SUGGESTIONS}, {OPEN_REQUESTS},
                (SELECT count(*) FROM contenu.courses WHERE statut = 'brouillon'),
                (SELECT count(*) FROM contenu.exams WHERE statut = 'attente_import'),
                (SELECT count(*) FROM contenu.exams WHERE statut = 'brouillon'),
                {UNANSWERED},
                (SELECT count(*) FROM site.users WHERE email_verified_at IS NULL)"
    ))
    .fetch_one(pool)
    .await?;
    Ok(ToDo {
        reports: row.0,
        suggestions: row.1,
        resource_requests: row.2,
        draft_courses: row.3,
        pending_exams: row.4,
        draft_exams: row.5,
        unanswered_questions: row.6,
        unverified_accounts: row.7,
    })
}

/// What someone sent and waits for an answer, for the badge of the admin
/// header (`enAttente()`): three queues where a person waits behind.
#[derive(Debug, Serialize)]
pub struct Pending {
    pub resource_requests: i64,
    pub reports: i64,
    pub suggestions: i64,
    pub total: i64,
}

pub async fn pending(pool: &PgPool) -> Result<Pending, sqlx::Error> {
    let (resource_requests, reports, suggestions): (i64, i64, i64) = sqlx::query_as(sql!(
        "SELECT {OPEN_REQUESTS}, {OPEN_REPORTS}, {OPEN_SUGGESTIONS}"
    ))
    .fetch_one(pool)
    .await?;
    Ok(Pending {
        resource_requests,
        reports,
        suggestions,
        total: resource_requests + reports + suggestions,
    })
}

/// A p95 latency by day and over the periods, in milliseconds; null
/// without any request.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Percentile {
    pub values: Vec<Option<i64>>,
    pub total: Option<i64>,
    pub previous: Option<i64>,
}

/// The API's HTTP traffic, from `site.http_daily` (`crate::http_daily`).
#[derive(Debug, Serialize)]
pub struct Http {
    /// False when nothing was recorded over the period nor the one before.
    pub available: bool,
    /// The first day recorded, `YYYY-MM-DD`.
    pub since: Option<String>,
    pub requests: Series,
    pub errors_5xx: Series,
    /// Read from the summed latency buckets of the days (`http_daily::p95`).
    pub p95_ms: Percentile,
}

pub async fn http(pool: &PgPool, bounds: &Bounds) -> Result<Http, sqlx::Error> {
    let rows: Vec<(NaiveDate, i64, i64, Vec<i64>)> = sqlx::query_as(
        "SELECT day, requests, errors_5xx, buckets FROM site.http_daily
          WHERE day >= $1 AND day <= $2",
    )
    .bind(bounds.previous)
    .bind(bounds.today)
    .fetch_all(pool)
    .await?;
    let since: Option<NaiveDate> = sqlx::query_scalar("SELECT min(day) FROM site.http_daily")
        .fetch_one(pool)
        .await?;
    let requests = rows.iter().map(|r| (r.0, r.1)).collect();
    let errors = rows.iter().map(|r| (r.0, r.2)).collect();
    let buckets: HashMap<NaiveDate, &[i64]> = rows.iter().map(|r| (r.0, r.3.as_slice())).collect();
    // Element by element, whatever the length of each row.
    let summed = |days: &mut dyn Iterator<Item = NaiveDate>| {
        let mut sum: Vec<i64> = Vec::new();
        for day in days {
            for (i, n) in buckets
                .get(&day)
                .copied()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                if sum.len() <= i {
                    sum.resize(i + 1, 0);
                }
                sum[i] += n;
            }
        }
        crate::http_daily::p95(&sum)
    };
    let dates = bounds.dates();
    let p95_ms = Percentile {
        values: dates
            .iter()
            .map(|d| summed(&mut std::iter::once(*d)))
            .collect(),
        total: summed(&mut dates.iter().copied()),
        previous: summed(&mut (0..bounds.days).map(|i| bounds.previous + Days::new(u64::from(i)))),
    };
    Ok(Http {
        available: !rows.is_empty(),
        since: since.map(|d| d.format("%Y-%m-%d").to_string()),
        requests: bounds.series(&requests),
        errors_5xx: bounds.series(&errors),
        p95_ms,
    })
}

/// What a page view is of, in `site.content_views`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Course,
    Exam,
}

impl ViewKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Course => "course",
            Self::Exam => "exam",
        }
    }
}

/// One more view today (UTC) of a course or an exam, in the transaction
/// that increments its `vues`.
pub async fn record_view(
    conn: &mut sqlx::PgConnection,
    kind: ViewKind,
    content_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.content_views AS v (kind, content_id, day, views)
         VALUES ($1, $2, (now() AT TIME ZONE 'UTC')::date, 1)
         ON CONFLICT (kind, content_id, day) DO UPDATE SET views = v.views + 1",
    )
    .bind(kind.as_str())
    .bind(content_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Published content of a subject.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CatalogSubject {
    pub subject_slug: String,
    pub subject_name: String,
    pub courses: i64,
    pub exercises: i64,
    /// Published exams with a statement PDF.
    pub pdfs: i64,
    /// Downloads of those exams' PDF over the period.
    pub downloads: i64,
}

/// Published content by subject: a course's subject is the first part of
/// its theme, an exam's its `matiere`, both slugified so they meet. The
/// name is the courses' when a course has the subject, the exam's
/// otherwise. Subjects with nothing are left out.
pub async fn catalog(pool: &PgPool, bounds: &Bounds) -> Result<Vec<CatalogSubject>, sqlx::Error> {
    sqlx::query_as(concat!(
        "WITH courses AS (SELECT c.id, c.subject_slug, c.subject_name FROM ",
        published_courses!(),
        " c),
         exams AS (SELECT x.id, contenu.slugify(x.subject) AS subject_slug, x.subject, x.has_pdf FROM ",
        published_exams!(),
        " x),
         by_course AS (SELECT subject_slug, min(subject_name) AS subject_name, count(*) AS courses
                         FROM courses GROUP BY 1),
         by_exercise AS (SELECT c.subject_slug, count(*) AS exercises FROM ",
        published_exercises!(),
        " e JOIN courses c ON c.id = e.course_id GROUP BY 1),
         by_exam AS (SELECT subject_slug, min(subject) AS subject_name,
                            count(*) FILTER (WHERE has_pdf) AS pdfs
                       FROM exams GROUP BY 1),
         by_download AS (SELECT x.subject_slug, count(*) AS downloads
                           FROM contenu.telechargements t JOIN exams x ON x.id = t.telechargeable_id
                          WHERE t.telechargeable_type = 'exam' AND t.created_at >= $1
                          GROUP BY 1)
         SELECT s.subject_slug,
                coalesce(c.subject_name, x.subject_name, s.subject_slug) AS subject_name,
                coalesce(c.courses, 0)::bigint AS courses,
                coalesce(e.exercises, 0)::bigint AS exercises,
                coalesce(x.pdfs, 0)::bigint AS pdfs,
                coalesce(d.downloads, 0)::bigint AS downloads
           FROM (SELECT subject_slug FROM by_course UNION SELECT subject_slug FROM by_exam) s
           LEFT JOIN by_course c USING (subject_slug)
           LEFT JOIN by_exercise e USING (subject_slug)
           LEFT JOIN by_exam x USING (subject_slug)
           LEFT JOIN by_download d USING (subject_slug)
          WHERE coalesce(c.courses, 0) + coalesce(e.exercises, 0)
                + coalesce(x.pdfs, 0) + coalesce(d.downloads, 0) > 0
          ORDER BY downloads DESC, subject_name, s.subject_slug"
    ))
    .bind(midnight(bounds.start))
    .fetch_all(pool)
    .await
}

/// A most viewed course or exam.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopContent {
    /// `course` or `exam`.
    pub kind: String,
    pub slug: String,
    pub title: String,
    pub subject_name: Option<String>,
    pub views: i64,
    /// Its views over the period before.
    pub previous: i64,
}

/// The ten courses and exams most viewed over the period
/// (`site.content_views`), still published.
pub async fn top_content(pool: &PgPool, bounds: &Bounds) -> Result<Vec<TopContent>, sqlx::Error> {
    sqlx::query_as(concat!(
        "WITH v AS (
             SELECT kind, content_id,
                    coalesce(sum(views) FILTER (WHERE day >= $1), 0)::bigint AS views,
                    coalesce(sum(views) FILTER (WHERE day < $1), 0)::bigint AS previous
               FROM site.content_views WHERE day >= $2 AND day <= $3 GROUP BY 1, 2),
         named AS (
             SELECT 'course' AS kind, c.id, c.slug, c.title, c.subject_name FROM ",
        published_courses!(),
        " c
             UNION ALL
             SELECT 'exam', x.id, x.slug, x.title, x.subject FROM ",
        published_exams!(),
        " x)
         SELECT v.kind, n.slug::text AS slug, n.title::text AS title,
                n.subject_name::text AS subject_name, v.views, v.previous
           FROM v JOIN named n ON n.kind = v.kind AND n.id = v.content_id
          WHERE v.views > 0
          ORDER BY v.views DESC, v.kind, n.slug
          LIMIT 10"
    ))
    .bind(bounds.start)
    .bind(bounds.previous)
    .bind(bounds.today)
    .fetch_all(pool)
    .await
}

/// The `objectif_dons` setting of `site.reglages`.
pub const DONATION_GOAL_KEY: &str = "objectif_dons";

/// This month's donation goal, typed in by hand (no payment goes through
/// the site): the amounts, 0 and 0 until an admin sets them.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct DonationGoal {
    pub goal: f64,
    pub raised: f64,
}

pub async fn donation_goal(pool: &PgPool) -> Result<DonationGoal, sqlx::Error> {
    let value: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT valeur::jsonb FROM site.reglages WHERE cle = $1")
            .bind(DONATION_GOAL_KEY)
            .fetch_optional(pool)
            .await?;
    let read = |key: &str| {
        value
            .as_ref()
            .and_then(|v| v.get(key))
            .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(0.0)
    };
    Ok(DonationGoal {
        goal: read("montant"),
        raised: read("collecte"),
    })
}

/// Writes the goal, as Laravel's `Reglage::ecrire()` (`{"montant", "collecte"}`).
pub async fn set_donation_goal(
    pool: &PgPool,
    goal: DonationGoal,
    by: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.reglages (cle, valeur, modifie_par, created_at, updated_at)
         VALUES ($1, json_build_object('montant', $2::float8, 'collecte', $3::float8), $4,
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (cle) DO UPDATE
            SET valeur = excluded.valeur, modifie_par = excluded.modifie_par,
                updated_at = excluded.updated_at",
    )
    .bind(DONATION_GOAL_KEY)
    .bind(goal.goal)
    .bind(goal.raised)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_periods_fall_back_to_thirty_days() {
        assert_eq!(period(Some("7")), 7);
        assert_eq!(period(Some("90")), 90);
        assert_eq!(period(Some("12")), 30);
        assert_eq!(period(Some("x")), 30);
        assert_eq!(period(None), 30);
    }

    #[test]
    fn a_series_is_filled_and_compared_with_the_period_before() {
        let now = "2026-10-28T15:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let bounds = Bounds::new(now, 7);
        let day = |d: &str| NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap();
        assert_eq!(bounds.start, day("2026-10-22"));
        assert_eq!(bounds.previous, day("2026-10-15"));
        let rows = HashMap::from([
            (day("2026-10-28"), 3),
            (day("2026-10-22"), 1),
            (day("2026-10-21"), 5),
            (day("2026-10-14"), 100),
        ]);
        let series = bounds.series(&rows);
        assert_eq!(series.values, [1, 0, 0, 0, 0, 0, 3]);
        assert_eq!(series.total, 4);
        assert_eq!(series.previous, 5);
    }
}
