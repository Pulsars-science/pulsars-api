//! The `game` schema (`game-protocol.md` §8, `migrations/`): matches,
//! runs, ratings, ghosts and the webhook outbox.

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::engine::{AnswerLog, Final, Match, SeatKind};
use super::glicko::{self, Rating};
use super::protocol::EndReason;
use super::ranking::Outcome;
use super::rules::{QueueKey, SubjectKey};

/// Runs the migrations of `migrations/` (the `game` schema only).
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub fn timestamp(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

fn rfc3339(ms: i64) -> String {
    timestamp(ms).to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The `game.matches` row of a match that starts.
#[derive(Debug, Clone)]
pub struct StartRow {
    pub id: Uuid,
    mode: &'static str,
    ranked: bool,
    subject: &'static str,
    rules: Value,
    seed: i32,
    created_at: DateTime<Utc>,
    started_at: DateTime<Utc>,
}

impl StartRow {
    pub fn of(m: &Match) -> Self {
        Self {
            id: m.id,
            mode: m.mode.key(),
            ranked: m.ranked,
            subject: m.key.subject.key(),
            rules: m.key.rules.to_json(),
            seed: i32::try_from(m.seed).unwrap_or(1),
            created_at: timestamp(m.created_at),
            started_at: timestamp(m.starts_at),
        }
    }
}

/// Records a match as it starts. Best effort: the end writes the full row
/// anyway, and a start written after the end leaves it alone.
pub async fn insert_start(pool: &PgPool, row: &StartRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO game.matches (id, mode, ranked, subject, rules, seed, created_at, started_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(row.id)
    .bind(row.mode)
    .bind(row.ranked)
    .bind(row.subject)
    .bind(Json(&row.rules))
    .bind(row.seed)
    .bind(row.created_at)
    .bind(row.started_at)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Rating change of a seat in a ranked match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatingChange {
    pub before: i32,
    pub after: i32,
}

/// The end of a match in one transaction: the match row, every seat, the
/// new ratings (ranked only) and the `match.completed` outbox row (unless
/// void or without account players). Returns the rating change per seat.
pub async fn finish(
    pool: &PgPool,
    m: &Match,
    result: &Final,
) -> Result<Vec<Option<RatingChange>>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO game.matches
                (id, mode, ranked, subject, rules, seed, created_at, started_at, ended_at, end_reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE
            SET ranked = EXCLUDED.ranked, ended_at = EXCLUDED.ended_at,
                end_reason = EXCLUDED.end_reason",
    )
    .bind(m.id)
    .bind(m.mode.key())
    .bind(result.ranked)
    .bind(m.key.subject.key())
    .bind(Json(m.key.rules.to_json()))
    .bind(i32::try_from(m.seed).unwrap_or(1))
    .bind(timestamp(m.created_at))
    .bind(timestamp(m.starts_at))
    .bind(timestamp(result.ended_at))
    .bind(result.reason.key())
    .execute(&mut *tx)
    .await?;

    let changes = if result.ranked {
        update_ratings(&mut tx, m, result).await?
    } else {
        vec![None; m.seats().len()]
    };

    for (seat, s) in m.seats().iter().enumerate() {
        let (rank, outcome) = result
            .places
            .get(seat)
            .copied()
            .unwrap_or((1, Outcome::Draw));
        let change = changes.get(seat).copied().flatten();
        let (guest_name, ghost_of) = match &s.kind {
            SeatKind::Live(p) if p.is_guest() => (Some(s.name.as_str()), None),
            SeatKind::Live(_) => (None, None),
            SeatKind::Ghost { source_match, .. } => (None, Some(*source_match)),
        };
        sqlx::query(
            "INSERT INTO game.match_players
                    (match_id, seat, user_id, guest_name, ghost_of, name, score, correct,
                     answered, answer_time_ms, rank, outcome, rating_before, rating_after, answers)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(m.id)
        .bind(i16::try_from(seat).unwrap_or(i16::MAX))
        .bind(s.user_id)
        .bind(guest_name)
        .bind(ghost_of)
        .bind(&s.name)
        .bind(s.score)
        .bind(i32::try_from(s.correct).unwrap_or(i32::MAX))
        .bind(i32::try_from(s.answered).unwrap_or(i32::MAX))
        .bind(s.answer_time_ms)
        .bind(i16::try_from(rank).unwrap_or(i16::MAX))
        .bind(outcome.key())
        .bind(change.map(|c| c.before))
        .bind(change.map(|c| c.after))
        .bind(Json(&s.answers))
        .execute(&mut *tx)
        .await?;
    }

    let event_id = Uuid::now_v7();
    if let Some(body) = webhook_body(event_id, m, result, &changes) {
        sqlx::query(
            "INSERT INTO game.webhook_outbox
                    (event_id, type, body, status, attempts, next_attempt_at, created_at)
             VALUES ($1, 'match.completed', $2, 'pending', 0, now(), now())",
        )
        .bind(event_id)
        .bind(Json(body))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(changes)
}

/// Glicko-2 update of the account players of a ranked match, each against
/// every other one (a duel: one opponent). Rows are locked in `user_id`
/// order so that concurrent matches cannot deadlock.
async fn update_ratings(
    tx: &mut Transaction<'_, Postgres>,
    m: &Match,
    result: &Final,
) -> Result<Vec<Option<RatingChange>>, sqlx::Error> {
    let subject = m.key.subject.key();
    let mut accounts: Vec<(usize, i64)> = m
        .seats()
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match &s.kind {
            SeatKind::Live(p) => p.user_id().map(|id| (i, id)),
            SeatKind::Ghost { .. } => None,
        })
        .collect();
    accounts.sort_by_key(|(_, id)| *id);

    let mut before = vec![None; m.seats().len()];
    for (seat, user_id) in &accounts {
        let row: Option<(f64, f64, f64)> = sqlx::query_as(
            "SELECT rating, rd, volatility FROM game.ratings
              WHERE user_id = $1 AND subject = $2 FOR UPDATE",
        )
        .bind(user_id)
        .bind(subject)
        .fetch_optional(&mut **tx)
        .await?;
        before[*seat] = Some(
            row.map_or(glicko::DEFAULT, |(rating, rd, volatility)| Rating {
                rating,
                rd,
                volatility,
            }),
        );
    }

    let mut changes = vec![None; m.seats().len()];
    for (seat, user_id) in &accounts {
        let Some(mine) = before[*seat] else { continue };
        let outcome = result.places.get(*seat).map_or(Outcome::Draw, |p| p.1);
        let games: Vec<glicko::Game> = accounts
            .iter()
            .filter(|(other, _)| other != seat)
            .filter_map(|(other, _)| before[*other])
            .map(|opponent| glicko::Game {
                opponent,
                score: outcome.score(),
            })
            .collect();
        if games.is_empty() {
            continue;
        }
        let after = glicko::update(mine, &games, glicko::TAU);
        let (win, loss, draw) = match outcome {
            Outcome::Win => (1, 0, 0),
            Outcome::Draw => (0, 0, 1),
            Outcome::Loss | Outcome::Forfeit => (0, 1, 0),
        };
        sqlx::query(
            "INSERT INTO game.ratings
                    (user_id, subject, rating, rd, volatility, games, wins, losses, draws, updated_at)
             VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, now())
             ON CONFLICT (user_id, subject) DO UPDATE
                SET rating = EXCLUDED.rating, rd = EXCLUDED.rd,
                    volatility = EXCLUDED.volatility,
                    games = game.ratings.games + 1,
                    wins = game.ratings.wins + EXCLUDED.wins,
                    losses = game.ratings.losses + EXCLUDED.losses,
                    draws = game.ratings.draws + EXCLUDED.draws,
                    updated_at = now()",
        )
        .bind(user_id)
        .bind(subject)
        .bind(after.rating)
        .bind(after.rd)
        .bind(after.volatility)
        .bind(win)
        .bind(loss)
        .bind(draw)
        .execute(&mut **tx)
        .await?;
        changes[*seat] = Some(RatingChange {
            before: mine.rounded(),
            after: after.rounded(),
        });
    }
    Ok(changes)
}

/// The `match.completed` event (`webhooks.md` §3), or `None` when nothing
/// is sent: a void match, or no account among the live players. Guests and
/// ghosts are not listed.
pub fn webhook_body(
    event_id: Uuid,
    m: &Match,
    result: &Final,
    changes: &[Option<RatingChange>],
) -> Option<Value> {
    if result.reason == EndReason::Void {
        return None;
    }
    let players: Vec<Value> = m
        .seats()
        .iter()
        .enumerate()
        .filter_map(|(seat, s)| {
            let user_id = match &s.kind {
                SeatKind::Live(p) => p.user_id()?,
                SeatKind::Ghost { .. } => return None,
            };
            let (rank, outcome) = result.places.get(seat).copied()?;
            let change = changes.get(seat).copied().flatten();
            Some(json!({
                "user_id": user_id.to_string(),
                "team": null,
                "outcome": outcome.key(),
                "score": s.score,
                "rank": rank,
                "rating_before": change.map(|c| c.before),
                "rating_after": change.map(|c| c.after),
            }))
        })
        .collect();
    if players.is_empty() {
        return None;
    }
    Some(json!({
        "event_id": event_id.to_string(),
        "type": "match.completed",
        "version": 1,
        "occurred_at": rfc3339(result.ended_at),
        "data": {
            "match_id": m.id.to_string(),
            "mode": m.mode.webhook_key(),
            "ranked": result.ranked,
            "started_at": rfc3339(m.starts_at),
            "ended_at": rfc3339(result.ended_at),
            "end_reason": result.reason.key(),
            "players": players,
        },
    }))
}

/// A player's rating for a subject, if they ever played ranked.
pub async fn rating(
    pool: &PgPool,
    user_id: i64,
    subject: SubjectKey,
) -> Result<Option<Rating>, sqlx::Error> {
    let row: Option<(f64, f64, f64)> = sqlx::query_as(
        "SELECT rating, rd, volatility FROM game.ratings WHERE user_id = $1 AND subject = $2",
    )
    .bind(user_id)
    .bind(subject.key())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(rating, rd, volatility)| Rating {
        rating,
        rd,
        volatility,
    }))
}

/// A recorded run to replay.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostRun {
    pub source_match: Uuid,
    pub seed: u32,
    pub name: String,
    pub log: Vec<AnswerLog>,
}

