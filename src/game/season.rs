//! Monthly duel seasons (`docs/contract/brains.md` §3).
//!
//! At the start of a UTC month the API closes the previous one, for each
//! subject key, in one transaction per subject:
//!
//! 1. the **standings**: accounts with at least [`MIN_RANKED_MATCHES`]
//!    ranked matches ended in the month in that subject, by current rating
//!    (desc), then more ranked matches in the month, then lower `user_id`;
//! 2. the **decay** of every rating of the subject ([`decay`]);
//! 3. the **rewards** of the standings, in brains ([`crate::db::brains`]);
//! 4. the `game.seasons` row, whose primary key `(month, subject)` makes the
//!    close happen once.
//!
//! A transaction-scoped advisory lock on `(month, subject)` serializes two
//! instances closing the same month: the second one waits, then finds the
//! row and does nothing. [`run_closer`] checks at startup and every
//! 10 minutes; "now" is a parameter of [`close_due`] so tests choose it.

use std::time::Duration;

use chrono::{DateTime, Datelike as _, NaiveDate, Utc};
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{Connection as _, PgConnection, PgPool};

use super::rules::{SUBJECT_KEYS, SubjectKey};
use crate::db::brains;
use crate::telemetry::names;

/// Ranked matches in the month needed to appear in the standings.
pub const MIN_RANKED_MATCHES: i64 = 5;

/// Ratings are pulled towards this value at every close.
pub const DECAY_CENTRE: f64 = 1500.0;
/// Share of the distance to [`DECAY_CENTRE`] a rating keeps.
pub const DECAY_FACTOR: f64 = 0.8;
/// RD added at every close, so the rating becomes a bit provisional again.
pub const DECAY_RD_INCREASE: f64 = 50.0;
/// RD never exceeds the one of a new player.
pub const MAX_RD: f64 = 350.0;

/// How often [`run_closer`] checks whether the previous month is closed.
pub const CHECK_EVERY: Duration = Duration::from_secs(10 * 60);
/// Wait after a failed check (database down, migrations not applied yet).
const RETRY_AFTER_ERROR: Duration = Duration::from_secs(60);

/// A UTC calendar month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
    year: i32,
    month: u32,
}

impl Month {
    /// `None` unless `month` is in 1..=12 and the year is one chrono handles.
    pub fn new(year: i32, month: u32) -> Option<Self> {
        NaiveDate::from_ymd_opt(year, month, 1).map(|_| Self { year, month })
    }

    /// The month `at` falls in, in UTC.
    pub fn of(at: DateTime<Utc>) -> Self {
        Self {
            year: at.year(),
            month: at.month(),
        }
    }

    pub fn previous(self) -> Self {
        if self.month == 1 {
            Self {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Self {
                year: self.year,
                month: self.month - 1,
            }
        }
    }

    pub fn next(self) -> Self {
        if self.month == 12 {
            Self {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Self {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    /// `YYYY-MM`, the `season` of the event and the `month` of `game.seasons`.
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    /// First instant of the month (inclusive).
    pub fn start(self) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map_or(DateTime::<Utc>::MIN_UTC, |d| d.and_utc())
    }

    /// First instant of the next month (exclusive end).
    pub fn end(self) -> DateTime<Utc> {
        self.next().start()
    }
}

/// A rating and RD after a season close.
pub fn decay(rating: f64, rd: f64) -> (f64, f64) {
    (
        DECAY_CENTRE + DECAY_FACTOR * (rating - DECAY_CENTRE),
        (rd + DECAY_RD_INCREASE).min(MAX_RD),
    )
}

/// A line of the standings, as kept in `game.seasons.standings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Standing {
    pub user_id: String,
    pub rank: i64,
    /// The rating before the decay, rounded.
    pub rating: i64,
    pub ranked_matches: i64,
}

/// What closing a month did for a subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Close {
    /// Closed now. `rewarded` is the number of accounts credited (an
    /// account deleted since the month ended is not).
    Closed {
        standings: Vec<Standing>,
        rewarded: usize,
    },
    /// Already closed earlier (or by another instance): nothing changed.
    AlreadyClosed,
}

/// Closes `month` for `subject` in one transaction on `conn` (a savepoint
/// when `conn` is already in a transaction, which is how tests roll it
/// back). Committed only when it closed the month.
pub async fn close_month(
    conn: &mut PgConnection,
    month: Month,
    subject: SubjectKey,
    now: DateTime<Utc>,
) -> Result<Close, sqlx::Error> {
    let mut tx = conn.begin().await?;
    let month_key = month.key();

    // Another instance closing the same month waits here until it commits.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("pulsars-api:season:{month_key}:{}", subject.key()))
        .execute(&mut *tx)
        .await?;
    let closed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM game.seasons WHERE month = $1 AND subject = $2)",
    )
    .bind(&month_key)
    .bind(subject.key())
    .fetch_one(&mut *tx)
    .await?;
    if closed {
        tx.rollback().await?;
        return Ok(Close::AlreadyClosed);
    }

