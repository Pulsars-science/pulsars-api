//! Route tree and middleware stack.

pub mod auth;
pub mod courses;
pub mod cursor;
pub mod exams;
pub mod exercises;
pub mod files;
pub mod game;
pub mod health;
pub mod me;
pub mod news;
pub mod params;
pub mod public;
pub mod resources;
pub mod token;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use tower::ServiceBuilder;
use tower_http::LatencyUnit;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::error::ApiError;
use crate::middleware::{self as mw, X_REQUEST_ID};
use crate::rate_limit;
use crate::state::AppState;

/// Builds the public application.
pub fn router(state: AppState) -> Router {
    let config = state.config.clone();
    let [(nosniff_name, nosniff), (robots_name, robots)] = mw::static_headers();

    // Public content: any origin, no credentials. The explicit OPTIONS
    // handlers route preflights through the CORS layer, which answers them;
    // without them the router's 405 fallback would.
    let content = Router::new()
        .route("/v1/catalog", get(courses::catalog).options(preflight))
        .route("/v1/courses", get(courses::list).options(preflight))
        .route("/v1/courses/{slug}", get(courses::show).options(preflight))
        .route(
            "/v1/courses/{slug}/lessons/{lesson}",
            get(courses::lesson).options(preflight),
        )
        .route("/v1/exercises", get(exercises::list).options(preflight))
        .route(
            "/v1/exercises/{id}",
            get(exercises::show).options(preflight),
        )
        .route("/v1/exams", get(exams::list).options(preflight))
        .route("/v1/exams/facets", get(exams::facets).options(preflight))
        .route("/v1/exams/{slug}", get(exams::show).options(preflight))
        .route("/v1/news", get(news::list).options(preflight))
        .route("/v1/news/{slug}", get(news::show).options(preflight))
        .route("/v1/resources", get(resources::list).options(preflight))
        .route(
            "/v1/resources/{slug}",
            get(resources::show).options(preflight),
        )
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(mw::cors_public());

    // Stored files: public like content, but not JSON (no `Accept` check).
    let files = Router::new()
        .route(
            "/v1/exams/{slug}/pdf",
            get(files::exam_pdf).options(preflight),
        )
        .route(
            "/v1/resources/{slug}/file",
            get(files::resource_file).options(preflight),
        )
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(mw::cors_public());

    // The leaderboard is public, like content.
    let game_public = Router::new()
        .route(
            "/v1/game/leaderboard",
            get(game::leaderboard).options(preflight),
        )
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(mw::cors_public());

    // The match WebSocket checks its `Origin` itself, before the upgrade.
    let game_ws = Router::new()
        .route("/v1/game/ws", get(game::ws))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // Authenticated endpoints: only the game front end may call them from a
    // browser. Without `Origin` (server to server: the Laravel admin
    // dashboard proxies the statistics) the CORS layer stays out of the way.
    let authenticated = Router::new()
        .route("/v1/me", get(me::me).options(preflight))
        .route("/v1/game/me", get(game::me).options(preflight))
        .route(
            "/v1/game/admin/stats",
            get(game::admin_stats).options(preflight),
        )
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(mw::cors_restricted(config.cors_allowed_origins.clone()));

    // Accounts and sessions: called by the site from its server, never by a
    // browser (no CORS layer: a cross-origin browser request is refused).
    let auth = Router::new()
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/session", get(auth::session))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/sessions", post(auth::open_for))
        .route("/v1/auth/password", put(auth::change_password))
        .route("/v1/auth/password/reset", post(auth::reset_password))
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // The game token, asked by the game front end with the session cookie:
    // its own origin check and credentialed CORS (`token::browser_guard`).
    let game_token = Router::new()
        .route("/v1/auth/token", post(token::issue).options(preflight))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(from_fn_with_state(state.clone(), token::browser_guard));

    // The API's public signing key, for anyone verifying its tokens.
    let jwks = Router::new()
        .route("/.well-known/jwks.json", get(token::jwks))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    let routes = Router::new()
        .route("/healthz", get(health::healthz))
        .merge(auth)
        .merge(game_token)
        .merge(jwks)
        .merge(content)
        .merge(files)
        .merge(game_public)
        .merge(game_ws)
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
                // PDF are compressed already, and a compressed response
                // loses `Accept-Ranges`.
                .layer(CompressionLayer::new().compress_when(
                    DefaultPredicate::new().and(NotForContentType::const_new("application/pdf")),
                ))
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