/// The `FROM … WHERE` of ghost candidates: `$1` subject, `$2` rules, `$3`
/// the account to exclude. A macro so that queries stay literals.
macro_rules! ghost_candidates {
    () => {
        "
    FROM game.match_players p
    JOIN game.matches m ON m.id = p.match_id
    LEFT JOIN game.ratings r ON r.user_id = p.user_id AND r.subject = m.subject
   WHERE m.subject = $1 AND m.rules = $2
     AND m.end_reason IS NOT NULL AND m.end_reason <> 'void'
     AND p.user_id IS NOT NULL AND p.user_id IS DISTINCT FROM $3
     AND p.answered > 0"
    };
}

/// Whether a run of another account exists for `key`.
pub async fn ghost_exists(
    pool: &PgPool,
    key: QueueKey,
    exclude: Option<i64>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "SELECT EXISTS (SELECT 1 ",
        ghost_candidates!(),
        ")"
    ))
    .bind(key.subject.key())
    .bind(Json(key.rules.to_json()))
    .bind(exclude)
    .fetch_one(pool)
    .await
}

/// A run of another account for `key`: random among the 20 whose player's
/// rating is closest to `rating`.
pub async fn pick_ghost(
    pool: &PgPool,
    key: QueueKey,
    exclude: Option<i64>,
    rating: f64,
) -> Result<Option<GhostRun>, sqlx::Error> {
    let rows: Vec<(Uuid, i32, String, Json<Vec<AnswerLog>>)> = sqlx::query_as(concat!(
        "SELECT p.match_id, m.seed, p.name, p.answers ",
        ghost_candidates!(),
        " ORDER BY abs(coalesce(r.rating, 1500) - $4), m.ended_at DESC LIMIT 20"
    ))
    .bind(key.subject.key())
    .bind(Json(key.rules.to_json()))
    .bind(exclude)
    .bind(rating)
    .fetch_all(pool)
    .await?;
    let len = rows.len() as u64;
    let pick = usize::try_from(super::random_below(len)).unwrap_or(0);
    Ok(rows
        .into_iter()
        .nth(pick)
        .map(|(source_match, seed, name, Json(mut log))| {
            log.sort_by_key(|e| e.t);
            GhostRun {
                source_match,
                seed: u32::try_from(seed).unwrap_or(1),
                name,
                log,
            }
        }))
}

