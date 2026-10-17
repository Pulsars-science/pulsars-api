//! Admin statistics of the game (`GET /v1/game/admin/stats`,
//! `game-protocol.md` §9): what is live now, from the hub's snapshot, and
//! what was played, from the `game` schema.
//!
//! Days and hours are UTC. A match counts on the day (and hour) of its
//! `started_at`. Its `game.matches` row is written as it is created (by the
//! runner, a few milliseconds after `match.start`), so the match counts as
//! started while it is still running. Its seats reach `game.match_players`
//! only at the end: today's accounts and guest seats add those of the live
//! matches from the hub's snapshot.
//!
//! The snapshot is taken first, then every query runs in one repeatable-read
//! transaction. A match that ends in between is in both: its accounts are
//! counted once (distinct ids), and the database seats of every live match
//! are left out of the guest count.

use std::collections::HashMap;

use chrono::{DateTime, Days, NaiveDate, NaiveTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::glicko::PROVISIONAL_RD;
use super::hub::LiveSnapshot;
use super::rules::{Mode, Rules, SUBJECT_KEYS};

/// `days` when absent, and its bounds.
pub const DEFAULT_DAYS: u32 = 30;
pub const MAX_DAYS: u32 = 90;

/// Closed season rows shown.
const SEASONS_SHOWN: i64 = 12;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    pub generated_at: String,
    pub live: Live,
    pub today: Today,
    pub daily: Vec<Day>,
    pub subjects: Vec<SubjectCount>,
    pub rules: Vec<RulesCount>,
    pub quality: Quality,
    pub ratings: Ratings,
    pub webhook: Webhook,
    pub seasons: Vec<Season>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Live {
    pub sessions: u64,
    pub guests: u64,
    pub queued: u64,
    pub rooms_open: u64,
    pub matches: ModeCounts,
    pub players_in_matches: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ModeCounts {
    pub duel: u64,
    pub room: u64,
    pub ghost: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Today {
    pub matches_started: i64,
    /// 24 entries, UTC hours 0 to 23.
    pub by_hour: Vec<i64>,
    pub accounts: i64,
    pub guests: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Day {
    /// `YYYY-MM-DD`, UTC.
    pub date: String,
    pub duel: i64,
    pub room: i64,
    pub ghost: i64,
    pub ranked: i64,
    pub void: i64,
    pub forfeit: i64,
    pub accounts: i64,
    pub guests: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubjectCount {
    pub subject: &'static str,
    pub matches: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RulesCount {
    pub rules: ShownRules,
    pub matches: i64,
}

/// Rules as the protocol writes them (`duration`, `target`, `penalty`, in
/// that order); a stored value that does not parse is shown as is.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ShownRules {
    Rules(Rules),
    Stored(Value),
}

impl ShownRules {
    fn of(stored: Value) -> Self {
        match serde_json::from_value::<Rules>(stored.clone()) {
            Ok(rules) => Self::Rules(rules),
            Err(_) => Self::Stored(stored),
        }
    }

    /// The JSON text, to order equal counts.
    fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quality {
    pub avg_match_seconds: Option<f64>,
    pub avg_answers_per_player: Option<f64>,
    pub accuracy: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ratings {
    pub players: i64,
    pub provisional: i64,
    pub by_subject: Vec<SubjectPlayers>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubjectPlayers {
    pub subject: &'static str,
    pub players: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Webhook {
    pub pending: i64,
    pub dead: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Season {
    pub month: String,
    pub subject: String,
    pub ranked_players: i64,
    pub closed_at: String,
}

/// The `days` UTC days ending today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub first: NaiveDate,
    pub today: NaiveDate,
}

impl Period {
    pub fn ending(now: DateTime<Utc>, days: u32) -> Self {
        let today = now.date_naive();
        let back = u64::from(days.clamp(1, MAX_DAYS) - 1);
        Self {
            first: today.checked_sub_days(Days::new(back)).unwrap_or(today),
            today,
        }
    }

    /// Every day of the period, oldest first.
    pub fn days(&self) -> impl Iterator<Item = NaiveDate> + '_ {
        self.first.iter_days().take_while(|d| *d <= self.today)
    }

    pub fn start(&self) -> DateTime<Utc> {
        midnight(self.first)
    }

    /// The end of today (exclusive).
    pub fn end(&self) -> DateTime<Utc> {
        midnight(next_day(self.today))
    }

    pub fn today_start(&self) -> DateTime<Utc> {
        midnight(self.today)
    }
}

fn midnight(day: NaiveDate) -> DateTime<Utc> {
    day.and_time(NaiveTime::MIN).and_utc()
}

fn next_day(day: NaiveDate) -> NaiveDate {
    day.checked_add_days(Days::new(1)).unwrap_or(day)
}

/// What the live matches add to today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveToday {
    /// Account seats of the live matches started today.
    pub accounts: Vec<i64>,
    /// Guest seats of the live matches started today.
    pub guests: i64,
    /// Every live match: its database seats, if any, are left out.
    pub match_ids: Vec<Uuid>,
}

impl LiveToday {
    pub fn of(snapshot: &LiveSnapshot, period: &Period) -> Self {
        let from = period.today_start().timestamp_millis();
        let to = period.end().timestamp_millis();
        let mut today = Self::default();
        for m in &snapshot.matches {
            today.match_ids.push(m.id);
            if (from..to).contains(&m.starts_at) {
                today.accounts.extend(&m.accounts);
                today.guests += i64::try_from(m.guests).unwrap_or(0);
            }
        }
        today
    }
}

/// Matches started on a day, from `game.matches`.
#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct DayRow {
    pub day: NaiveDate,
    pub duel: i64,
    pub room: i64,
    pub ghost: i64,
    pub ranked: i64,
    pub void: i64,
    pub forfeit: i64,
}

/// Seats of the finished matches started on a day.
#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct DayPlayersRow {
    pub day: NaiveDate,
    pub accounts: i64,
    pub guests: i64,
}

#[derive(Debug, Clone, Default, PartialEq, sqlx::FromRow)]
pub struct QualityRow {
    pub avg_match_seconds: Option<f64>,
    pub avg_answers: Option<f64>,
    pub correct: Option<i64>,
    pub answered: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SeasonRow {
    pub month: String,
    pub subject: String,
    pub ranked_players: i64,
    pub closed_at: DateTime<Utc>,
}

/// Everything read from the database.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recorded {
    pub days: Vec<DayRow>,
    pub players: Vec<DayPlayersRow>,
    /// `(UTC hour, matches)` of today.
    pub hours: Vec<(i32, i64)>,
    /// Distinct accounts today, the live ones included.
    pub today_accounts: i64,
    pub subjects: Vec<(String, i64)>,
    pub rules: Vec<(Value, i64)>,
    pub quality: QualityRow,
    pub rated_players: i64,
    pub provisional_players: i64,
    pub rated_by_subject: Vec<(String, i64)>,
    pub webhook_pending: i64,
    pub webhook_dead: i64,
    pub seasons: Vec<SeasonRow>,
}

/// Reads the statistics of `period` in one read-only, repeatable-read
/// transaction (one snapshot for every number).
pub async fn recorded(
    pool: &PgPool,
    period: &Period,
    live: &LiveToday,
) -> Result<Recorded, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let recorded = read(&mut tx, period, live).await?;
    tx.commit().await?;
    Ok(recorded)
}

async fn read(
    tx: &mut Transaction<'_, Postgres>,
    period: &Period,
    live: &LiveToday,
) -> Result<Recorded, sqlx::Error> {
    let (start, end, today) = (period.start(), period.end(), period.today_start());

    let days: Vec<DayRow> = sqlx::query_as(
        "SELECT (started_at AT TIME ZONE 'UTC')::date AS day,
                count(*) FILTER (WHERE mode = 'duel') AS duel,
                count(*) FILTER (WHERE mode = 'room') AS room,
                count(*) FILTER (WHERE mode = 'ghost') AS ghost,
                count(*) FILTER (WHERE ranked) AS ranked,
                count(*) FILTER (WHERE end_reason = 'void') AS void,
                count(*) FILTER (WHERE end_reason = 'forfeit') AS forfeit
           FROM game.matches
          WHERE started_at >= $1 AND started_at < $2
          GROUP BY 1",
    )
    .bind(start)
    .bind(end)
    .fetch_all(&mut **tx)
    .await?;

    let players: Vec<DayPlayersRow> = sqlx::query_as(
        "SELECT (m.started_at AT TIME ZONE 'UTC')::date AS day,
                count(DISTINCT p.user_id) AS accounts,
                count(p.guest_name) AS guests
           FROM game.matches m
           JOIN game.match_players p ON p.match_id = m.id
          WHERE m.started_at >= $1 AND m.started_at < $2 AND m.id <> ALL($3)
          GROUP BY 1",
    )
    .bind(start)
    .bind(end)
    .bind(&live.match_ids)
    .fetch_all(&mut **tx)
    .await?;

    let hours: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT extract(hour FROM started_at AT TIME ZONE 'UTC')::int AS hour, count(*)
           FROM game.matches
          WHERE started_at >= $1 AND started_at < $2
          GROUP BY 1",
    )
    .bind(today)
    .bind(end)
    .fetch_all(&mut **tx)
    .await?;

    let today_accounts: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT u) FROM (
             SELECT p.user_id AS u
               FROM game.matches m
               JOIN game.match_players p ON p.match_id = m.id
              WHERE m.started_at >= $1 AND m.started_at < $2 AND p.user_id IS NOT NULL
             UNION ALL
             SELECT unnest($3::bigint[])
         ) t",
    )
    .bind(today)
    .bind(end)
    .bind(&live.accounts)
    .fetch_one(&mut **tx)
    .await?;

    let subjects: Vec<(String, i64)> = sqlx::query_as(
        "SELECT subject, count(*) FROM game.matches
          WHERE started_at >= $1 AND started_at < $2
          GROUP BY 1",
    )
    .bind(start)
    .bind(end)
    .fetch_all(&mut **tx)
    .await?;

    let rules: Vec<(Json<Value>, i64)> = sqlx::query_as(
        "SELECT rules, count(*) FROM game.matches
          WHERE started_at >= $1 AND started_at < $2
          GROUP BY 1",
    )
    .bind(start)
    .bind(end)
    .fetch_all(&mut **tx)
    .await?;

    // Finished, non-void matches; live seats only (not the ghosts').
    let quality: QualityRow = sqlx::query_as(
        "SELECT (SELECT avg(greatest(extract(epoch FROM ended_at - started_at), 0))::float8
                   FROM game.matches
                  WHERE started_at >= $1 AND started_at < $2
                    AND end_reason IS NOT NULL AND end_reason <> 'void') AS avg_match_seconds,
                avg(p.answered)::float8 AS avg_answers,
                sum(p.correct)::bigint AS correct,
                sum(p.answered)::bigint AS answered
           FROM game.matches m
           JOIN game.match_players p ON p.match_id = m.id
          WHERE m.started_at >= $1 AND m.started_at < $2
            AND m.end_reason IS NOT NULL AND m.end_reason <> 'void'
            AND p.ghost_of IS NULL",
    )
    .bind(start)
    .bind(end)
    .fetch_one(&mut **tx)
    .await?;

    // A player is provisional when every rating they have is.
    let (rated_players, provisional_players): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE best_rd > $1)
           FROM (SELECT min(rd) AS best_rd FROM game.ratings
                  WHERE games > 0 GROUP BY user_id) r",
    )
    .bind(PROVISIONAL_RD)
    .fetch_one(&mut **tx)
    .await?;

    let rated_by_subject: Vec<(String, i64)> =
        sqlx::query_as("SELECT subject, count(*) FROM game.ratings WHERE games > 0 GROUP BY 1")
            .fetch_all(&mut **tx)
            .await?;

    let (webhook_pending, webhook_dead): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM game.webhook_outbox WHERE status = 'pending'),
                (SELECT count(*) FROM game.webhook_outbox WHERE status = 'dead')",
    )
    .fetch_one(&mut **tx)
    .await?;

    let seasons: Vec<SeasonRow> = sqlx::query_as(
        "SELECT month, subject, jsonb_array_length(standings)::bigint AS ranked_players,
                closed_at
           FROM game.seasons
          ORDER BY month DESC, array_position($1::text[], subject)
          LIMIT $2",
    )
    .bind(SUBJECT_KEYS)
    .bind(SEASONS_SHOWN)
    .fetch_all(&mut **tx)
    .await?;

    Ok(Recorded {
        days,
        players,
        hours,
        today_accounts,
        subjects,
        rules: rules.into_iter().map(|(Json(r), n)| (r, n)).collect(),
        quality,
        rated_players,
        provisional_players,
        rated_by_subject,
        webhook_pending,
        webhook_dead,
        seasons,
    })
}

