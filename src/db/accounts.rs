//! Members' accounts in `site.users`, for logging in, signing up and
//! changing passwords (`docs/contract/auth-sessions.md`). The API can read
//! only the columns listed here: never the two-factor secrets nor the
//! remember token (column grants of `20261029000000_auth_sessions`).

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};

/// The login columns of an account.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    /// Bcrypt hash; `None` for an account created through Google.
    pub password: Option<String>,
    pub locale: Option<String>,
    pub onboarding_at: Option<DateTime<Utc>>,
}

const ACCOUNT_COLUMNS: &str = "id, pseudo::text AS pseudo, email::text AS email,
    email_verified_at AT TIME ZONE 'UTC' AS email_verified_at, password::text AS password,
    locale::text AS locale, onboarding_at AT TIME ZONE 'UTC' AS onboarding_at";

/// The account whose e-mail or pseudo is `login`, ignoring case. When both
/// match different accounts, the exact e-mail wins, then the lowest id.
pub async fn find_by_login(pool: &PgPool, login: &str) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCOUNT_COLUMNS} FROM site.users
          WHERE lower(email) = lower($1) OR lower(pseudo) = lower($1)
          ORDER BY (email = $1) DESC, id
          LIMIT 1"
    )))
    .bind(login)
    .fetch_optional(pool)
    .await
}

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCOUNT_COLUMNS} FROM site.users WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Whether the pseudo and the e-mail are already used, ignoring case.
pub async fn taken(pool: &PgPool, pseudo: &str, email: &str) -> Result<(bool, bool), sqlx::Error> {
    sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM site.users WHERE lower(pseudo) = lower($1)),
                EXISTS (SELECT 1 FROM site.users WHERE lower(email) = lower($2))",
    )
    .bind(pseudo)
    .bind(email)
    .fetch_one(pool)
    .await
}

/// Why an account could not be created.
#[derive(Debug)]
pub enum CreateError {
    PseudoTaken,
    EmailTaken,
    Database(sqlx::Error),
}

/// Creates an account as Laravel's `CreateNewUser` does: `name` is the
/// pseudo, e-mail not verified, onboarding not done.
pub async fn create(
    pool: &PgPool,
    pseudo: &str,
    email: &str,
    hash: &str,
    locale: Option<&str>,
) -> Result<AccountRow, CreateError> {
    let result = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO site.users (pseudo, name, email, password, locale, created_at, updated_at)
         VALUES ($1, $1, $2, $3, $4, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING {ACCOUNT_COLUMNS}"
    )))
    .bind(pseudo)
    .bind(email)
    .bind(hash)
    .bind(locale)
    .fetch_one(pool)
    .await;
    result.map_err(create_error)
}

fn create_error(error: sqlx::Error) -> CreateError {
    let constraint = error
        .as_database_error()
        .and_then(|e| e.constraint())
        .map(str::to_owned);
    match constraint.as_deref() {
        Some("users_pseudo_unique") => CreateError::PseudoTaken,
        Some("users_email_unique") => CreateError::EmailTaken,
        _ => CreateError::Database(error),
    }
}

/// The account whose e-mail is `email`, ignoring case; the exact spelling
/// first, then the lowest id.
pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCOUNT_COLUMNS} FROM site.users
          WHERE lower(email) = lower($1)
          ORDER BY (email = $1) DESC, id
          LIMIT 1"
    )))
    .bind(email)
    .fetch_optional(pool)
    .await
}

/// Which of `pseudos` are already used, ignoring case: their lower case.
pub async fn taken_pseudos(pool: &PgPool, pseudos: &[String]) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT lower(pseudo)::text FROM site.users WHERE lower(pseudo) = ANY($1::text[])",
    )
    .bind(pseudos.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>())
    .fetch_all(pool)
    .await
}

/// Creates an account for a provider identity, as the site's
/// `ConnexionSociale::creer`: no password, the address verified only if the
/// provider says so.
pub async fn create_social(
    tx: &mut Transaction<'_, Postgres>,
    pseudo: &str,
    email: &str,
    verified: bool,
) -> Result<AccountRow, CreateError> {
    let result = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO site.users (pseudo, name, email, password, email_verified_at, created_at, updated_at)
         VALUES ($1, $1, $2, NULL, CASE WHEN $3 THEN now() AT TIME ZONE 'UTC' END,
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING {ACCOUNT_COLUMNS}"
    )))
    .bind(pseudo)
    .bind(email)
    .bind(verified)
    .fetch_one(&mut **tx)
    .await;
    result.map_err(create_error)
}

pub async fn set_password(pool: &PgPool, id: i64, hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE site.users SET password = $2, updated_at = now() AT TIME ZONE 'UTC' WHERE id = $1",
    )
    .bind(id)
    .bind(hash)
    .execute(pool)
    .await?;
    Ok(())
}

/// The permission roles of account `user_id` (`moderateur`, `admin`,
/// `super-admin`), highest first: the `roles` claim of a game token.
pub async fn permission_roles(pool: &PgPool, user_id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT r.slug::text FROM site.role_user ru
           JOIN site.roles r ON r.id = ru.role_id
          WHERE ru.user_id = $1 AND r.categorie = 'permission'
          ORDER BY r.niveau DESC, r.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
