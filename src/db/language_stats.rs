//! Admin statistics of the languages section (`GET /v1/languages/admin/stats`,
//! `docs/languages.md` §Statistics), from `langues.events`.
//!
//! Days are UTC; an event counts on the day the API received it. Learners
//! are the browsers' random ids: one person on two devices counts twice.
//! Every query runs in one read-only repeatable-read transaction.

use chrono::{DateTime, Days, NaiveDate, NaiveTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};

/// `days` when absent, and its bounds.
pub const DEFAULT_DAYS: u32 = 30;
pub const MAX_DAYS: u32 = 90;

/// Rows of the "hardest" lists.
const TOP: i64 = 10;
/// A character needs this many writings to rank among the hardest.
const MIN_WRITINGS: i64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub generated_at: String,
    pub days: u32,
    pub totals: Totals,
    pub all_time: AllTime,
    pub daily: Vec<Day>,
    pub levels: Vec<Level>,
    pub missed_words: Vec<MissedWord>,
    pub hardest_characters: Vec<HardCharacter>,
}

/// Over the period.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Totals {
    pub learners: i64,
    pub lessons: i64,
    pub questions: i64,
    pub correct: i64,
    /// `correct / questions`, null without questions.
    #[sqlx(skip)]
    pub accuracy: Option<f64>,
    /// Time spent in lessons, in minutes (one decimal).
    #[sqlx(skip)]
    pub lesson_minutes: f64,
    #[serde(skip)]
    pub duration_ms: i64,
    pub writings: i64,
    /// Characters written from memory without a mistake.
    pub from_memory: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct AllTime {
    pub learners: i64,
    pub lessons: i64,
    pub writings: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Day {
    /// `YYYY-MM-DD`, UTC.
    pub date: String,
    pub learners: i64,
    pub lessons: i64,
    pub writings: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct DayRow {
    offset: i32,
    learners: i64,
    lessons: i64,
    writings: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct Level {
    pub language: String,
    pub level: String,
    pub title: String,
    pub learners: i64,
    pub lessons: i64,
    #[serde(skip)]
    pub questions: i64,
    #[serde(skip)]
    pub correct: i64,
    #[sqlx(skip)]
    pub accuracy: Option<f64>,
    pub average_score: Option<f64>,
    pub best_score: Option<i32>,
    pub writings: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct MissedWord {
    pub id: i64,
    pub text: String,
    pub reading: Option<String>,
    /// In French.
    pub meaning: String,
    pub language: String,
    pub level: String,
    pub misses: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct HardCharacter {
    pub character: String,
    pub writings: i64,
    pub average_mistakes: f64,
    /// Written from memory without a mistake.
    pub from_memory: i64,
}

fn ratio(part: i64, whole: i64) -> Option<f64> {
    (whole > 0).then(|| (part as f64 / whole as f64 * 1000.0).round() / 1000.0)
}

/// Midnight UTC, `days - 1` days before `now`'s day.
fn start_of_period(now: DateTime<Utc>, days: u32) -> (NaiveDate, DateTime<Utc>) {
    let today = now.date_naive();
    let first = today
        .checked_sub_days(Days::new(u64::from(days.saturating_sub(1))))
        .unwrap_or(today);
    (first, first.and_time(NaiveTime::MIN).and_utc())
}

pub async fn collect(pool: &PgPool, now: DateTime<Utc>, days: u32) -> Result<Stats, sqlx::Error> {
    let (first, start) = start_of_period(now, days);
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let stats = read(&mut tx, now, days, first, start).await?;
    tx.commit().await?;
    Ok(stats)
}

async fn read(
    tx: &mut Transaction<'_, Postgres>,
    now: DateTime<Utc>,
    days: u32,
    first: NaiveDate,
    start: DateTime<Utc>,
) -> Result<Stats, sqlx::Error> {
    let mut totals: Totals = sqlx::query_as(
        "SELECT count(DISTINCT learner) AS learners,
                count(*) FILTER (WHERE kind = 'lesson.completed') AS lessons,
                coalesce(sum(questions), 0)::bigint AS questions,
                coalesce(sum(correct), 0)::bigint AS correct,
                coalesce(sum(duration_ms), 0)::bigint AS duration_ms,
                count(*) FILTER (WHERE kind = 'writing.completed') AS writings,
                count(*) FILTER (WHERE mode = 'memory' AND mistakes = 0) AS from_memory
           FROM langues.events
          WHERE received_at >= $1",
    )
    .bind(start)
    .fetch_one(&mut **tx)
    .await?;
    totals.accuracy = ratio(totals.correct, totals.questions);
    totals.lesson_minutes = (totals.duration_ms as f64 / 6000.0).round() / 10.0;

    let all_time: AllTime = sqlx::query_as(
        "SELECT count(DISTINCT learner) AS learners,
                count(*) FILTER (WHERE kind = 'lesson.completed') AS lessons,
                count(*) FILTER (WHERE kind = 'writing.completed') AS writings
           FROM langues.events",
    )
    .fetch_one(&mut **tx)
    .await?;

    let rows: Vec<DayRow> = sqlx::query_as(
        "SELECT d.i AS offset,
                count(DISTINCT e.learner) AS learners,
                count(e.id) FILTER (WHERE e.kind = 'lesson.completed') AS lessons,
                count(e.id) FILTER (WHERE e.kind = 'writing.completed') AS writings
           FROM generate_series(0, $2 - 1) AS d(i)
           LEFT JOIN langues.events e
             ON e.received_at >= $1 + d.i * interval '1 day'
            AND e.received_at < $1 + (d.i + 1) * interval '1 day'
          GROUP BY d.i
          ORDER BY d.i",
    )
    .bind(start)
    .bind(i32::try_from(days).unwrap_or(i32::MAX))
    .fetch_all(&mut **tx)
    .await?;
    let daily = rows
        .into_iter()
        .map(|row| Day {
            date: first
                .checked_add_days(Days::new(u64::try_from(row.offset).unwrap_or(0)))
                .unwrap_or(first)
                .to_string(),
            learners: row.learners,
            lessons: row.lessons,
            writings: row.writings,
        })
        .collect();

    let mut levels: Vec<Level> = sqlx::query_as(
        "SELECT l.language_code AS language, l.slug AS level, l.title->>'fr' AS title,
                count(DISTINCT e.learner) AS learners,
                count(e.id) FILTER (WHERE e.kind = 'lesson.completed') AS lessons,
                coalesce(sum(e.questions), 0)::bigint AS questions,
                coalesce(sum(e.correct), 0)::bigint AS correct,
                round(avg(e.score), 1)::float8 AS average_score,
                max(e.score) AS best_score,
                count(e.id) FILTER (WHERE e.kind = 'writing.completed') AS writings
           FROM langues.levels l
           LEFT JOIN langues.events e
             ON e.language_code = l.language_code AND e.level_slug = l.slug AND e.received_at >= $1
          GROUP BY l.id
          ORDER BY l.language_code, l.position",
    )
    .bind(start)
    .fetch_all(&mut **tx)
    .await?;
    for level in &mut levels {
        level.accuracy = ratio(level.correct, level.questions);
    }

    let missed_words: Vec<MissedWord> = sqlx::query_as(
        "SELECT w.id, w.text, w.reading, w.meaning->>'fr' AS meaning,
                l.language_code AS language, l.slug AS level, count(*) AS misses
           FROM langues.events e
          CROSS JOIN LATERAL unnest(e.missed_words) AS m(word_id)
           JOIN langues.words w ON w.id = m.word_id
           JOIN langues.levels l ON l.id = w.level_id
          WHERE e.kind = 'lesson.completed' AND e.received_at >= $1
          GROUP BY w.id, l.language_code, l.slug
          ORDER BY misses DESC, w.id
          LIMIT $2",
    )
    .bind(start)
    .bind(TOP)
    .fetch_all(&mut **tx)
    .await?;

    let hardest_characters: Vec<HardCharacter> = sqlx::query_as(
        "SELECT character, count(*) AS writings,
                round(avg(mistakes), 2)::float8 AS average_mistakes,
                count(*) FILTER (WHERE mode = 'memory' AND mistakes = 0) AS from_memory
           FROM langues.events
          WHERE kind = 'writing.completed' AND received_at >= $1
          GROUP BY character
         HAVING count(*) >= $3
          ORDER BY avg(mistakes) DESC, count(*) DESC, character
          LIMIT $2",
    )
    .bind(start)
    .bind(TOP)
    .bind(MIN_WRITINGS)
    .fetch_all(&mut **tx)
    .await?;

    Ok(Stats {
        generated_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        days,
        totals,
        all_time,
        daily,
        levels,
        missed_words,
        hardest_characters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_period_ends_today_and_starts_at_midnight_utc() {
        let now = DateTime::parse_from_rfc3339("2026-10-20T15:30:00Z")
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_default();
        let (first, start) = start_of_period(now, 7);
        assert_eq!(first.to_string(), "2026-10-14");
        assert_eq!(start.to_rfc3339(), "2026-10-14T00:00:00+00:00");
        assert_eq!(start_of_period(now, 1).0.to_string(), "2026-10-20");
    }

    #[test]
    fn ratios_are_rounded_and_null_without_a_base() {
        assert_eq!(ratio(2, 3), Some(0.667));
        assert_eq!(ratio(0, 0), None);
    }
}