/// The response, from the live snapshot and the database rows: missing
/// days and hours are zeros, every subject key is listed.
pub fn assemble(
    now: DateTime<Utc>,
    period: &Period,
    snapshot: &LiveSnapshot,
    live: &LiveToday,
    recorded: Recorded,
) -> Stats {
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    let mut modes = ModeCounts::default();
    for m in &snapshot.matches {
        match m.mode {
            Mode::Duel => modes.duel += 1,
            Mode::Room => modes.room += 1,
            Mode::Ghost => modes.ghost += 1,
        }
    }

    let mut by_hour = vec![0i64; 24];
    for (hour, n) in &recorded.hours {
        if let Some(slot) = usize::try_from(*hour).ok().and_then(|h| by_hour.get_mut(h)) {
            *slot += n;
        }
    }

    let days: HashMap<NaiveDate, &DayRow> = recorded.days.iter().map(|d| (d.day, d)).collect();
    let players: HashMap<NaiveDate, &DayPlayersRow> =
        recorded.players.iter().map(|p| (p.day, p)).collect();
    let daily: Vec<Day> = period
        .days()
        .map(|date| {
            let row = days.get(&date).copied().cloned().unwrap_or_default();
            let seats = players.get(&date).copied().cloned().unwrap_or_default();
            let (accounts, guests) = if date == period.today {
                (recorded.today_accounts, seats.guests + live.guests)
            } else {
                (seats.accounts, seats.guests)
            };
            Day {
                date: date.format("%Y-%m-%d").to_string(),
                duel: row.duel,
                room: row.room,
                ghost: row.ghost,
                ranked: row.ranked,
                void: row.void,
                forfeit: row.forfeit,
                accounts,
                guests,
            }
        })
        .collect();
    let (today_accounts, today_guests) = daily.last().map_or((0, 0), |d| (d.accounts, d.guests));

    let quality = &recorded.quality;
    let accuracy = match (quality.correct, quality.answered) {
        (Some(correct), Some(answered)) if answered > 0 => {
            Some(round(correct as f64 / answered as f64, 3))
        }
        _ => None,
    };

    let mut rules: Vec<RulesCount> = recorded
        .rules
        .into_iter()
        .map(|(raw, matches)| RulesCount {
            rules: ShownRules::of(raw),
            matches,
        })
        .collect();
    rules.sort_by(|a, b| {
        b.matches
            .cmp(&a.matches)
            .then_with(|| a.rules.text().cmp(&b.rules.text()))
    });

    Stats {
        generated_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        live: Live {
            sessions: count(snapshot.sessions),
            guests: count(snapshot.guests),
            queued: count(snapshot.queued),
            rooms_open: count(snapshot.rooms_open),
            matches: modes,
            players_in_matches: count(snapshot.players_in_matches),
        },
        today: Today {
            matches_started: by_hour.iter().sum(),
            by_hour,
            accounts: today_accounts,
            guests: today_guests,
        },
        daily,
        subjects: per_subject(&recorded.subjects)
            .into_iter()
            .map(|(subject, matches)| SubjectCount { subject, matches })
            .collect(),
        rules,
        quality: Quality {
            avg_match_seconds: quality.avg_match_seconds.map(|s| round(s, 1)),
            avg_answers_per_player: quality.avg_answers.map(|a| round(a, 1)),
            accuracy,
        },
        ratings: Ratings {
            players: recorded.rated_players,
            provisional: recorded.provisional_players,
            by_subject: per_subject(&recorded.rated_by_subject)
                .into_iter()
                .map(|(subject, players)| SubjectPlayers { subject, players })
                .collect(),
        },
        webhook: Webhook {
            pending: recorded.webhook_pending,
            dead: recorded.webhook_dead,
        },
        seasons: recorded
            .seasons
            .into_iter()
            .map(|s| Season {
                month: s.month,
                subject: s.subject,
                ranked_players: s.ranked_players,
                closed_at: s.closed_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            })
            .collect(),
    }
}

