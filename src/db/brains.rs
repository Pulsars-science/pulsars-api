//! The brains ledger, `site.cerveaux_mouvements` (`docs/contract/brains.md`),
//! as Laravel's `App\Services\Cerveaux`: the balance is the sum of a
//! member's movements, and every credit carries a key unique per member
//! (`lecon:42`, `partie:<match_id>`…) that makes it idempotent.
//!
//! The credits run inside the transaction of what earns them (a lesson
//! completed, a match ended, a season closed), on its connection.

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

/// A lesson completed, once per lesson.
pub const LESSON: i32 = 5;
/// A match played, whatever the mode or the outcome.
pub const MATCH: i32 = 2;
/// On top of that, a match won (`outcome = win`).
pub const WIN: i32 = 3;
/// Matches and wins of one UTC day (that of `ended_at`), at most.
pub const MATCH_DAILY_CAP: i32 = 20;

/// The reward of a rank in the monthly standings of a subject.
pub fn season_reward(rank: i64) -> i32 {
    match rank {
        1 => 300,
        2..=3 => 200,
        4..=10 => 100,
        11..=50 => 40,
        _ => 10,
    }
}

/// `timestamp(0)` rounds fractions to the nearest second: truncate first,
/// so that 23:59:59.7 stays on its day.
fn seconds(at: DateTime<Utc>) -> NaiveDateTime {
    DateTime::from_timestamp(at.timestamp(), 0)
        .unwrap_or(at)
        .naive_utc()
}

/// Writes a movement, once per (member, key). Returns the amount actually
/// credited: 0 when the key exists already, or for an amount of 0, which is
/// not written. The conflict is ignored rather than raised, so the
/// surrounding transaction goes on.
pub async fn credit(
    conn: &mut PgConnection,
    user_id: i64,
    amount: i32,
    reason: &str,
    key: &str,
    details: Value,
    at: DateTime<Utc>,
) -> Result<i32, sqlx::Error> {
    if amount == 0 {
        return Ok(0);
    }
    let written = sqlx::query(
        "INSERT INTO site.cerveaux_mouvements (user_id, montant, raison, cle, details, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id, cle) DO NOTHING",
    )
    .bind(user_id)
    .bind(amount)
    .bind(reason)
    .bind(key)
    .bind(sqlx::types::Json(details))
    .bind(seconds(at))
    .execute(conn)
    .await?
    .rows_affected();
    Ok(if written > 0 { amount } else { 0 })
}

/// A lesson completed. Nothing the second time, even after taking the
/// lesson back and completing it again.
pub async fn credit_lesson(
    conn: &mut PgConnection,
    user_id: i64,
    lesson_id: i64,
    at: DateTime<Utc>,
) -> Result<i32, sqlx::Error> {
    credit(
        conn,
        user_id,
        LESSON,
        "lecon",
        &format!("lecon:{lesson_id}"),
        json!({"lesson_id": lesson_id}),
        at,
    )
    .await
}

/// A match ended: 2 for playing, 3 more for a win, within 20 a UTC day (of
/// `ended_at`). A credit over the cap is cut to what is left and written
/// with that amount; one cut to zero is not written. The member's row is
/// locked so that two matches of the same day cannot pass the cap
/// together. `None` when the account no longer exists.
pub async fn credit_match(
    conn: &mut PgConnection,
    user_id: i64,
    match_id: &str,
    won: bool,
    ended_at: DateTime<Utc>,
) -> Result<Option<i32>, sqlx::Error> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM site.users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await?;
    if exists.is_none() {
        return Ok(None);
    }
    let day = seconds(ended_at).date();
    let start = day.and_hms_opt(0, 0, 0).unwrap_or_default();
    let earned: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(montant), 0)::bigint FROM site.cerveaux_mouvements
          WHERE user_id = $1 AND raison IN ('partie', 'victoire')
            AND created_at >= $2 AND created_at < $3",
    )
    .bind(user_id)
    .bind(start)
    .bind(start + Duration::days(1))
    .fetch_one(&mut *conn)
    .await?;
    let mut left = i64::from(MATCH_DAILY_CAP) - earned;
    let mut total = 0;
    let mut gains = vec![("partie", MATCH)];
    if won {
        gains.push(("victoire", WIN));
    }
    for (reason, points) in gains {
        let amount = i32::try_from(i64::from(points).min(left.max(0))).unwrap_or(0);
        let credited = credit(
            conn,
            user_id,
            amount,
            reason,
            &format!("{reason}:{match_id}"),
            json!({"match_id": match_id}),
            ended_at,
        )
        .await?;
        left -= i64::from(credited);
        total += credited;
    }
    Ok(Some(total))
}

/// The reward of one line of a closed season, key
/// `saison:<YYYY-MM>:<subject>`. A deleted account is skipped (0).
pub async fn credit_season(
    conn: &mut PgConnection,
    user_id: i64,
    season: &str,
    subject: &str,
    rank: i64,
    at: DateTime<Utc>,
) -> Result<i32, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM site.users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?;
    if !exists {
        return Ok(0);
    }
    credit(
        conn,
        user_id,
        season_reward(rank),
        "saison",
        &format!("saison:{season}:{subject}"),
        json!({"season": season, "subject": subject, "rank": rank}),
        at,
    )
    .await
}

/// A movement, for the dashboard's history.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Movement {
    pub id: i64,
    pub amount: i32,
    pub reason: String,
    pub details: Option<sqlx::types::Json<Value>>,
    pub created_at: DateTime<Utc>,
}

/// The movements of `user_id`, newest first, after `after` (a
/// `(created_at, id)` of the previous page), at most `limit`.
pub async fn history(
    pool: &PgPool,
    user_id: i64,
    after: Option<(DateTime<Utc>, i64)>,
    limit: i64,
) -> Result<Vec<Movement>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, montant AS amount, raison::text AS reason, details::jsonb AS details,
                created_at AT TIME ZONE 'UTC' AS created_at
           FROM site.cerveaux_mouvements
          WHERE user_id = $1
            AND ($2::timestamptz IS NULL
                 OR (created_at AT TIME ZONE 'UTC', id) < ($2::timestamptz, $3::bigint))
          ORDER BY created_at DESC, id DESC
          LIMIT $4",
    )
    .bind(user_id)
    .bind(after.map(|a| a.0))
    .bind(after.map(|a| a.1))
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_rewards_follow_the_tiers() {
        let rewards: Vec<i32> = [1, 2, 3, 4, 10, 11, 50, 51, 500]
            .into_iter()
            .map(season_reward)
            .collect();
        assert_eq!(rewards, vec![300, 200, 200, 100, 100, 40, 40, 10, 10]);
    }

    #[test]
    fn credits_are_dated_to_the_second() {
        let at = DateTime::parse_from_rfc3339("2026-10-31T23:59:59.900Z")
            .unwrap_or_default()
            .with_timezone(&Utc);
        assert_eq!(seconds(at).to_string(), "2026-10-31 23:59:59");
    }
}
