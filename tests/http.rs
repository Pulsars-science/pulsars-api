//! End-to-end tests of the HTTP layer.
//!
//! Tests that need real services read `TEST_DATABASE_URL` (connecting as
//! `pulsars_api`, after `scripts/dev-db-init.sh`) and `TEST_REDIS_URL`. When
//! they are absent the tests are skipped with a message, unless
//! `REQUIRE_INTEGRATION=1` (set in CI), in which case they fail.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use ed25519_dalek::SigningKey;
use pulsars_api::auth::token::testing;
use pulsars_api::auth::{Jwks, JwksSettings};
use pulsars_api::config::Config;
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes};
use serde_json::{Value, json};
use tower::ServiceExt as _;

/// Nothing listens on port 1: connections are refused immediately.
const DEAD_POSTGRES: &str = "postgres://pulsars_api:x@127.0.0.1:1/pulsars";
const DEAD_REDIS: &str = "redis://127.0.0.1:1";
const DEAD_JWKS: &str = "http://127.0.0.1:1/.well-known/jwks.json";

fn integration_urls() -> Option<(String, String)> {
    let db = std::env::var("TEST_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty());
    let redis = std::env::var("TEST_REDIS_URL")
        .ok()
        .filter(|v| !v.is_empty());
    match (db, redis) {
        (Some(db), Some(redis)) => Some((db, redis)),
        _ if std::env::var("REQUIRE_INTEGRATION").as_deref() == Ok("1") => {
            panic!("REQUIRE_INTEGRATION=1 but TEST_DATABASE_URL or TEST_REDIS_URL is not set")
        }
        _ => {
            eprintln!("skipped: TEST_DATABASE_URL and TEST_REDIS_URL are not set");
            None
        }
    }
}

fn config(database_url: &str, redis_url: &str, extra: &[(&str, &str)]) -> Config {
    let mut vars = vec![
        ("PULSARS_ENV".to_owned(), "development".to_owned()),
        ("DATABASE_URL".to_owned(), database_url.to_owned()),
        ("REDIS_URL".to_owned(), redis_url.to_owned()),
        ("JWKS_URL".to_owned(), DEAD_JWKS.to_owned()),
    ];
    vars.extend(
        extra
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
    );
    Config::from_vars(vars).expect("valid test config")
}

/// Short timeouts, production intervals otherwise.
fn jwks_settings() -> JwksSettings {
    JwksSettings {
        fetch_timeout: Duration::from_secs(2),
        ..JwksSettings::default()
    }
}

/// The application, with a JWKS client for `config.jwks_url` that has not
/// fetched anything yet.
async fn app(config: Config) -> (Router, AppState) {
    let jwks = Jwks::with_settings(&config.jwks_url, false, jwks_settings()).unwrap();
    app_with(config, jwks)
}

fn app_with(config: Config, jwks: Jwks) -> (Router, AppState) {
    let state = AppState {
        db: db::connect_lazy(&config).unwrap(),
        redis: redis::connect(&config).unwrap(),
        config: Arc::new(config),
        jwks,
    };
    (routes::router(state.clone()), state)
}

/// The application with its JWKS loaded from a fresh [`JwksServer`].
async fn app_with_keys(database_url: &str, redis_url: &str) -> (Router, AppState, JwksServer) {
    let server = JwksServer::start().await;
    let (app, state) = app(config(
        database_url,
        redis_url,
        &[("JWKS_URL", server.url.as_str())],
    ))
    .await;
    state.jwks.refresh().await.expect("JWKS loads");
    (app, state, server)
}

/// Laravel's JWKS endpoint, served in-process. Starts with one key, `KID`.
struct JwksServer {
    url: String,
    key: SigningKey,
    shared: Arc<JwksShared>,
}

struct JwksShared {
    document: Mutex<Value>,
    status: AtomicU16,
    hits: AtomicUsize,
}

const KID: &str = "2026-10-ed25519-1";

impl JwksServer {
    async fn start() -> Self {
        let key = testing::signing_key();
        let shared = Arc::new(JwksShared {
            document: Mutex::new(json!({"keys": [testing::jwk(&key, KID)]})),
            status: AtomicU16::new(200),
            hits: AtomicUsize::new(0),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/.well-known/jwks.json",
            listener.local_addr().unwrap()
        );
        let router = Router::new()
            .route(
                "/.well-known/jwks.json",
                axum::routing::get(|State(shared): State<Arc<JwksShared>>| async move {
                    shared.hits.fetch_add(1, Ordering::SeqCst);
                    let status =
                        StatusCode::from_u16(shared.status.load(Ordering::SeqCst)).unwrap();
                    let body = shared.document.lock().unwrap().to_string();
                    (
                        status,
                        [(header::CONTENT_TYPE, "application/jwk-set+json")],
                        body,
                    )
                }),
            )
            .with_state(shared.clone());
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { url, key, shared }
    }

    fn set_document(&self, document: Value) {
        *self.shared.document.lock().unwrap() = document;
    }

    fn set_status(&self, status: u16) {
        self.shared.status.store(status, Ordering::SeqCst);
    }

    fn hits(&self) -> usize {
        self.shared.hits.load(Ordering::SeqCst)
    }

    /// A valid token for user `sub`, signed with the served key.
    fn token(&self, sub: &str) -> String {
        testing::token(&self.key, KID, sub, now())
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn get_with_token(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn check<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()
}

#[tokio::test]
async fn healthz_is_down_when_postgres_is_unreachable() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let (status, headers, body) = send(&app, get("/healthz")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "down");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    let postgres = check(&body, "postgres");
    assert_eq!(postgres["ok"], false);
    // The pool retries refused connections until the 1 s check timeout.
    assert!(
        ["timeout", "connection_refused"].contains(&postgres["error"].as_str().unwrap()),
        "{postgres}"
    );
    assert_eq!(check(&body, "redis")["ok"], false);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn healthz_never_leaks_connection_details() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let (_, _, body) = send(&app, get("/healthz")).await;
    let text = body.to_string();
    assert!(!text.contains("127.0.0.1"));
    assert!(!text.contains("pulsars_api"));
}

#[tokio::test]
async fn every_response_carries_the_common_headers() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let (_, headers, _) = send(&app, get("/healthz")).await;
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-robots-tag"], "noindex, nofollow");
    let request_id = headers["x-request-id"].to_str().unwrap();
    assert_eq!(request_id.len(), 36, "a UUID is generated");
}

#[tokio::test]
async fn a_client_request_id_is_echoed() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let request = Request::get("/healthz")
        .header("x-request-id", "abc-123")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .body(Body::empty())
        .unwrap();
    let (_, headers, _) = send(&app, request).await;
    assert_eq!(headers["x-request-id"], "abc-123");
}

#[tokio::test]
async fn unknown_paths_answer_a_problem_document() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let (status, headers, body) = send(&app, get("/v1/nothing-here")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/not-found"
    );
    assert_eq!(body["status"], 404);
    assert_eq!(body["instance"], "/v1/nothing-here");
}

