//! Route tree and middleware stack.

pub mod cursor;
pub mod health;
pub mod me;
pub mod params;
pub mod public;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::from_fn;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::LatencyUnit;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::error::ApiError;
use crate::middleware::{self as mw, X_REQUEST_ID};
use crate::state::AppState;

/// Builds the public application.
pub fn router(state: AppState) -> Router {
    let config = state.config.clone();
    let [(nosniff_name, nosniff), (robots_name, robots)] = mw::static_headers();

    // Authenticated endpoints: only the game front end may call them from a
    // browser. Content endpoints (phase 3) will get `mw::cors_public()`.
    // The explicit OPTIONS handler routes preflights through the CORS layer,
    // which answers them; without it the router's 405 fallback would.
    let authenticated = Router::new()
        .route("/v1/me", get(me::me).options(preflight))
        .layer(mw::cors_restricted(config.cors_allowed_origins.clone()));

    let routes = Router::new()
        .route("/healthz", get(health::healthz))
        .merge(authenticated)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        // Per-route layer: sees the matched route template.
        .layer(from_fn(mw::record_metrics));

    routes
        .layer(
            // Listed from the outermost layer to the innermost.
            ServiceBuilder::new()
                .layer(CatchPanicLayer::custom(mw::panic_response))
                .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
                .layer(from_fn(mw::trace_context))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request| {
                            let request_id = request
                                .headers()
                                .get(X_REQUEST_ID)
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or_default()
                                .to_owned();
                            tracing::info_span!(
                                "request",
                                method = %request.method(),
                                path = %request.uri().path(),
                                request_id = %request_id,
                                trace_id = %mw::span_trace_id(request),
                            )
                        })
                        .on_response(
                            DefaultOnResponse::new()
                                .level(Level::INFO)
                                .latency_unit(LatencyUnit::Millis),
                        ),
                )
                .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
                .layer(SetResponseHeaderLayer::overriding(nosniff_name, nosniff))
                .layer(SetResponseHeaderLayer::overriding(robots_name, robots))
                .layer(CompressionLayer::new())
                .layer(HandleErrorLayer::new(mw::handle_timeout))
                .timeout(config.request_timeout)
                .layer(from_fn(mw::problem_for_bare_errors))
                .layer(RequestBodyLimitLayer::new(config.body_limit_bytes)),
        )
        .with_state(state)
}

/// Router of the internal metrics listener.
pub fn metrics_router(handle: metrics_exporter_prometheus::PrometheusHandle) -> Router {
    Router::new().route(
        "/metrics",
        get(move || {
            let handle = handle.clone();
            async move {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        HeaderValue::from_static("text/plain; version=0.0.4"),
                    )],
                    handle.render(),
                )
            }
        }),
    )
}

async fn not_found(OriginalUri(uri): OriginalUri) -> Response {
    ApiError::NotFound {
        detail: "No resource exists at this path.".to_owned(),
        instance: Some(uri.path().to_owned()),
    }
    .into_response()
}

/// Reached only by an OPTIONS request that is not a CORS preflight.
async fn preflight() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn method_not_allowed() -> Response {
    let mut response = ApiError::MethodNotAllowed.into_response();
    *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    response
}