/// A row of the leaderboard.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LeaderRow {
    pub rank: i64,
    pub user_id: i64,
    pub pseudo: String,
    pub avatar_path: Option<String>,
    pub rating: f64,
    pub rd: f64,
    pub games: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
}

/// Best ratings first, among players with a ranked match whose account
/// still exists. Equal shown ratings share a rank.
pub async fn leaderboard(
    pool: &PgPool,
    subject: SubjectKey,
    limit: i64,
) -> Result<Vec<LeaderRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT rank() OVER (ORDER BY round(r.rating) DESC) AS rank,
                r.user_id, mb.pseudo, mb.avatar_path, r.rating, r.rd,
                r.games, r.wins, r.losses, r.draws
           FROM game.ratings r
           JOIN api_read.members mb ON mb.id = r.user_id
          WHERE r.subject = $1 AND r.games > 0
          ORDER BY r.rating DESC, r.games DESC, r.user_id
          LIMIT $2",
    )
    .bind(subject.key())
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MyRating {
    pub subject: String,
    pub rating: f64,
    pub rd: f64,
    pub games: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
    /// Rank in the leaderboard of the subject.
    pub rank: i64,
}

pub async fn my_ratings(pool: &PgPool, user_id: i64) -> Result<Vec<MyRating>, sqlx::Error> {
    sqlx::query_as(
        "SELECT r.subject, r.rating, r.rd, r.games, r.wins, r.losses, r.draws,
                1 + (SELECT count(*)
                       FROM game.ratings o
                       JOIN api_read.members mb ON mb.id = o.user_id
                      WHERE o.subject = r.subject AND o.games > 0
                        AND round(o.rating) > round(r.rating)) AS rank
           FROM game.ratings r
          WHERE r.user_id = $1 AND r.games > 0
          ORDER BY r.subject",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecentMatch {
    pub match_id: Uuid,
    pub mode: String,
    pub ranked: bool,
    pub subject: String,
    pub rules: Json<Value>,
    pub ended_at: DateTime<Utc>,
    pub end_reason: String,
    pub outcome: String,
    pub score: i32,
    pub rating_before: Option<i32>,
    pub rating_after: Option<i32>,
    pub opponents: Vec<String>,
}

/// The last 20 finished matches of `user_id`, newest first.
pub async fn recent(pool: &PgPool, user_id: i64) -> Result<Vec<RecentMatch>, sqlx::Error> {
    sqlx::query_as(
        "SELECT m.id AS match_id, m.mode, m.ranked, m.subject, m.rules, m.ended_at,
                m.end_reason, p.outcome, p.score, p.rating_before, p.rating_after,
                coalesce((SELECT array_agg(o.name ORDER BY o.seat)
                            FROM game.match_players o
                           WHERE o.match_id = m.id AND o.seat <> p.seat),
                         '{}') AS opponents
           FROM game.match_players p
           JOIN game.matches m ON m.id = p.match_id
          WHERE p.user_id = $1 AND m.ended_at IS NOT NULL
          ORDER BY m.ended_at DESC, m.id DESC
          LIMIT 20",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