#[tokio::test]
async fn wrong_methods_answer_405_with_allow() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let request = Request::delete("/healthz").body(Body::empty()).unwrap();
    let (status, headers, body) = send(&app, request).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(headers.contains_key(header::ALLOW));
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/method-not-allowed"
    );
}

#[tokio::test]
async fn oversized_bodies_are_refused_with_a_problem() {
    let (app, _) = app(config(
        DEAD_POSTGRES,
        DEAD_REDIS,
        &[("BODY_LIMIT_BYTES", "1024")],
    ))
    .await;
    let request = Request::post("/healthz")
        .header(header::CONTENT_LENGTH, "4096")
        .body(Body::from(vec![b'x'; 4096]))
        .unwrap();
    let (status, headers, body) = send(&app, request).await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(body["status"], 413);
}

#[tokio::test]
async fn metrics_router_renders_prometheus_text() {
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();
    let app = routes::metrics_router(handle);
    let response = app.oneshot(get("/metrics")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
}

// --- Real PostgreSQL and Redis -------------------------------------------

#[tokio::test]
async fn healthz_is_ok_with_real_services() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let (app, state, _jwks) = app_with_keys(&database_url, &redis_url).await;
    // The Redis connection is established in the background.
    tokio::time::timeout(Duration::from_secs(5), async {
        use fred::prelude::ClientLike as _;
        state.redis.wait_for_connect().await
    })
    .await
    .expect("redis connects within 5 s")
    .expect("redis connects");

    let (status, _, body) = send(&app, get("/healthz")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn healthz_is_degraded_without_redis() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    let (app, _, _jwks) = app_with_keys(&database_url, DEAD_REDIS).await;
    let (status, _, body) = send(&app, get("/healthz")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "degraded");
    assert_eq!(check(&body, "postgres")["ok"], true);
}

#[tokio::test]
async fn the_api_role_reads_views_but_not_site_tables() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();

    let denied = sqlx::query("SELECT 1 FROM site.courses LIMIT 1")
        .execute(&pool)
        .await
        .expect_err("pulsars_api must not read site");
    assert!(denied.to_string().contains("permission denied"), "{denied}");

    let courses: i64 = sqlx::query_scalar("SELECT count(*) FROM api_read.courses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(courses > 0, "demo data is loaded");

    let draft: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM api_read.courses WHERE slug = 'demo-draft-course'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(draft, 0, "drafts are invisible");

    sqlx::query("CREATE TABLE IF NOT EXISTS game.write_check (id int)")
        .execute(&pool)
        .await
        .expect("pulsars_api owns the game schema");
    sqlx::query("DROP TABLE game.write_check")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn no_answer_or_private_data_reaches_the_views() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let views: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.views WHERE table_schema = 'api_read'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(views.len() >= 18, "views: {views:?}");

    let mut dump = String::new();
    for view in &views {
        // View names come from the catalogue, not from user input.
        let rows: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT string_agg(row_to_json(t)::text, '\n') FROM api_read.\"{view}\" t"
        )))
        .fetch_one(&pool)
        .await
        .unwrap();
        dump.push_str(&rows.unwrap_or_default());
    }

    // Markers planted in dev/db/30-demo-data.sql.
    for marker in [
        "SOLUTION-MARKER",
        "CORRIGE-MARKER",
        "PROMPT-MARKER",
        "example.invalid",
        "demofriendtoken",
        "not-a-real-hash",
        "Terminale B (private)",
        "correctes",
        "acceptees",
        "demo-draft-course",
        "demo-future-course",
        "demo-pending-resource",
        "-corrige.pdf",
    ] {
        assert!(!dump.contains(marker), "`{marker}` leaked into api_read");
    }
}

