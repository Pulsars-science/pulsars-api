//! Login sessions in `site.auth_sessions` (`docs/contract/auth-sessions.md`).
//! Only the SHA-256 of a token is stored; the token itself is shown once,
//! to whoever opened the session.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// How long a session lives after its last use: two hours, or a year with
/// « Se souvenir de moi ».
const LIFETIME: &str =
    "CASE WHEN remember THEN interval '365 days' ELSE interval '120 minutes' END";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: i64,
    pub remember: bool,
    pub expires_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
}

/// Opens a session for `user_id`, keyed by the hash of a new token.
pub async fn create(
    pool: &PgPool,
    user_id: i64,
    token_hash: &[u8],
    remember: bool,
    ip: Option<&str>,
    user_agent: Option<&str>,
) -> Result<SessionRow, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO site.auth_sessions (id, user_id, token_hash, remember, expires_at, ip, user_agent)
         VALUES ($1, $2, $3, $4,
                 now() + CASE WHEN $4 THEN interval '365 days' ELSE interval '120 minutes' END,
                 $5::inet, $6)
         RETURNING id, user_id, remember, expires_at, last_used_at",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(token_hash)
    .bind(remember)
    .bind(ip)
    .bind(user_agent)
    .fetch_one(pool)
    .await
}

/// The live session of `token_hash`, extended by its lifetime. The row is
/// written at most once a minute, so a page full of requests costs one
/// write.
pub async fn use_token(
    pool: &PgPool,
    token_hash: &[u8],
) -> Result<Option<SessionRow>, sqlx::Error> {
    let found: Option<SessionRow> = sqlx::query_as(
        "SELECT id, user_id, remember, expires_at, last_used_at FROM site.auth_sessions
          WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    let Some(mut row) = found else {
        return Ok(None);
    };
    if Utc::now() - row.last_used_at >= chrono::Duration::minutes(1) {
        let extended: Option<DateTime<Utc>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "UPDATE site.auth_sessions SET last_used_at = now(), expires_at = now() + {LIFETIME}
              WHERE id = $1 AND revoked_at IS NULL
              RETURNING expires_at"
        )))
        .bind(row.id)
        .fetch_optional(pool)
        .await?;
        match extended {
            Some(at) => {
                row.expires_at = at;
                row.last_used_at = Utc::now();
            }
            // Revoked in between.
            None => return Ok(None),
        }
    }
    Ok(Some(row))
}

/// Revokes the session of `token_hash`, if it is still live: its account.
pub async fn revoke(pool: &PgPool, token_hash: &[u8]) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE site.auth_sessions SET revoked_at = now()
          WHERE token_hash = $1 AND revoked_at IS NULL
          RETURNING user_id",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

/// Revokes every live session of `user_id` but `keep`.
pub async fn revoke_others(
    pool: &PgPool,
    user_id: i64,
    keep: Option<Uuid>,
) -> Result<u64, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE site.auth_sessions SET revoked_at = now()
          WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)",
    )
    .bind(user_id)
    .bind(keep)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}