/// Every subject key with its count (0 when absent), largest first; equal
/// counts keep the order of [`SUBJECT_KEYS`]. Unknown keys are dropped.
fn per_subject(rows: &[(String, i64)]) -> Vec<(&'static str, i64)> {
    let mut out: Vec<(&'static str, i64)> = SUBJECT_KEYS
        .iter()
        .map(|key| {
            let n = rows
                .iter()
                .filter(|(subject, _)| subject == key)
                .map(|(_, n)| *n)
                .sum();
            (*key, n)
        })
        .collect();
    // Stable: ties stay in key order.
    out.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    out
}

fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    (value * factor).round() / factor
}

/// The statistics of the `days` days ending at `now`.
pub async fn collect(
    pool: &PgPool,
    now: DateTime<Utc>,
    days: u32,
    snapshot: &LiveSnapshot,
) -> Result<Stats, sqlx::Error> {
    let period = Period::ending(now, days);
    let live = LiveToday::of(snapshot, &period);
    let recorded = recorded(pool, &period, &live).await?;
    Ok(assemble(now, &period, snapshot, &live, recorded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::hub::LiveMatchInfo;
    use chrono::TimeZone;
    use serde_json::json;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .unwrap_or_default()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default()
    }

    fn live_match(
        mode: Mode,
        starts_at: DateTime<Utc>,
        accounts: &[i64],
        guests: usize,
    ) -> LiveMatchInfo {
        LiveMatchInfo {
            id: Uuid::now_v7(),
            mode,
            starts_at: starts_at.timestamp_millis(),
            accounts: accounts.to_vec(),
            guests,
        }
    }

    #[test]
    fn the_period_ends_today_and_spans_days_utc_days() {
        let now = at(2026, 10, 17, 13, 40);
        let period = Period::ending(now, 30);
        assert_eq!(period.first, date(2026, 9, 18));
        assert_eq!(period.today, date(2026, 10, 17));
        assert_eq!(period.days().count(), 30);
        assert_eq!(period.start(), at(2026, 9, 18, 0, 0));
        assert_eq!(period.end(), at(2026, 10, 18, 0, 0));
        assert_eq!(period.today_start(), at(2026, 10, 17, 0, 0));

        let one = Period::ending(now, 1);
        assert_eq!(one.days().collect::<Vec<_>>(), vec![date(2026, 10, 17)]);
        // Across a month and a year.
        let jan = Period::ending(at(2027, 1, 2, 0, 0), 4);
        let days: Vec<NaiveDate> = jan.days().collect();
        assert_eq!(days.first(), Some(&date(2026, 12, 30)));
        assert_eq!(days.len(), 4);
    }

    #[test]
    fn missing_days_and_hours_are_zeros() {
        let now = at(2026, 10, 17, 13, 40);
        let period = Period::ending(now, 5);
        let recorded = Recorded {
            days: vec![
                DayRow {
                    day: date(2026, 10, 14),
                    duel: 3,
                    room: 1,
                    ghost: 2,
                    ranked: 2,
                    void: 1,
                    forfeit: 1,
                },
                DayRow {
                    day: date(2026, 10, 17),
                    duel: 4,
                    ..DayRow::default()
                },
            ],
            players: vec![DayPlayersRow {
                day: date(2026, 10, 14),
                accounts: 5,
                guests: 2,
            }],
            hours: vec![(0, 1), (13, 3)],
            ..Recorded::default()
        };
        let stats = assemble(
            now,
            &period,
            &LiveSnapshot::default(),
            &LiveToday::default(),
            recorded,
        );
        assert_eq!(stats.generated_at, "2026-10-17T13:40:00Z");
        let dates: Vec<&str> = stats.daily.iter().map(|d| d.date.as_str()).collect();
        assert_eq!(
            dates,
            [
                "2026-10-13",
                "2026-10-14",
                "2026-10-15",
                "2026-10-16",
                "2026-10-17"
            ]
        );
        assert_eq!(
            stats.daily[0],
            Day {
                date: "2026-10-13".to_owned(),
                ..Day::default()
            }
        );
        assert_eq!(
            stats.daily[1],
            Day {
                date: "2026-10-14".to_owned(),
                duel: 3,
                room: 1,
                ghost: 2,
                ranked: 2,
                void: 1,
                forfeit: 1,
                accounts: 5,
                guests: 2,
            }
        );
        assert_eq!(stats.daily[4].duel, 4);
        assert_eq!(stats.today.by_hour.len(), 24);
        assert_eq!(stats.today.by_hour[0], 1);
        assert_eq!(stats.today.by_hour[13], 3);
        assert_eq!(stats.today.by_hour.iter().filter(|n| **n == 0).count(), 22);
        assert_eq!(stats.today.matches_started, 4);
        assert_eq!(
            stats.quality,
            Quality {
                avg_match_seconds: None,
                avg_answers_per_player: None,
                accuracy: None,
            }
        );
    }

    #[test]
    fn live_matches_count_now_and_add_their_seats_to_today() {
        let now = at(2026, 10, 17, 13, 40);
        let period = Period::ending(now, 3);
        let snapshot = LiveSnapshot {
            sessions: 12,
            guests: 4,
            queued: 2,
            rooms_open: 1,
            players_in_matches: 9,
            matches: vec![
                live_match(Mode::Duel, at(2026, 10, 17, 13, 39), &[7, 8], 0),
                live_match(Mode::Duel, at(2026, 10, 17, 13, 38), &[], 2),
                live_match(Mode::Room, at(2026, 10, 17, 13, 30), &[7, 9], 3),
                // Started yesterday: live now, not today.
                live_match(Mode::Ghost, at(2026, 10, 16, 23, 59), &[10], 0),
            ],
        };
        let live = LiveToday::of(&snapshot, &period);
        assert_eq!(live.accounts, vec![7, 8, 7, 9]);
        assert_eq!(live.guests, 5);
        assert_eq!(live.match_ids.len(), 4);

        let recorded = Recorded {
            players: vec![DayPlayersRow {
                day: date(2026, 10, 17),
                accounts: 3,
                guests: 1,
            }],
            // The database query already counts the live accounts.
            today_accounts: 6,
            ..Recorded::default()
        };
        let stats = assemble(now, &period, &snapshot, &live, recorded);
        assert_eq!(
            serde_json::to_value(&stats.live).unwrap_or_default(),
            json!({
                "sessions": 12, "guests": 4, "queued": 2, "rooms_open": 1,
                "matches": {"duel": 2, "room": 1, "ghost": 1},
                "players_in_matches": 9,
            })
        );
        assert_eq!(stats.today.accounts, 6);
        assert_eq!(stats.today.guests, 6, "1 recorded + 5 live");
        let last = stats.daily.last().cloned().unwrap_or_default();
        assert_eq!((last.accounts, last.guests), (6, 6));
    }

    #[test]
    fn subjects_list_every_key_largest_first() {
        let rows = vec![
            ("physics".to_owned(), 4),
            ("maths".to_owned(), 9),
            ("all".to_owned(), 4),
            ("history".to_owned(), 50),
        ];
        assert_eq!(
            per_subject(&rows),
            vec![
                ("maths", 9),
                ("physics", 4),
                ("all", 4),
                ("chemistry", 0),
                ("computer-science", 0),
            ]
        );
        assert_eq!(per_subject(&[]).len(), 5);
    }

    #[test]
    fn rules_are_canonical_and_sorted_and_quality_is_rounded() {
        let now = at(2026, 10, 17, 13, 40);
        let period = Period::ending(now, 1);
        let recorded = Recorded {
            rules: vec![
                // As jsonb returns it: keys by length.
                (json!({"target": 10, "penalty": true, "duration": null}), 3),
                (
                    json!({"target": null, "penalty": false, "duration": 60}),
                    80,
                ),
                (json!({"odd": true}), 3),
            ],
            quality: QualityRow {
                avg_match_seconds: Some(64.236),
                avg_answers: Some(23.06),
                correct: Some(71),
                answered: Some(97),
            },
            rated_players: 25,
            provisional_players: 18,
            rated_by_subject: vec![("maths".to_owned(), 20)],
            webhook_pending: 2,
            webhook_dead: 1,
            seasons: vec![SeasonRow {
                month: "2026-09".to_owned(),
                subject: "maths".to_owned(),
                ranked_players: 4,
                closed_at: at(2026, 10, 1, 0, 0),
            }],
            ..Recorded::default()
        };
        let stats = assemble(
            now,
            &period,
            &LiveSnapshot::default(),
            &LiveToday::default(),
            recorded,
        );
        let json = serde_json::to_value(&stats).unwrap_or_default();
        assert_eq!(
            json["rules"][0],
            json!({"rules": {"duration": 60, "target": null, "penalty": false}, "matches": 80})
        );
        // Field order too: the client may show the JSON as is.
        assert_eq!(
            serde_json::to_string(&stats.rules[0].rules).unwrap_or_default(),
            r#"{"duration":60,"target":null,"penalty":false}"#
        );
        // Equal counts: by their JSON text.
        assert_eq!(
            json["rules"][1]["rules"],
            json!({"duration": null, "target": 10, "penalty": true})
        );
        assert_eq!(json["rules"][2]["rules"], json!({"odd": true}));
        assert_eq!(
            json["quality"],
            json!({"avg_match_seconds": 64.2, "avg_answers_per_player": 23.1, "accuracy": 0.732})
        );
        assert_eq!(json["ratings"]["players"], 25);
        assert_eq!(json["ratings"]["provisional"], 18);
        assert_eq!(
            json["ratings"]["by_subject"][0],
            json!({"subject": "maths", "players": 20})
        );
        assert_eq!(
            json["ratings"]["by_subject"].as_array().map(Vec::len),
            Some(5)
        );
        assert_eq!(json["webhook"], json!({"pending": 2, "dead": 1}));
        assert_eq!(
            json["seasons"],
            json!([{"month": "2026-09", "subject": "maths", "ranked_players": 4,
                    "closed_at": "2026-10-01T00:00:00Z"}])
        );
    }

    #[test]
    fn accuracy_needs_answers() {
        let now = at(2026, 10, 17, 13, 40);
        let period = Period::ending(now, 1);
        let recorded = Recorded {
            quality: QualityRow {
                avg_match_seconds: Some(12.0),
                avg_answers: Some(0.0),
                correct: Some(0),
                answered: Some(0),
            },
            ..Recorded::default()
        };
        let stats = assemble(
            now,
            &period,
            &LiveSnapshot::default(),
            &LiveToday::default(),
            recorded,
        );
        assert_eq!(stats.quality.accuracy, None);
        assert_eq!(stats.quality.avg_match_seconds, Some(12.0));
    }
}
