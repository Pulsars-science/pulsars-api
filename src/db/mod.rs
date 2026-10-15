//! PostgreSQL access.
//!
//! The API's role (`pulsars_api`) can read the `api_read` views and owns the
//! `game` schema; it has no right on `site` (see `docs/contract/database.md`).
//! Its `search_path` is `game, api_read`, so queries name views without a
//! schema only when unambiguous; new code should qualify them anyway.

pub mod courses;
pub mod exams;
pub mod exercises;
pub mod members;
pub mod resources;

use std::str::FromStr;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::config::Config;

/// Creates the pool without connecting. A database that is down at startup
/// must not crash the process: `/healthz` reports it, and the pool connects
/// as soon as the database answers.
pub fn connect_lazy(config: &Config) -> Result<PgPool, sqlx::Error> {
    let options =
        PgConnectOptions::from_str(config.database_url.expose())?.application_name("pulsars-api");
    Ok(PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .min_connections(0)
        // Short: a request waiting for a connection is better answered with
        // a 503 than left hanging.
        .acquire_timeout(Duration::from_secs(2))
        .idle_timeout(Duration::from_secs(300))
        .connect_lazy_with(options))
}

/// Outcome of a health check, as published by `/healthz`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    Timeout,
    ConnectionRefused,
    AuthFailed,
    Other,
}

impl CheckError {
    /// Short, stable code. Never the underlying message, which may contain a
    /// host name, a user name or a path.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::ConnectionRefused => "connection_refused",
            Self::AuthFailed => "auth_failed",
            Self::Other => "error",
        }
    }
}

pub async fn ping(pool: &PgPool, timeout: Duration) -> Result<(), CheckError> {
    let query = sqlx::query("SELECT 1").execute(pool);
    match tokio::time::timeout(timeout, query).await {
        Err(_) => Err(CheckError::Timeout),
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(classify(&error)),
    }
}

fn classify(error: &sqlx::Error) -> CheckError {
    match error {
        sqlx::Error::PoolTimedOut => CheckError::Timeout,
        sqlx::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused => {
            CheckError::ConnectionRefused
        }
        // 28000 invalid_authorization_specification, 28P01 invalid_password.
        sqlx::Error::Database(db)
            if matches!(db.code().as_deref(), Some("28000") | Some("28P01")) =>
        {
            CheckError::AuthFailed
        }
        _ => CheckError::Other,
    }
}