// --- Authentication (in-process JWKS, no database needed) -----------------

#[tokio::test]
async fn healthz_is_down_until_the_jwks_is_loaded() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let server = JwksServer::start().await;
    server.set_status(500);
    let (app, state) = app(config(
        &database_url,
        &redis_url,
        &[("JWKS_URL", server.url.as_str())],
    ))
    .await;

    // Laravel down at boot: the process lives on, but is not ready.
    assert!(state.jwks.refresh().await.is_err());
    let (status, _, body) = send(&app, get("/healthz")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["status"], "down");
    assert_eq!(check(&body, "postgres")["ok"], true);

    server.set_status(200);
    state.jwks.refresh().await.expect("JWKS loads");
    let (status, _, body) = send(&app, get("/healthz")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn the_refresh_task_loads_the_keys_once_laravel_answers() {
    let server = JwksServer::start().await;
    server.set_status(503);
    let jwks = Jwks::with_settings(
        &server.url,
        false,
        JwksSettings {
            min_retry_interval: Duration::from_millis(20),
            max_retry_interval: Duration::from_millis(50),
            ..jwks_settings()
        },
    )
    .unwrap();
    let task = jwks.spawn_refresh();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!jwks.is_loaded());
    assert!(server.hits() >= 2, "failures are retried");

    server.set_status(200);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !jwks.is_loaded() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("loaded once the JWKS answers");
    task.abort();
}

#[tokio::test]
async fn me_without_a_token_is_401_with_a_bare_challenge() {
    let (app, _, _jwks) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    let (status, headers, body) = send(&app, get("/v1/me")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        headers[header::WWW_AUTHENTICATE],
        "Bearer realm=\"api.pulsars.science\""
    );
    assert_eq!(
        body,
        json!({
            "type": "https://api.pulsars.science/problems/unauthorized",
            "title": "Invalid token",
            "status": 401,
            "detail": "No bearer token was provided.",
            "instance": "/v1/me",
            "reason": "missing",
        })
    );
}

