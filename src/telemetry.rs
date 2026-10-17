//! Logs, trace context and metrics.
//!
//! - Logs are JSON lines (or human-readable in development) written by
//!   `tracing-subscriber`.
//! - Every request carries a W3C Trace Context (`traceparent`): taken from the
//!   caller when valid, created otherwise. Its trace id is attached to every
//!   log line of the request and to 500 error bodies, and will be forwarded
//!   on outgoing calls (webhooks).
//! - Metrics are recorded with the `metrics` facade and rendered by the
//!   Prometheus exporter on a separate, internal listener.

use std::fmt::Write as _;

use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::LogFormat;

/// Installs the global log subscriber. Call once, before anything logs.
pub fn init_logging(format: LogFormat, filter: &str) -> Result<(), String> {
    let filter = EnvFilter::try_new(filter).map_err(|e| format!("RUST_LOG: {e}"))?;
    let registry = tracing_subscriber::registry().with(filter);
    let result = match format {
        LogFormat::Json => registry
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .try_init(),
        LogFormat::Pretty => registry.with(tracing_subscriber::fmt::layer()).try_init(),
    };
    result.map_err(|e| format!("cannot install the log subscriber: {e}"))
}

/// W3C Trace Context of the current request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    /// 32 lowercase hex characters.
    pub trace_id: String,
    /// The caller's span id, when the request carried a valid `traceparent`.
    pub parent_id: Option<String>,
    /// Span id of this service's handling of the request (16 hex characters).
    pub span_id: String,
    pub sampled: bool,
}

impl TraceContext {
    /// Continues the caller's trace if `header` is a valid `traceparent`,
    /// starts a new one otherwise. Invalid headers are ignored, as the
    /// specification requires.
    pub fn from_header(header: Option<&str>) -> Self {
        header.and_then(Self::parse).unwrap_or_else(Self::new_root)
    }

    fn parse(header: &str) -> Option<Self> {
        let header = header.trim();
        let mut parts = header.split('-');
        let version = parts.next()?;
        let trace_id = parts.next()?;
        let parent_id = parts.next()?;
        let flags = parts.next()?;
        let rest = parts.next();

        if !is_lower_hex(version, 2) || version == "ff" {
            return None;
        }
        // Version 00 has exactly four fields; later versions may append more.
        if version == "00" && rest.is_some() {
            return None;
        }
        if !is_lower_hex(trace_id, 32) || trace_id.bytes().all(|b| b == b'0') {
            return None;
        }
        if !is_lower_hex(parent_id, 16) || parent_id.bytes().all(|b| b == b'0') {
            return None;
        }
        if !is_lower_hex(flags, 2) {
            return None;
        }
        let flags = u8::from_str_radix(flags, 16).ok()?;

        Some(Self {
            trace_id: trace_id.to_owned(),
            parent_id: Some(parent_id.to_owned()),
            span_id: new_span_id(),
            sampled: flags & 0x01 == 0x01,
        })
    }

    fn new_root() -> Self {
        Self {
            trace_id: hex(uuid::Uuid::new_v4().as_bytes()),
            parent_id: None,
            span_id: new_span_id(),
            sampled: false,
        }
    }

    /// The `traceparent` value to send on outgoing requests made while
    /// handling this one: same trace, this service's span as parent.
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{}",
            self.trace_id,
            self.span_id,
            if self.sampled { "01" } else { "00" }
        )
    }
}

tokio::task_local! {
    /// The trace context of the request being handled, readable from any
    /// code running inside the request (error rendering, outgoing calls).
    pub static CURRENT_TRACE: TraceContext;
}

/// Trace id of the request being handled, if any.
pub fn current_trace_id() -> Option<String> {
    CURRENT_TRACE.try_with(|ctx| ctx.trace_id.clone()).ok()
}

