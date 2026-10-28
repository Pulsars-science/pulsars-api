//! Per-client rate limits (`docs/contract/README.md`, rate limits).
//!
//! - A request with the service key (`Pulsars-Service-Key`, `SERVICE_KEY`)
//!   is not counted: it is the site, calling from the same machine on
//!   behalf of all its visitors.
//! - A request with a valid game token is counted against its `sub`
//!   (120 per minute by default); any other request against the client IP
//!   (60 per minute), taken from the last `X-Forwarded-For` entry, which
//!   the edge sets.
//! - Counters live in Redis, so every API instance shares them: a key per
//!   client, incremented on each request, that expires 60 seconds after the
//!   client's first request of the window.
//! - Every limited response carries `RateLimit-Limit`, `RateLimit-Remaining`
//!   and `RateLimit-Reset`; a refused one is a 429 problem with
//!   `Retry-After`.
//!
//! The contract does not say what happens when Redis is down. `/healthz`
//! keeps content readable without Redis, so limits **fail open**: the
//! request is served without rate limit headers, and the failure is counted
//! and logged at most once a minute.
//!
//! A token is verified here, once: the outcome is stored in the request so
//! the [`crate::auth::AuthUser`] extractor does not verify it again. An
//! invalid token is simply counted against the IP; endpoints that require
//! one still answer 401.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use fred::prelude::{ClientLike as _, KeysInterface as _};
use fred::types::ExpireOptions;

use crate::auth::{self, Expected, TokenCheck};
use crate::error::ApiError;
use crate::middleware as mw;
use crate::state::AppState;
use crate::telemetry::names;

/// Length of a window, in seconds.
pub const WINDOW_SECS: i64 = 60;

/// Prefix of every counter key in Redis.
const KEY_PREFIX: &str = "pulsars-api:ratelimit";

/// How long a request may wait for Redis before being let through.
const REDIS_TIMEOUT: Duration = Duration::from_millis(250);

pub const RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("ratelimit-limit");
pub const RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("ratelimit-remaining");
pub const RATELIMIT_RESET: HeaderName = HeaderName::from_static("ratelimit-reset");
pub const SERVICE_KEY: HeaderName = HeaderName::from_static("pulsars-service-key");

/// Who a request is counted against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Client {
    /// Authenticated: the token's `sub`.
    Member(String),
    /// Anonymous: the client IP, or `unknown` when there is none.
    Ip(String),
}

impl Client {
    fn scope(&self) -> &'static str {
        match self {
            Self::Member(_) => "sub",
            Self::Ip(_) => "ip",
        }
    }

    fn key(&self) -> String {
        match self {
            Self::Member(sub) => format!("{KEY_PREFIX}:sub:{sub}"),
            Self::Ip(ip) => format!("{KEY_PREFIX}:ip:{ip}"),
        }
    }
}

/// The state of a client's window after counting one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub limit: u32,
    /// Requests counted in the window, this one included.
    pub count: u64,
    /// Seconds until the window ends.
    pub reset: u64,
}

impl Decision {
    pub fn allowed(&self) -> bool {
        self.count <= u64::from(self.limit)
    }

    pub fn remaining(&self) -> u64 {
        u64::from(self.limit).saturating_sub(self.count)
    }

    fn apply(&self, headers: &mut HeaderMap) {
        headers.insert(RATELIMIT_LIMIT, HeaderValue::from(self.limit));
        headers.insert(RATELIMIT_REMAINING, HeaderValue::from(self.remaining()));
        headers.insert(RATELIMIT_RESET, HeaderValue::from(self.reset));
    }
}

/// Middleware: counts the request and refuses it once the client is over
/// its limit.
pub async fn rate_limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if is_service(&state, request.headers()) {
        return next.run(request).await;
    }
    let (mut parts, body) = request.into_parts();
    let client = identify(&state, &mut parts).await;
    let request = Request::from_parts(parts, body);
    let limit = match client {
        Client::Member(_) => state.config.rate_limit_authenticated,
        Client::Ip(_) => state.config.rate_limit_anonymous,
    };

    let Some(decision) = count(&state.redis, &client, limit).await else {
        return next.run(request).await;
    };
    if !decision.allowed() {
        metrics::counter!(names::RATE_LIMITED_TOTAL, "scope" => client.scope()).increment(1);
        let mut response = ApiError::RateLimited {
            limit,
            retry_after: decision.reset.max(1),
        }
        .into_response();
        decision.apply(response.headers_mut());
        return response;
    }
    let mut response = next.run(request).await;
    decision.apply(response.headers_mut());
    response
}

