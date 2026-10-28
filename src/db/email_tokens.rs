//! Links sent by mail, in `site.email_tokens` (`docs/contract/auth-sessions.md`):
//! the address verification link and the password reset link. Only the
//! SHA-256 of a link's token is stored; a token works once, before it
//! expires, for the address it was sent to.

use sqlx::PgPool;
use uuid::Uuid;

/// What a link is for (`kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Verify,
    Reset,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Reset => "reset",
        }
    }
}

/// Stores a new link for `user_id` at `email`, valid `minutes`. The other
/// unused links of the same kind stop working: only the last mail counts,
/// as with Laravel's password broker.
pub async fn create(
    pool: &PgPool,
    user_id: i64,
    kind: Kind,
    token_hash: &[u8],
    email: &str,
    minutes: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM site.email_tokens WHERE user_id = $1 AND kind = $2 AND used_at IS NULL",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO site.email_tokens (id, user_id, kind, token_hash, email, expires_at)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(mins => $6::int))",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(kind.as_str())
    .bind(token_hash)
    .bind(email)
    .bind(i32::try_from(minutes).unwrap_or(i32::MAX))
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Whether a link of `kind` was sent to `user_id` in the last `seconds`.
pub async fn sent_within(
    pool: &PgPool,
    user_id: i64,
    kind: Kind,
    seconds: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.email_tokens
                         WHERE user_id = $1 AND kind = $2
                           AND created_at > now() - make_interval(secs => $3::double precision))",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .bind(seconds as f64)
    .fetch_one(pool)
    .await
}

/// Uses the verification link of `token_hash`: the account's address is
/// verified if the link is unused, unexpired and was sent to the address
/// the account still has. The account's id, or `None`.
pub async fn verify_email(pool: &PgPool, token_hash: &[u8]) -> Result<Option<i64>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let user_id: Option<i64> = sqlx::query_scalar(
        "UPDATE site.email_tokens t SET used_at = now()
           FROM site.users u
          WHERE t.token_hash = $1 AND t.kind = 'verify' AND t.used_at IS NULL
            AND t.expires_at > now() AND u.id = t.user_id AND u.email = t.email
          RETURNING t.user_id",
    )
    .bind(token_hash)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(id) = user_id {
        sqlx::query(
            "UPDATE site.users
                SET email_verified_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
              WHERE id = $1 AND email_verified_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(user_id)
}

/// Uses the reset link of `token_hash` sent to `email` (ignoring case):
/// the account's id if it was unused and unexpired.
pub async fn use_reset(
    pool: &PgPool,
    token_hash: &[u8],
    email: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE site.email_tokens SET used_at = now()
          WHERE token_hash = $1 AND kind = 'reset' AND used_at IS NULL
            AND expires_at > now() AND lower(email) = lower($2)
          RETURNING user_id",
    )
    .bind(token_hash)
    .bind(email)
    .fetch_optional(pool)
    .await
}