#[tokio::test]
async fn me_with_an_invalid_token_is_401_invalid_token() {
    let (app, _, server) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    let other = testing::signing_key();
    let expired = testing::token(&server.key, KID, "1", now() - 3600);
    let forged = testing::token(&other, KID, "1", now());
    let none = testing::sign(
        &server.key,
        &json!({"alg": "none", "kid": KID}),
        &testing::claims("1", now()),
    );
    let wrong_aud = {
        let mut claims = testing::claims("1", now());
        claims["aud"] = json!("other.pulsars.science");
        testing::sign(&server.key, &json!({"alg": "EdDSA", "kid": KID}), &claims)
    };

    for (token, reason, description) in [
        (expired.as_str(), "expired", "token expired"),
        (forged.as_str(), "bad_signature", "invalid signature"),
        (none.as_str(), "bad_algorithm", "unsupported algorithm"),
        (wrong_aud.as_str(), "bad_audience", "invalid audience"),
        ("not-a-jwt", "malformed", "token malformed"),
    ] {
        let (status, headers, body) = send(&app, get_with_token("/v1/me", token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{reason}");
        assert_eq!(body["reason"], reason);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            headers[header::WWW_AUTHENTICATE],
            format!(
                "Bearer realm=\"api.pulsars.science\", error=\"invalid_token\", \
                 error_description=\"{description}\""
            )
        );
        // The problem never echoes the token.
        assert!(!body.to_string().contains(token));
    }
}

#[tokio::test]
async fn unknown_kids_refetch_the_jwks_at_most_once_a_minute() {
    let (app, _, server) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    assert_eq!(server.hits(), 1);

    for i in 0..5 {
        let token = testing::token(&server.key, &format!("random-{i}"), "1", now());
        let (status, _, body) = send(&app, get_with_token("/v1/me", &token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["reason"], "unknown_kid");
    }
    assert_eq!(server.hits(), 2, "a single refetch for five unknown kids");
}

#[tokio::test]
async fn a_new_key_is_picked_up_through_the_unknown_kid_refetch() {
    let (app, state, server) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    // Rotation step 1: Laravel publishes the next key.
    let next = testing::signing_key();
    server.set_document(json!({"keys": [
        testing::jwk(&server.key, KID),
        testing::jwk(&next, "2027-04-ed25519-1"),
    ]}));

    let token = testing::token(&next, "2027-04-ed25519-1", "1", now());
    let (status, headers, body) = send(&app, get_with_token("/v1/me", &token)).await;
    // Authenticated: the request got as far as the (dead) database.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(!headers.contains_key(header::WWW_AUTHENTICATE));
    assert_eq!(server.hits(), 2);
    assert!(matches!(
        state.jwks.key("2027-04-ed25519-1").await,
        pulsars_api::auth::Lookup::Found(_)
    ));
}

#[tokio::test]
async fn a_failed_refresh_keeps_the_previous_keys() {
    let (app, state, server) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    server.set_status(500);
    assert!(state.jwks.refresh().await.is_err());
    server.set_status(200);
    server.set_document(json!({"keys": []}));
    assert!(
        state.jwks.refresh().await.is_err(),
        "an empty set is a failure"
    );

    let (status, _, body) = send(&app, get_with_token("/v1/me", &server.token("1"))).await;
    assert_ne!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn keys_older_than_24_hours_are_not_used() {
    let (app, state, server) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    server.set_status(500);
    state.jwks.age_by(Duration::from_secs(24 * 60 * 60 + 1));

    let (status, headers, body) = send(&app, get_with_token("/v1/me", &server.token("1"))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/unavailable"
    );
    assert!(!headers.contains_key(header::WWW_AUTHENTICATE));
}

#[tokio::test]
async fn me_answers_503_while_no_key_was_ever_loaded() {
    let (app, _) = app(config(DEAD_POSTGRES, DEAD_REDIS, &[])).await;
    let key = testing::signing_key();
    let token = testing::token(&key, KID, "1", now());
    let (status, headers, body) = send(&app, get_with_token("/v1/me", &token)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert!(!headers.contains_key(header::WWW_AUTHENTICATE));
}

#[tokio::test]
async fn https_only_clients_refuse_a_plain_http_jwks() {
    let server = JwksServer::start().await;
    let jwks = Jwks::with_settings(&server.url, true, jwks_settings()).unwrap();
    assert!(jwks.refresh().await.is_err());
    assert_eq!(server.hits(), 0);
}

#[tokio::test]
async fn oversized_jwks_documents_are_refused() {
    let server = JwksServer::start().await;
    server.set_document(json!({
        "keys": [testing::jwk(&server.key, KID)],
        "padding": "x".repeat(100 * 1024),
    }));
    let jwks = Jwks::with_settings(&server.url, false, jwks_settings()).unwrap();
    assert!(jwks.refresh().await.is_err());
    assert!(!jwks.is_loaded());
}

#[tokio::test]
async fn me_allows_only_the_game_origin() {
    let (app, _, _jwks) = app_with_keys(DEAD_POSTGRES, DEAD_REDIS).await;
    let preflight = |origin: &str| {
        Request::options("/v1/me")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
            .body(Body::empty())
            .unwrap()
    };

    let (_, headers, _) = send(&app, preflight("http://localhost:5173")).await;
    assert_eq!(
        headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://localhost:5173"
    );
    let (_, headers, _) = send(&app, preflight("https://evil.example")).await;
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));

    // Errors carry the CORS headers too, so the front end can read `reason`.
    let request = Request::get("/v1/me")
        .header(header::ORIGIN, "https://game.pulsars.science")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://game.pulsars.science"
    );
}

// --- /v1/me against the demo data -----------------------------------------

#[tokio::test]
async fn me_returns_the_public_profile() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let (app, _, server) = app_with_keys(&database_url, &redis_url).await;

    // `ada` (dev/db/30-demo-data.sql): bio, two roles, 102 XP.
    let (status, headers, body) = send(&app, get_with_token("/v1/me", &server.token("2"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
    assert!(
        headers
            .get_all(header::VARY)
            .iter()
            .any(|v| v.to_str().unwrap().contains("Authorization")),
        "{headers:?}"
    );
    assert_eq!(
        body,
        json!({"data": {
            "id": "2",
            "pseudo": "ada",
            "avatar_url": null,
            "bio": "Mathématiques et café.",
            "member_since": "2026-08-10T09:00:00Z",
            "roles": [
                {"slug": "mathematiques", "name": "Mathematiques",
                 "category": "matiere", "color": "#4f7df0"},
                {"slug": "membre-fondateur", "name": "Membre fondateur",
                 "category": "distinction", "color": "#c98a34"},
            ],
            "xp": {"total": 102, "level": 2, "level_start": 100, "next_level_at": 300},
            "profile_url": "https://pulsars.science/membres/ada",
        }})
    );

    // `jikuji`: the OpenAPI example.
    let (status, _, body) = send(&app, get_with_token("/v1/me", &server.token("1"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["roles"][0]["slug"], "super-admin");
    assert_eq!(
        body["data"]["xp"],
        json!({"total": 0, "level": 1, "level_start": 0, "next_level_at": 100})
    );
}

#[tokio::test]
async fn me_never_returns_private_fields() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let (app, _, server) = app_with_keys(&database_url, &redis_url).await;
    let (_, _, body) = send(&app, get_with_token("/v1/me", &server.token("2"))).await;
    let text = body.to_string();
    for marker in [
        "example.invalid",
        "Terminale B",
        "lycee",
        "demofriendtoken",
        "\"en\"",
    ] {
        assert!(!text.contains(marker), "`{marker}` leaked into /v1/me");
    }
}

#[tokio::test]
async fn me_is_404_when_the_account_is_gone() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let (app, _, server) = app_with_keys(&database_url, &redis_url).await;
    for sub in ["999999", "not-a-number", "02"] {
        let (status, headers, body) =
            send(&app, get_with_token("/v1/me", &server.token(sub))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{sub}: {body}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            body["type"],
            "https://api.pulsars.science/problems/not-found"
        );
        assert_eq!(body["instance"], "/v1/me");
    }
}