/// Whether the request carries the configured service key, compared in
/// constant time.
pub fn is_service(state: &AppState, headers: &HeaderMap) -> bool {
    let (Some(key), Some(given)) = (&state.config.service_key, headers.get(SERVICE_KEY)) else {
        return false;
    };
    let (key, given) = (key.expose().as_bytes(), given.as_bytes());
    key.len() == given.len() && key.iter().zip(given).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// The member of a valid token, or else the client IP. The token check is
/// left in the request for [`crate::auth::AuthUser`].
async fn identify(state: &AppState, parts: &mut axum::http::request::Parts) -> Client {
    if let Some(header) = parts.headers.get(header::AUTHORIZATION) {
        let expected = Expected {
            issuer: &state.config.jwt_issuer,
            audience: &state.config.jwt_audience,
        };
        let now = chrono::Utc::now().timestamp();
        let result = auth::authenticate(Some(header.as_bytes()), &state.jwks, expected, now).await;
        let member = result.as_ref().ok().map(|claims| claims.sub.clone());
        parts.extensions.insert(TokenCheck(result));
        if let Some(sub) = member {
            return Client::Member(sub);
        }
    }
    Client::Ip(mw::client_ip(parts).map_or_else(|| "unknown".to_owned(), |ip| ip.to_string()))
}

/// Counts one request for `client`. `None` when Redis cannot answer: the
/// request is then let through.
async fn count(redis: &fred::prelude::Client, client: &Client, limit: u32) -> Option<Decision> {
    hit(redis, &client.key(), limit, WINDOW_SECS).await
}

/// Counts one attempt of a per-action limit (`auth-sessions.md`: login,
/// sign-up), in a window of `window_secs` started by the first attempt.
/// `None` when Redis cannot answer: the attempt is let through, as for the
/// per-client limits.
pub async fn attempt(
    redis: &fred::prelude::Client,
    action: &str,
    who: &str,
    limit: u32,
    window_secs: i64,
) -> Option<Decision> {
    let key = format!("{KEY_PREFIX}:{action}:{who}");
    hit(redis, &key, limit, window_secs).await
}

async fn hit(
    redis: &fred::prelude::Client,
    key: &str,
    limit: u32,
    window_secs: i64,
) -> Option<Decision> {
    // Without a connection, a command would wait for the reconnection: do
    // not make content requests wait on Redis.
    if !redis.is_connected() {
        redis_failed("not connected");
        return None;
    }
    let pipeline = redis.pipeline();
    let queued: Result<(), fred::error::Error> = async {
        pipeline.incr::<(), _>(key).await?;
        pipeline
            .expire::<(), _>(key, window_secs, Some(ExpireOptions::NX))
            .await?;
        pipeline.ttl::<(), _>(key).await?;
        Ok(())
    }
    .await;
    if let Err(error) = queued {
        redis_failed(&error.to_string());
        return None;
    }
    match tokio::time::timeout(REDIS_TIMEOUT, pipeline.all::<(i64, i64, i64)>()).await {
        Ok(Ok((count, _, ttl))) => Some(Decision {
            limit,
            count: u64::try_from(count).unwrap_or(0),
            // -1 (no expiry) cannot happen after EXPIRE NX; be safe anyway.
            reset: u64::try_from(ttl)
                .ok()
                .filter(|t| *t > 0)
                .unwrap_or(window_secs as u64),
        }),
        Ok(Err(error)) => {
            redis_failed(&error.to_string());
            None
        }
        Err(_) => {
            redis_failed("timeout");
            None
        }
    }
}

/// Unix time of the last warning about Redis, to log at most once a minute
/// while it is down.
static LAST_WARNING: AtomicI64 = AtomicI64::new(0);

fn redis_failed(error: &str) {
    metrics::counter!(names::RATE_LIMIT_ERRORS_TOTAL).increment(1);
    let now = chrono::Utc::now().timestamp();
    let last = LAST_WARNING.load(Ordering::Relaxed);
    if now - last >= WINDOW_SECS
        && LAST_WARNING
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        tracing::warn!(
            error,
            "rate limits unavailable (Redis), requests are not limited"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_count_down_to_zero() {
        let first = Decision {
            limit: 60,
            count: 1,
            reset: 60,
        };
        assert!(first.allowed());
        assert_eq!(first.remaining(), 59);
        let last = Decision { count: 60, ..first };
        assert!(last.allowed());
        assert_eq!(last.remaining(), 0);
        let over = Decision { count: 61, ..first };
        assert!(!over.allowed());
        assert_eq!(over.remaining(), 0);
    }

    #[test]
    fn members_and_ips_have_separate_counters() {
        assert_eq!(
            Client::Member("42".into()).key(),
            "pulsars-api:ratelimit:sub:42"
        );
        assert_eq!(
            Client::Ip("2001:db8::1".into()).key(),
            "pulsars-api:ratelimit:ip:2001:db8::1"
        );
    }

    #[test]
    fn headers_follow_the_draft() {
        let mut headers = HeaderMap::new();
        Decision {
            limit: 60,
            count: 3,
            reset: 41,
        }
        .apply(&mut headers);
        assert_eq!(headers["ratelimit-limit"], "60");
        assert_eq!(headers["ratelimit-remaining"], "57");
        assert_eq!(headers["ratelimit-reset"], "41");
    }
}