    // 1. Standings, from the ratings as they are before the decay.
    let rows: Vec<(i64, f64, i64)> = sqlx::query_as(
        "WITH played AS (
             SELECT p.user_id, count(*) AS ranked_matches
               FROM game.matches m
               JOIN game.match_players p ON p.match_id = m.id
              WHERE m.subject = $1 AND m.ranked
                AND m.end_reason IS NOT NULL AND m.end_reason <> 'void'
                AND m.ended_at >= $2 AND m.ended_at < $3
                AND p.user_id IS NOT NULL
              GROUP BY p.user_id
             HAVING count(*) >= $4
         )
         SELECT pl.user_id, r.rating, pl.ranked_matches
           FROM played pl
           JOIN game.ratings r ON r.user_id = pl.user_id AND r.subject = $1
          ORDER BY r.rating DESC, pl.ranked_matches DESC, pl.user_id",
    )
    .bind(subject.key())
    .bind(month.start())
    .bind(month.end())
    .bind(MIN_RANKED_MATCHES)
    .fetch_all(&mut *tx)
    .await?;
    let standings: Vec<Standing> = rows
        .into_iter()
        .zip(1..)
        .map(|((user_id, rating, ranked_matches), rank)| Standing {
            user_id: user_id.to_string(),
            rank,
            rating: rating.round() as i64,
            ranked_matches,
        })
        .collect();

    // 2. Decay. Rows are locked in `user_id` order first, the order a match
    // end locks them in, so the two cannot deadlock.
    sqlx::query("SELECT 1 FROM game.ratings WHERE subject = $1 ORDER BY user_id FOR UPDATE")
        .bind(subject.key())
        .execute(&mut *tx)
        .await?;
    let decayed = sqlx::query(
        "UPDATE game.ratings
            SET rating = $2 + $3 * (rating - $2),
                rd = least($5, rd + $4),
                updated_at = $6
          WHERE subject = $1",
    )
    .bind(subject.key())
    .bind(DECAY_CENTRE)
    .bind(DECAY_FACTOR)
    .bind(DECAY_RD_INCREASE)
    .bind(MAX_RD)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // 3. Rewards, as Laravel's `Cerveaux::crediterSaison`, once per member
    // and subject (`saison:<month>:<subject>`). In `user_id` order, the
    // order a match end locks accounts in.
    let mut by_account: Vec<(i64, i64)> = standings
        .iter()
        .filter_map(|s| s.user_id.parse().ok().map(|id| (id, s.rank)))
        .collect();
    by_account.sort_unstable();
    let mut rewarded = 0;
    for (user_id, rank) in by_account {
        if brains::credit_season(&mut tx, user_id, &month_key, subject.key(), rank, now).await? > 0
        {
            rewarded += 1;
        }
    }

    // 4. The month is closed. The advisory lock makes a conflict impossible;
    // should one happen anyway, everything above is rolled back.
    let inserted = sqlx::query(
        "INSERT INTO game.seasons (month, subject, closed_at, standings)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (month, subject) DO NOTHING",
    )
    .bind(&month_key)
    .bind(subject.key())
    .bind(now)
    .bind(Json(&standings))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        tx.rollback().await?;
        return Ok(Close::AlreadyClosed);
    }
    tx.commit().await?;

    metrics::counter!(names::GAME_SEASONS_CLOSED_TOTAL, "subject" => subject.key()).increment(1);
    tracing::info!(
        season = %month_key,
        subject = subject.key(),
        ranked_players = standings.len(),
        ratings_decayed = decayed,
        rewarded,
        "duel season closed"
    );
    Ok(Close::Closed {
        standings,
        rewarded,
    })
}

