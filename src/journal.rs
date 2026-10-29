//! What the admin journal records beside the existing tables
//! (`docs/privacy.md`): page views (only after consent), security events
//! and proofs of consent, and their daily purge.
//!
//! - A page view needs a member or a visitor id (`Pulsars-Visitor`, the
//!   site's `pulsars_vid` cookie, which the site only sets after the
//!   visitor accepted the "mesure" cookies): without either nothing is
//!   written.
//! - When a session opens with a visitor id, the visitor's anonymous page
//!   views become the member's ([`link_visitor`]).
//! - A security event that cannot be written is logged, never an error for
//!   the request that caused it ([`security`]).
//! - Retention: page views 13 months, security events 12, consents 25
//!   ([`purge`], daily and at start-up).

use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::auth::Caller;

/// How often old rows are purged.
pub const PURGE_EVERY: Duration = Duration::from_secs(24 * 3600);

/// Retention of each table, as a PostgreSQL interval.
pub const RETENTION: [(&str, &str); 3] = [
    ("site.page_views", "13 months"),
    ("site.security_events", "12 months"),
    ("site.consents", "25 months"),
];

/// The kinds of `site.security_events` the API writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Signup,
    Login,
    LoginFailed,
    Logout,
    PasswordChanged,
    PasswordResetRequested,
    PasswordReset,
    EmailVerified,
    OauthLogin,
    OauthLinked,
    OauthUnlinked,
}

impl Security {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signup => "signup",
            Self::Login => "login",
            Self::LoginFailed => "login_failed",
            Self::Logout => "logout",
            Self::PasswordChanged => "password_changed",
            Self::PasswordResetRequested => "password_reset_requested",
            Self::PasswordReset => "password_reset",
            Self::EmailVerified => "email_verified",
            Self::OauthLogin => "oauth_login",
            Self::OauthLinked => "oauth_linked",
            Self::OauthUnlinked => "oauth_unlinked",
        }
    }
}

/// Records a security event of `user_id` (none: an unknown account), with
/// the caller's address and browser. A failure is logged only.
pub async fn security(
    pool: &PgPool,
    user_id: Option<i64>,
    kind: Security,
    caller: &Caller,
    detail: Value,
) {
    let written = sqlx::query(
        "INSERT INTO site.security_events (user_id, kind, ip, user_agent, detail)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .bind(caller.ip.as_deref())
    .bind(caller.user_agent.as_deref())
    .bind(sqlx::types::Json(&detail))
    .execute(pool)
    .await;
    if let Err(error) = written {
        tracing::warn!(error = %error, kind = kind.as_str(), "cannot record a security event");
    }
}

/// The visitor's anonymous page views become `user_id`'s. A failure is
/// logged only.
pub async fn link_visitor(pool: &PgPool, user_id: i64, visitor: Option<Uuid>) {
    let Some(visitor) = visitor else { return };
    let linked = sqlx::query(
        "UPDATE site.page_views SET user_id = $1 WHERE visitor_id = $2 AND user_id IS NULL",
    )
    .bind(user_id)
    .bind(visitor)
    .execute(pool)
    .await;
    if let Err(error) = linked {
        tracing::warn!(error = %error, "cannot link a visitor's page views");
    }
}

/// One page view; nothing without a member nor a visitor.
pub async fn page_view(
    pool: &PgPool,
    user_id: Option<i64>,
    visitor: Option<Uuid>,
    path: &str,
    referrer: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if user_id.is_none() && visitor.is_none() {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO site.page_views (user_id, visitor_id, path, referrer) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(visitor)
    .bind(path)
    .bind(referrer)
    .execute(pool)
    .await?;
    Ok(true)
}

/// A proof of the banner's answer.
pub async fn consent(
    pool: &PgPool,
    user_id: Option<i64>,
    visitor: Option<Uuid>,
    measure: bool,
    version: i32,
    ip_hash: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.consents (user_id, visitor_id, measure, version, ip_hash)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(visitor)
    .bind(measure)
    .bind(version)
    .bind(ip_hash)
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes the rows past their retention; the number deleted per table.
pub async fn purge(pool: &PgPool) -> Result<Vec<(&'static str, u64)>, sqlx::Error> {
    let mut deleted = Vec::new();
    for (table, keep) in RETENTION {
        // Both are constants of this module.
        let done = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE at < now() - interval '{keep}'"
        )))
        .execute(pool)
        .await?;
        deleted.push((table, done.rows_affected()));
    }
    Ok(deleted)
}

/// Purges now, then every [`PURGE_EVERY`], forever.
pub async fn run_purger(pool: PgPool) {
    let mut tick = tokio::time::interval(PURGE_EVERY);
    loop {
        tick.tick().await;
        match purge(&pool).await {
            Ok(deleted) => {
                for (table, rows) in deleted.into_iter().filter(|(_, n)| *n > 0) {
                    tracing::info!(table, rows, "journal rows past their retention deleted");
                }
            }
            Err(error) => tracing::warn!(error = %error, "cannot purge the journal"),
        }
    }
}

/// The `Pulsars-Visitor` id: a UUID v4, anything else ignored.
pub fn visitor_id(header: Option<&str>) -> Option<Uuid> {
    let id = Uuid::try_parse(header?.trim()).ok()?;
    (id.get_version_num() == 4).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_uuid_v4_is_a_visitor() {
        let v4 = "3b241101-e2bb-4255-8caf-4136c566a962";
        assert_eq!(
            visitor_id(Some(v4)).map(|u| u.to_string()).as_deref(),
            Some(v4)
        );
        assert!(visitor_id(Some(&format!(" {v4} "))).is_some());
        assert!(visitor_id(Some("3B241101-E2BB-4255-8CAF-4136C566A962")).is_some());
        // A v7, garbage, nothing.
        assert!(visitor_id(Some("01920000-0000-7000-8000-000000000000")).is_none());
        assert!(visitor_id(Some("not-a-uuid")).is_none());
        assert!(visitor_id(Some("")).is_none());
        assert!(visitor_id(None).is_none());
    }
}