fn new_span_id() -> String {
    let bytes = uuid::Uuid::new_v4();
    let id = hex(&bytes.as_bytes()[..8]);
    // An all-zero span id is invalid; the odds are 2^-64, but cheap to rule out.
    if id.bytes().all(|b| b == b'0') {
        "0000000000000001".to_owned()
    } else {
        id
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Metric names, kept in one place so dashboards and code agree.
pub mod names {
    pub const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
    pub const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";
    pub const HEALTH_CHECK_UP: &str = "health_check_up";
    pub const AUTH_REJECTIONS_TOTAL: &str = "auth_rejections_total";
    pub const JWKS_FETCHES_TOTAL: &str = "jwks_fetches_total";
    pub const JWKS_KEYS: &str = "jwks_keys";
    pub const RATE_LIMITED_TOTAL: &str = "rate_limited_total";
    pub const RATE_LIMIT_ERRORS_TOTAL: &str = "rate_limit_errors_total";
    pub const GAME_CONNECTIONS_TOTAL: &str = "game_connections_total";
    pub const GAME_AUTH_FAILURES_TOTAL: &str = "game_auth_failures_total";
    pub const GAME_SESSIONS: &str = "game_sessions";
    pub const GAME_QUEUED: &str = "game_queued_players";
    pub const GAME_MATCHES_LIVE: &str = "game_matches_live";
    pub const GAME_MATCHES_STARTED_TOTAL: &str = "game_matches_started_total";
    pub const GAME_MATCHES_ENDED_TOTAL: &str = "game_matches_ended_total";
    pub const GAME_PERSIST_ERRORS_TOTAL: &str = "game_persist_errors_total";
    pub const GAME_SEASONS_CLOSED_TOTAL: &str = "game_seasons_closed_total";
    pub const WEBHOOK_ATTEMPTS_TOTAL: &str = "webhook_attempts_total";
    pub const WEBHOOK_PENDING: &str = "webhook_pending";
    pub const WEBHOOK_DEAD_TOTAL: &str = "webhook_dead_total";
    pub const WEBHOOK_LATENCY_SECONDS: &str = "webhook_latency_seconds";
}

/// Installs the global metrics recorder and returns the handle that renders
/// the Prometheus text format.
pub fn install_metrics_recorder() -> Result<PrometheusHandle, BuildError> {
    let handle = build_metrics_recorder()?;
    describe_metrics();
    Ok(handle)
}

fn build_metrics_recorder() -> Result<PrometheusHandle, BuildError> {
    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(names::HTTP_REQUEST_DURATION_SECONDS.to_owned()),
            &[
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        )?
        .install_recorder()
}

fn describe_metrics() {
    metrics::describe_counter!(
        names::HTTP_REQUESTS_TOTAL,
        "HTTP requests handled, by method, route template and status."
    );
    metrics::describe_histogram!(
        names::HTTP_REQUEST_DURATION_SECONDS,
        metrics::Unit::Seconds,
        "HTTP request latency, by method and route template."
    );
    metrics::describe_gauge!(
        names::HEALTH_CHECK_UP,
        "1 when the dependency answered the last health check, 0 otherwise."
    );
    metrics::describe_counter!(
        names::AUTH_REJECTIONS_TOTAL,
        "Requests refused by token verification, by reason (no_keys: JWKS unusable)."
    );
    metrics::describe_counter!(
        names::JWKS_FETCHES_TOTAL,
        "JWKS fetches, by result (ok, error)."
    );
    metrics::describe_gauge!(names::JWKS_KEYS, "Usable keys in the last JWKS loaded.");
    metrics::describe_counter!(
        names::RATE_LIMITED_TOTAL,
        "Requests refused with 429, by scope (ip, sub)."
    );
    metrics::describe_counter!(
        names::RATE_LIMIT_ERRORS_TOTAL,
        "Requests let through unlimited because Redis could not count them."
    );
    metrics::describe_counter!(
        names::GAME_CONNECTIONS_TOTAL,
        "Game WebSocket connections opened."
    );
    metrics::describe_counter!(
        names::GAME_AUTH_FAILURES_TOTAL,
        "Game WebSockets closed during authentication, by close code."
    );
    metrics::describe_gauge!(names::GAME_SESSIONS, "Authenticated game sessions.");
    metrics::describe_gauge!(names::GAME_QUEUED, "Players waiting in matchmaking.");
    metrics::describe_gauge!(names::GAME_MATCHES_LIVE, "Matches in progress.");
    metrics::describe_counter!(
        names::GAME_MATCHES_STARTED_TOTAL,
        "Matches started, by mode (duel, room, ghost)."
    );
    metrics::describe_counter!(
        names::GAME_MATCHES_ENDED_TOTAL,
        "Matches ended, by mode and end reason."
    );
    metrics::describe_counter!(
        names::GAME_PERSIST_ERRORS_TOTAL,
        "Match ends that could not be written to the database."
    );
    metrics::describe_counter!(
        names::GAME_SEASONS_CLOSED_TOTAL,
        "Monthly duel seasons closed, by subject key."
    );
    metrics::describe_counter!(
        names::WEBHOOK_ATTEMPTS_TOTAL,
        "Webhook delivery attempts, by status (sent, rejected, failed)."
    );
    metrics::describe_gauge!(
        names::WEBHOOK_PENDING,
        "Webhook outbox rows waiting to be sent."
    );
    metrics::describe_counter!(
        names::WEBHOOK_DEAD_TOTAL,
        "Webhook events given up (dead letters)."
    );
    metrics::describe_histogram!(
        names::WEBHOOK_LATENCY_SECONDS,
        metrics::Unit::Seconds,
        "Duration of webhook delivery attempts."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    #[test]
    fn continues_a_valid_trace() {
        let ctx = TraceContext::from_header(Some(VALID));
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.parent_id.as_deref(), Some("00f067aa0ba902b7"));
        assert!(ctx.sampled);
        assert_ne!(ctx.span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.span_id.len(), 16);
    }

    #[test]
    fn outgoing_header_keeps_the_trace_and_uses_our_span() {
        let ctx = TraceContext::from_header(Some(VALID));
        let out = ctx.to_traceparent();
        assert!(out.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"));
        assert!(out.ends_with("-01"));
        assert!(out.contains(&ctx.span_id));
    }

    #[test]
    fn starts_a_new_trace_for_invalid_headers() {
        for bad in [
            "",
            "garbage",
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra",
            "00-4bf92f3577b34da6a3ce929d0e0e473-00f067aa0ba902b7-01",
        ] {
            let ctx = TraceContext::from_header(Some(bad));
            assert_eq!(ctx.parent_id, None, "accepted {bad:?}");
            assert_eq!(ctx.trace_id.len(), 32);
            assert_ne!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        }
    }

    #[test]
    fn accepts_future_versions_with_extra_fields() {
        let ctx = TraceContext::from_header(Some(
            "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-whatever",
        ));
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
    }

    #[test]
    fn new_roots_are_unique() {
        let a = TraceContext::from_header(None);
        let b = TraceContext::from_header(None);
        assert_ne!(a.trace_id, b.trace_id);
        assert!(!a.sampled);
    }
}
