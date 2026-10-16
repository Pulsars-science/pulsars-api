//! `GET /healthz`: liveness and readiness probe (see `openapi.yaml`).
//!
//! PostgreSQL is vital: without it no content can be served, so its failure
//! answers 503. Redis only carries the game: its failure degrades the service
//! but keeps the answer at 200, so a load balancer keeps sending content
//! traffic.
//!
//! The instance is not ready either until Laravel's JWKS has been loaded
//! once (`docs/contract/auth-jwt.md` §3): `down` and 503 meanwhile. The
//! `Health` schema only lists the `postgres` and `redis` checks, so the JWKS
//! state is not a check entry; the refresh task logs why it is missing.

use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;

use crate::db::{self, CheckError};
use crate::state::AppState;
use crate::telemetry::names;

const CHECK_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
    pub checked_at: String,
    pub checks: Vec<Check>,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub duration_ms: f64,
    pub error: Option<&'static str>,
}

pub async fn healthz(State(state): State<AppState>) -> Response {
    let (postgres, redis) = tokio::join!(
        timed("postgres", db::ping(&state.db, CHECK_TIMEOUT)),
        timed("redis", crate::redis::ping(&state.redis, CHECK_TIMEOUT)),
    );

    let (status, code) = match (postgres.ok, redis.ok, state.jwks.is_loaded()) {
        (false, _, _) | (_, _, false) => ("down", StatusCode::SERVICE_UNAVAILABLE),
        (true, true, true) => ("ok", StatusCode::OK),
        (true, false, true) => ("degraded", StatusCode::OK),
    };

    let body = Health {
        status,
        version: env!("CARGO_PKG_VERSION"),
        checked_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        checks: vec![postgres, redis],
    };
    let mut response = (code, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn timed(name: &'static str, check: impl Future<Output = Result<(), CheckError>>) -> Check {
    let started = Instant::now();
    let result = check.await;
    let duration_ms = (started.elapsed().as_secs_f64() * 10_000.0).round() / 10.0;
    metrics::gauge!(names::HEALTH_CHECK_UP, "dependency" => name).set(if result.is_ok() {
        1.0
    } else {
        0.0
    });
    Check {
        name,
        ok: result.is_ok(),
        duration_ms,
        error: result.err().map(|e| e.code()),
    }
}