/// Closes the month before `now`'s, for every subject key that is not
/// closed yet. Each subject is its own transaction.
pub async fn close_due(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<Vec<(SubjectKey, Close)>, sqlx::Error> {
    let month = Month::of(now).previous();
    let mut done = Vec::with_capacity(SUBJECT_KEYS.len());
    for subject in SUBJECT_KEYS.iter().filter_map(|k| SubjectKey::parse(k)) {
        let close = close_month(conn, month, subject, now).await?;
        done.push((subject, close));
    }
    Ok(done)
}

/// Checks now and then every [`CHECK_EVERY`] whether the previous month is
/// closed, and closes it. Never returns: spawn it, abort it to stop.
pub async fn run_closer(pool: PgPool) {
    loop {
        let result = match pool.acquire().await {
            Ok(mut conn) => close_due(&mut conn, Utc::now()).await,
            Err(error) => Err(error),
        };
        let wait = match result {
            Ok(_) => CHECK_EVERY,
            Err(error) => {
                tracing::warn!(error = %error, retry_secs = RETRY_AFTER_ERROR.as_secs(),
                    "cannot close the previous duel season");
                RETRY_AFTER_ERROR
            }
        };
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_default()
    }

    #[test]
    fn months_are_utc_calendar_months() {
        assert_eq!(
            Month::of(at("2026-11-01T00:00:00Z")).previous().key(),
            "2026-10"
        );
        assert_eq!(
            Month::of(at("2026-10-31T23:59:59Z")).previous().key(),
            "2026-09"
        );
        // 00:30 in Paris on 1 November is still October in UTC.
        assert_eq!(Month::of(at("2026-11-01T00:30:00+01:00")).key(), "2026-10");
        assert_eq!(
            Month::of(at("2027-01-01T00:00:00Z")).previous().key(),
            "2026-12"
        );
        assert_eq!(
            Month::of(at("2026-12-15T12:00:00Z")).next().key(),
            "2027-01"
        );

        let feb = Month::new(2028, 2).expect("valid month");
        assert_eq!(feb.start(), at("2028-02-01T00:00:00Z"));
        assert_eq!(feb.end(), at("2028-03-01T00:00:00Z"));
        assert_eq!(
            Month::new(2026, 12).map(Month::end),
            Some(at("2027-01-01T00:00:00Z"))
        );
        assert_eq!(Month::new(2026, 0), None);
        assert_eq!(Month::new(2026, 13), None);
    }

    #[test]
    fn decay_pulls_towards_1500_and_widens_rd() {
        assert_eq!(decay(1700.0, 60.0), (1660.0, 110.0));
        assert_eq!(decay(1300.0, 100.0), (1340.0, 150.0));
        assert_eq!(decay(1500.0, 320.0), (1500.0, 350.0));
        assert_eq!(decay(1500.0, 350.0), (1500.0, 350.0));
        // Repeated closes converge on 1500.
        let mut r = 2100.0;
        for _ in 0..30 {
            r = decay(r, 50.0).0;
        }
        assert!((r - 1500.0).abs() < 1.0, "{r}");
    }
}
