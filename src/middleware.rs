//! HTTP middleware shared by every route.

use std::any::Any;
use std::time::Instant;

use axum::body::Body;
use axum::extract::{MatchedPath, Request};
use axum::http::header::{CONTENT_TYPE, HeaderName};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tower::BoxError;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::{ApiError, PROBLEM_CONTENT_TYPE};
use crate::telemetry::{self, CURRENT_TRACE, TraceContext, names};

pub const TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");
pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub const X_ROBOTS_TAG: HeaderName = HeaderName::from_static("x-robots-tag");

/// Reads (or starts) the W3C trace context and makes it available to the
/// rest of the request: as a request extension for the log span, and as a
/// task-local for error rendering and outgoing calls.
pub async fn trace_context(mut request: Request, next: Next) -> Response {
    let header = request
        .headers()
        .get(TRACEPARENT)
        .and_then(|v| v.to_str().ok());
    let context = TraceContext::from_header(header);
    request.extensions_mut().insert(context.clone());
    CURRENT_TRACE.scope(context, next.run(request)).await
}

/// Counts requests and records their latency, labelled by route template
/// (never the raw path, which would explode the label cardinality).
pub async fn record_metrics(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().as_str().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |p| p.as_str().to_owned());

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    metrics::counter!(names::HTTP_REQUESTS_TOTAL,
        "method" => method.clone(), "route" => route.clone(), "status" => status)
    .increment(1);
    metrics::histogram!(names::HTTP_REQUEST_DURATION_SECONDS,
        "method" => method, "route" => route)
    .record(started.elapsed().as_secs_f64());
    response
}

/// Turns the plain-text errors produced by lower layers (body limit) into
/// problem documents, so every error a client sees has the same shape.
pub async fn problem_for_bare_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let is_problem = response
        .headers()
        .get(CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(PROBLEM_CONTENT_TYPE.as_bytes()));
    if is_problem {
        return response;
    }
    match response.status() {
        StatusCode::PAYLOAD_TOO_LARGE => ApiError::PayloadTooLarge.into_response(),
        _ => response,
    }
}

/// Error handler of the timeout layer.
pub async fn handle_timeout(error: BoxError) -> Response {
    if error.is::<tower::timeout::error::Elapsed>() {
        ApiError::Timeout.into_response()
    } else {
        ApiError::Internal(format!("unhandled middleware error: {error}")).into_response()
    }
}

/// Answer for a handler that panicked. The panic itself is logged by the
/// panic hook; the client gets a 500 problem with the trace id.
pub fn panic_response(panic: Box<dyn Any + Send + 'static>) -> Response<Body> {
    let message = panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned());
    ApiError::Internal(format!("handler panicked: {message}")).into_response()
}

/// Headers added to every response.
pub fn static_headers() -> [(HeaderName, HeaderValue); 2] {
    [
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (X_ROBOTS_TAG, HeaderValue::from_static("noindex, nofollow")),
    ]
}

/// CORS for public content endpoints: any origin, read-only, no credentials.
pub fn cors_public() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
        .allow_headers([header::ACCEPT, header::IF_NONE_MATCH, TRACEPARENT])
        .expose_headers([
            header::ETAG,
            X_REQUEST_ID,
            HeaderName::from_static("ratelimit-limit"),
            HeaderName::from_static("ratelimit-remaining"),
            HeaderName::from_static("ratelimit-reset"),
            header::RETRY_AFTER,
        ])
        .max_age(std::time::Duration::from_secs(600))
}

/// CORS for authenticated and game endpoints: only the game front end, with
/// the `Authorization` header.
pub fn cors_restricted(origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            TRACEPARENT,
        ])
        .expose_headers([X_REQUEST_ID, header::RETRY_AFTER])
        .max_age(std::time::Duration::from_secs(600))
}

/// The trace id to put in the request's log span.
pub fn span_trace_id(request: &Request) -> String {
    request
        .extensions()
        .get::<TraceContext>()
        .map(|c| c.trace_id.clone())
        .or_else(telemetry::current_trace_id)
        .unwrap_or_default()
}
