//! Route tree and middleware stack.

pub mod account;
pub mod admin;
pub mod auth;
pub mod corrections;
pub mod courses;
pub mod cursor;
pub mod dashboard;
pub mod exams;
pub mod exercises;
pub mod feedback;
pub mod files;
pub mod forum;
pub mod forum_moderation;
pub mod game;
pub mod health;
pub mod languages;
pub mod learning;
pub mod me;
pub mod news;
pub mod oauth;
pub mod params;
pub mod practice;
pub mod profile;
pub mod public;
pub mod resources;
pub mod site;
pub mod social;
pub mod telemetry;
pub mod token;
pub mod upload;
pub mod views;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::extract::{OriginalUri, Request};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use tower::ServiceBuilder;
use tower_http::LatencyUnit;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
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
        .route("/v1/languages", get(languages::list).options(preflight))
        .route(
            "/v1/languages/{code}",
            get(languages::show).options(preflight),
        )
        .route(
            "/v1/languages/{code}/levels/{level}",
            get(languages::level).options(preflight),
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
            "/v1/exams/{slug}/correction-pdf",
            get(files::exam_correction_pdf).options(preflight),
        )
        .route(
            "/v1/resources/{slug}/file",
            get(files::resource_file).options(preflight),
        )
        .route(
            "/v1/storage/{*path}",
            get(profile::storage).options(preflight),
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

    // What learners do in the languages section, sent by its front end
    // (pulsars.science/langues): only that origin may post from a browser.
    let languages_events = Router::new()
        .route(
            "/v1/languages/events",
            axum::routing::post(languages::record_event).options(preflight),
        )
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
        .layer(mw::cors_restricted(config.cors_allowed_origins.clone()));

    // The match WebSocket checks its `Origin` itself, before the upgrade.
    let game_ws = Router::new()
        .route("/v1/game/ws", get(game::ws))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // Authenticated endpoints: only the game front end may call them from a
    // browser. Without `Origin` (server to server) the CORS layer stays out
    // of the way.
    let authenticated = Router::new()
        .route("/v1/me", get(me::me).options(preflight))
        .route("/v1/game/me", get(game::me).options(preflight))
        .route(
            "/v1/game/admin/stats",
            get(game::admin_stats).options(preflight),
        )
        .route(
            "/v1/languages/admin/stats",
            get(languages::admin_stats).options(preflight),
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
        .route("/v1/auth/password/forgot", post(account::forgot))
        .route(
            "/v1/auth/password/reset-with-token",
            post(account::reset_with_token),
        )
        .route("/v1/auth/email/verification", post(account::send_again))
        .route("/v1/auth/email/verify", post(account::verify))
        .route("/v1/auth/oauth/resolve", post(oauth::resolve))
        .route("/v1/auth/oauth/{provider}", delete(oauth::unlink))
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // The forum and its moderation: called by the site (pulsars-web), from
    // its server or through its `/api` relay, never by a browser directly.
    let forum = Router::new()
        .route("/v1/forum/themes", get(forum::themes))
        .route("/v1/forum/themes/{slug}", get(forum::theme))
        .route(
            "/v1/forum/questions",
            get(forum::questions).post(forum::ask),
        )
        .route("/v1/forum/questions/{slug}", get(forum::question))
        .route("/v1/forum/questions/{slug}/answers", post(forum::answer))
        .route(
            "/v1/forum/questions/{slug}/answers/{answer}/accept",
            post(forum::accept),
        )
        .route(
            "/v1/forum/questions/{slug}/vote",
            post(forum::vote_question),
        )
        .route(
            "/v1/forum/questions/{slug}/translations",
            post(forum::translate_question),
        )
        .route("/v1/forum/answers/{id}/vote", post(forum::vote_answer))
        .route(
            "/v1/forum/answers/{id}/translations",
            post(forum::translate_answer),
        )
        .route("/v1/forum/reports", post(forum::report))
        .route(
            "/v1/moderation/forum/questions/{id}",
            delete(forum_moderation::delete_question),
        )
        .route(
            "/v1/moderation/forum/questions/{id}/hide",
            post(forum_moderation::hide_question),
        )
        .route(
            "/v1/moderation/forum/questions/{id}/lock",
            post(forum_moderation::lock_question),
        )
        .route(
            "/v1/moderation/forum/questions/{id}/pin",
            post(forum_moderation::pin_question),
        )
        .route(
            "/v1/moderation/forum/questions/{id}/restore",
            post(forum_moderation::restore_question),
        )
        .route(
            "/v1/moderation/forum/answers/{id}",
            delete(forum_moderation::delete_answer),
        )
        .route(
            "/v1/moderation/forum/answers/{id}/hide",
            post(forum_moderation::hide_answer),
        )
        .route(
            "/v1/moderation/forum/answers/{id}/restore",
            post(forum_moderation::restore_answer),
        )
        .route(
            "/v1/moderation/members/{id}/mute",
            put(forum_moderation::mute).delete(forum_moderation::unmute),
        )
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // What the site's pages need about its members and from them, called by
    // pulsars-web from its server or through its `/api` relay (no CORS
    // layer: never straight from a browser).
    let site = Router::new()
        .route("/v1/me/progress", get(learning::started))
        .route("/v1/me/courses/{slug}/progress", get(learning::progress))
        .route("/v1/me/lessons/{id}/completion", put(learning::complete))
        .route("/v1/me/exercises/{id}", get(practice::mine))
        .route("/v1/exercises/{id}/attempts", post(practice::attempt))
        .route("/v1/exercises/catalog", get(practice::catalog))
        .route("/v1/exercises/random", get(practice::random))
        .route("/v1/exercises/facets", get(practice::facets))
        .route("/v1/me/dashboard", get(dashboard::dashboard))
        .route("/v1/me/brains", get(dashboard::brains_history))
        .route("/v1/leaderboard/xp", get(dashboard::leaderboard))
        .route("/v1/leaderboard/top", get(dashboard::top))
        .route("/v1/rewards", get(dashboard::rewards))
        .route("/v1/me/profile", get(profile::show).patch(profile::update))
        .route("/v1/me/onboarding", post(profile::onboarding))
        .route("/v1/me/locale", put(profile::locale))
        .route("/v1/roles", get(profile::roles))
        .route("/v1/captcha", get(feedback::captcha))
        .route("/v1/reports", post(feedback::report))
        .route("/v1/licenses", get(feedback::licenses))
        .route("/v1/settings/donation-goal", get(feedback::donation_goal))
        .route(
            "/v1/exams/{slug}/corrections",
            get(corrections::corrections),
        )
        .route("/v1/courses/{slug}/view", post(views::course))
        .route("/v1/telemetry/page-view", post(telemetry::page_view))
        .route("/v1/consent", post(telemetry::consent))
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    // The members' uploads (`multipart/form-data`): their body limit is
    // `upload::UPLOAD_BODY_LIMIT` (`mw::body_limit`).
    let uploads = Router::new()
        .route(
            "/v1/me/avatar",
            put(profile::put_avatar).delete(profile::delete_avatar),
        )
        .route("/v1/suggestions", post(feedback::suggestion))
        .route("/v1/resource-requests", post(feedback::resource_request))
        .layer(DefaultBodyLimit::max(upload::UPLOAD_BODY_LIMIT))
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

    // The administration (`admin`): called by the site from its server, like
    // `/v1/auth/*`. Its two uploads have their own body limits, Laravel's
    // (`mw::body_limit`).
    let admin_json = Router::new()
        .route("/v1/admin/stats", get(admin::dashboard::dashboard))
        .route("/v1/admin/pending", get(admin::dashboard::pending))
        .route("/v1/admin/containers", get(admin::containers::containers))
        .route(
            "/v1/admin/settings/donation-goal",
            get(admin::dashboard::donation_goal).put(admin::dashboard::set_donation_goal),
        )
        .route("/v1/admin/game/stats", get(admin::dashboard::game_stats))
        .route(
            "/v1/admin/game/leaderboard",
            get(admin::dashboard::game_leaderboard),
        )
        .route(
            "/v1/admin/languages/stats",
            get(admin::dashboard::language_stats),
        )
        .route(
            "/v1/admin/news",
            get(admin::news::list).post(admin::news::create),
        )
        .route(
            "/v1/admin/news/{slug}",
            put(admin::news::update).delete(admin::news::delete),
        )
        .route("/v1/admin/news/{slug}/publish", post(admin::news::publish))
        .route(
            "/v1/admin/news/{slug}/unpublish",
            post(admin::news::unpublish),
        )
        .route(
            "/v1/admin/courses",
            get(admin::courses::list).post(admin::courses::create),
        )
        .route("/v1/admin/courses/{slug}", delete(admin::courses::delete))
        .route(
            "/v1/admin/courses/{slug}/publish",
            post(admin::courses::publish),
        )
        .route(
            "/v1/admin/courses/{slug}/unpublish",
            post(admin::courses::unpublish),
        )
        .route(
            "/v1/admin/courses/{slug}/archive",
            post(admin::courses::archive),
        )
        .route("/v1/admin/exams", get(admin::exams::list))
        .route("/v1/admin/exams/{slug}", delete(admin::exams::delete))
        .route(
            "/v1/admin/exams/{slug}/publish",
            post(admin::exams::publish),
        )
        .route(
            "/v1/admin/exams/{slug}/unpublish",
            post(admin::exams::unpublish),
        )
        .route("/v1/admin/resource-requests", get(admin::requests::list))
        .route(
            "/v1/admin/resource-requests/{id}/accept",
            post(admin::requests::accept),
        )
        .route(
            "/v1/admin/resource-requests/{id}/reject",
            post(admin::requests::reject),
        )
        .route("/v1/admin/members", get(admin::roles::list))
        .route(
            "/v1/admin/members/{id}/roles/{slug}",
            put(admin::roles::grant).delete(admin::roles::revoke),
        )
        .route("/v1/admin/reports", get(admin::queues::reports))
        .route("/v1/admin/reports/{id}", patch(admin::queues::file_report))
        .route("/v1/admin/suggestions", get(admin::queues::suggestions))
        .route(
            "/v1/admin/suggestions/{id}",
            patch(admin::queues::file_suggestion),
        )
        .layer(from_fn(public::require_json));
    // PDF, not JSON: no `Accept` check.
    let admin_files = Router::new()
        .route("/v1/admin/exams/{slug}/pdf", get(admin::exams::pdf))
        .route(
            "/v1/admin/resource-requests/{id}/file",
            get(admin::requests::preview),
        );
    let admin_upload = Router::new()
        .route("/v1/admin/exams", post(admin::exams::create))
        .layer(from_fn(public::require_json))
        .layer(DefaultBodyLimit::disable());
    let admin_import = Router::new()
        .route("/v1/admin/exams/{slug}/import", post(admin::exams::import))
        .layer(from_fn(public::require_json))
        .layer(DefaultBodyLimit::max(admin::exams::IMPORT_BODY_LIMIT));
    let admin_uploads = admin_upload
        .merge(admin_import)
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));
    let admin = admin_json
        .merge(admin_files)
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit));

    let routes = Router::new()
        .route("/healthz", get(health::healthz))
        .merge(admin)
        .merge(auth)
        .merge(forum)
        .merge(game_token)
        .merge(jwks)
        .merge(content)
        .merge(files)
        .merge(game_public)
        .merge(game_ws)
        .merge(languages_events)
        .merge(authenticated)
        .merge(social::router(&state))
        .merge(admin_uploads)
        .merge(site)
        .merge(uploads)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        // Per-route layer: sees the matched route template.
        .layer(from_fn_with_state(
            state.http_daily.clone(),
            mw::record_metrics,
        ));

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
                .layer(from_fn_with_state(state.clone(), mw::body_limit)),
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
