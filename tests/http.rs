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
use pulsars_api::game::{Game, GameSettings, store};
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

/// Test configuration. Rate limits are lifted so tests sharing a Redis
/// do not throttle each other; rate limit tests set their own.
fn config(database_url: &str, redis_url: &str, extra: &[(&str, &str)]) -> Config {
    let mut vars = vec![
        ("PULSARS_ENV".to_owned(), "development".to_owned()),
        ("DATABASE_URL".to_owned(), database_url.to_owned()),
        ("REDIS_URL".to_owned(), redis_url.to_owned()),
        ("JWKS_URL".to_owned(), DEAD_JWKS.to_owned()),
        (
            "RATE_LIMIT_ANONYMOUS_PER_MINUTE".to_owned(),
            "1000000".to_owned(),
        ),
        (
            "RATE_LIMIT_AUTHENTICATED_PER_MINUTE".to_owned(),
            "1000000".to_owned(),
        ),
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
    let db = db::connect_lazy(&config).unwrap();
    let state = AppState {
        game: Game::start(db.clone(), GameSettings::default()),
        db,
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
    // The methods of this route, not a list shared by every route.
    assert_eq!(headers[header::ALLOW], "GET,HEAD");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/method-not-allowed"
    );

    let request = Request::post("/v1/courses").body(Body::empty()).unwrap();
    let (status, headers, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers[header::ALLOW], "GET,HEAD,OPTIONS");
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

/// Applies the migrations as the role allowed to create tables:
/// `TEST_MIGRATION_DATABASE_URL` (`pulsars_migrate`), else the tests' own
/// URL, for a database whose service role still owns its schemas.
async fn apply_migrations(database_url: &str) {
    let url = std::env::var("TEST_MIGRATION_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| database_url.to_owned());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    store::migrate(&pool).await.expect("migrations apply");
    pool.close().await;
}

/// The grants of deploy/sql/phase-1.sql, as the service's role sees them.
#[tokio::test]
async fn the_api_role_reads_content_writes_game_and_changes_no_structure() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    // The game tables must exist for the structure checks below.
    apply_migrations(&database_url).await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let denied = |error: sqlx::Error| {
        let message = error.to_string();
        assert!(message.contains("permission denied"), "{message}");
    };

    // No access to accounts before phase 2.
    denied(
        sqlx::query("SELECT 1 FROM site.users LIMIT 1")
            .execute(&pool)
            .await
            .expect_err("pulsars_api must not read site"),
    );

    // Content: read, never written.
    let raw: i64 = sqlx::query_scalar("SELECT count(*) FROM contenu.courses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(raw > 0, "demo data is loaded");
    denied(
        sqlx::query("UPDATE contenu.courses SET vues = vues WHERE false")
            .execute(&pool)
            .await
            .expect_err("pulsars_api must not write content"),
    );

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

    // Structure belongs to pulsars_owner: `pulsars-api migrate` changes it.
    for ddl in [
        "CREATE TABLE game.write_check (id int)",
        "CREATE TABLE contenu.write_check (id int)",
        "ALTER TABLE game.ratings ADD COLUMN write_check int",
        "DROP TABLE game.seasons",
    ] {
        let error = sqlx::query(ddl)
            .execute(&pool)
            .await
            .expect_err("pulsars_api must not change the structure");
        let message = error.to_string();
        assert!(
            message.contains("permission denied") || message.contains("must be owner"),
            "{ddl}: {message}"
        );
    }

    // But it writes the rows of game.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO game.ratings (user_id, subject, rating, rd, volatility, games, wins, losses, draws)
         VALUES (999999, 'all', 1500, 350, 0.06, 0, 0, 0, 0)",
    )
    .execute(&mut *tx)
    .await
    .expect("pulsars_api writes game rows");
    tx.rollback().await.unwrap();
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

/// What the API reads of `contenu` (src/db/sources.rs) is, row for row,
/// what the `api_read` views showed: same published rows, same columns.
#[tokio::test]
async fn the_content_sources_read_exactly_what_the_views_show() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    // contenu.matieres, contenu.licences.
    apply_migrations(&database_url).await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let comparisons = pulsars_api::db::sources::compare_with_views(&pool)
        .await
        .unwrap();
    assert_eq!(comparisons.len(), pulsars_api::db::sources::SOURCES.len());
    for c in &comparisons {
        assert!(c.is_same(), "{c:?}");
    }
    let courses = comparisons.iter().find(|c| c.view == "courses").unwrap();
    assert!(courses.rows > 0, "demo data is loaded");
}

#[tokio::test]
async fn no_answer_or_private_data_reaches_the_content_sources() {
    let Some((database_url, _)) = integration_urls() else {
        return;
    };
    apply_migrations(&database_url).await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let mut dump = String::new();
    for &(view, source) in pulsars_api::db::sources::SOURCES {
        // The sources are constants of the API, not user input.
        let rows: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT string_agg(row_to_json(t)::text, '\n') FROM {source} t"
        )))
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|error| panic!("{view}: {error}"));
        dump.push_str(&rows.unwrap_or_default());
    }
    assert!(!dump.is_empty(), "demo data is loaded");

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
        "solution_md",
        "corrige_md",
        "demo-draft-course",
        "demo-future-course",
        "demo-pending-resource",
        "-corrige.pdf",
    ] {
        assert!(
            !dump.contains(marker),
            "`{marker}` leaked into a content source"
        );
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
        .header(header::ORIGIN, "https://pulsars.science")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://pulsars.science"
    );
}

// --- /v1/me against the demo data -----------------------------------------

#[tokio::test]
async fn me_returns_the_public_profile() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let (app, _, server) = app_with_keys(&database_url, &redis_url).await;

    // `ada` (dev/db/30-demo-data.sql): bio, two roles, 102 XP, 15 brains.
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
            "brains": 15,
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
    // No movement: 0, not null.
    assert_eq!(body["data"]["brains"], 0);

    // `emmy` hides her balance from others, not from herself.
    let (status, _, body) = send(&app, get_with_token("/v1/me", &server.token("3"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["brains"], 0);
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

// --- Content endpoints against the demo data ------------------------------
//
// `dev/db/30-demo-data.sql`: two published courses (`les-bases-du-calcul`,
// `chimie-le-tableau-periodique`), a draft and a future course; exercises
// 1-5 published, 6 in the draft; exams 2 and 5 published, 1 a draft;
// resources 1-3 published, 4 pending. Every exercise carries its answer and
// every exam part its correction, so the tests can prove none comes out.

/// The application against the demo database, or `None` when skipped.
async fn content_app() -> Option<Router> {
    let (database_url, redis_url) = integration_urls()?;
    let (app, _) = app(config(&database_url, &redis_url, &[])).await;
    Some(app)
}

async fn get_json(app: &Router, path: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    send(app, get(path)).await
}

/// Values of `field` in a list response's `data`.
fn field(body: &Value, field: &str) -> Vec<Value> {
    body["data"]
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {body}"))
        .iter()
        .map(|item| item[field].clone())
        .collect()
}

/// Follows `next_cursor` from `path` to the last page.
async fn all_pages(app: &Router, path: &str) -> (Vec<Value>, usize) {
    let mut items = Vec::new();
    let mut pages = 0;
    let mut next = path.to_owned();
    loop {
        let (status, _, body) = get_json(app, &next).await;
        assert_eq!(status, StatusCode::OK, "{next}: {body}");
        pages += 1;
        items.extend(body["data"].as_array().unwrap().iter().cloned());
        match body["next_cursor"].as_str() {
            Some(cursor) => {
                let separator = if path.contains('?') { '&' } else { '?' };
                next = format!("{path}{separator}cursor={cursor}");
            }
            None => return (items, pages),
        }
        assert!(pages < 50, "pagination does not end");
    }
}

fn course_basics_fr() -> Value {
    json!({
        "id": 1,
        "slug": "les-bases-du-calcul",
        "lang": "fr",
        "title": "Les bases du calcul",
        "description": "Priorités opératoires, fractions, puissances et pourcentages. De quoi calculer juste, sans calculatrice. Aucun prérequis.",
        "theme": "Mathématiques",
        "subject": {"slug": "mathematiques", "name": "Mathématiques"},
        "topic": null,
        "difficulty": "debutant",
        "levels": [],
        "duration_minutes": 60,
        "tags": ["maths", "calcul"],
        "author": "jikuji",
        "chapter_count": 2,
        "lesson_count": 4,
        "exercise_count": 5,
        "available_langs": ["fr", "en"],
        "published_at": "2026-08-17T09:27:34Z",
        "updated_at": "2026-08-17T09:27:34Z",
        "web_url": "https://pulsars.science/cours/les-bases-du-calcul",
    })
}

#[tokio::test]
async fn catalog_derives_subjects_and_topics_from_themes() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, headers, body) = get_json(&app, "/v1/catalog").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_LANGUAGE], "fr");
    assert_eq!(
        body,
        json!({"data": {
            "taxonomy": "theme",
            "lang": "fr",
            "subjects": [
                {"slug": "chimie", "name": "Chimie", "course_count": 1, "topics": [
                    {"slug": "la-matiere", "name": "La matière", "course_count": 1},
                ]},
                {"slug": "mathematiques", "name": "Mathématiques", "course_count": 1, "topics": []},
            ],
        }})
    );

    // Subject names are translated, topic names never.
    let (_, headers, body) = get_json(&app, "/v1/catalog?lang=en").await;
    assert_eq!(headers[header::CONTENT_LANGUAGE], "en");
    assert_eq!(body["data"]["lang"], "en");
    assert_eq!(body["data"]["subjects"][0]["name"], "Chemistry");
    assert_eq!(
        body["data"]["subjects"][0]["topics"][0]["name"],
        "La matière"
    );
    assert_eq!(body["data"]["subjects"][1]["name"], "Mathematics");
}

#[tokio::test]
async fn courses_list_published_courses_only() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, headers, body) = get_json(&app, "/v1/courses").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=300, stale-while-revalidate=60"
    );
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(body["next_cursor"], Value::Null);

    let mut slugs = field(&body, "slug");
    slugs.sort_by_key(|s| s.to_string());
    assert_eq!(
        slugs,
        vec![
            json!("chimie-le-tableau-periodique"),
            json!("les-bases-du-calcul")
        ]
    );
    let find = |slug: &str| {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["slug"] == slug)
            .unwrap()
            .clone()
    };
    assert_eq!(find("les-bases-du-calcul"), course_basics_fr());
    let chimie = find("chimie-le-tableau-periodique");
    assert_eq!(
        chimie["topic"],
        json!({"slug": "la-matiere", "name": "La matière"})
    );
    assert_eq!(chimie["levels"], json!(["lycee"]));
    assert_eq!(chimie["available_langs"], json!(["fr"]));
}

#[tokio::test]
async fn courses_filters() {
    let Some(app) = content_app().await else {
        return;
    };
    for (query, expected) in [
        ("subject=chimie", vec!["chimie-le-tableau-periodique"]),
        (
            "subject=chimie&topic=la-matiere",
            vec!["chimie-le-tableau-periodique"],
        ),
        ("subject=chimie&topic=analyse", vec![]),
        ("subject=mathematiques", vec!["les-bases-du-calcul"]),
        ("difficulty=debutant", vec!["les-bases-du-calcul"]),
        // The only `avance` course is a draft.
        ("difficulty=avance", vec![]),
        ("level=lycee", vec!["chimie-le-tableau-periodique"]),
        // The draft's `supérieur` tag must not show through.
        ("level=superieur", vec![]),
        ("tag=calcul", vec!["les-bases-du-calcul"]),
        ("tag=lyc%C3%A9e", vec!["chimie-le-tableau-periodique"]),
        ("available_in=en", vec!["les-bases-du-calcul"]),
        (
            "available_in=fr",
            vec!["chimie-le-tableau-periodique", "les-bases-du-calcul"],
        ),
        ("q=CALCULATRICE", vec!["les-bases-du-calcul"]),
        // Searched in the translation for `lang`, and only then.
        (
            "q=without%20a%20calculator&lang=en",
            vec!["les-bases-du-calcul"],
        ),
        ("q=without%20a%20calculator", vec![]),
        // `%` and `_` are literal characters, not wildcards.
        ("q=%25%25", vec![]),
        ("q=__", vec![]),
        ("q=brouillon", vec![]),
    ] {
        let (status, _, body) = get_json(&app, &format!("/v1/courses?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let mut slugs: Vec<String> = field(&body, "slug")
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        slugs.sort();
        assert_eq!(slugs, expected, "{query}");
    }
}

#[tokio::test]
async fn course_detail_matches_the_openapi_example() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(&app, "/v1/courses/les-bases-du-calcul").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut expected = course_basics_fr();
    expected["chapters"] = json!([
        {"position": 1, "title": "L'ordre des opérations", "lang": "fr",
         "lessons": [
            {"slug": "priorites-operatoires", "position": 1, "title": "Les priorités opératoires", "lang": "fr"},
            {"slug": "les-fractions", "position": 2, "title": "Les fractions", "lang": "fr"},
         ],
         "exercises": [
            {"id": 1, "position": 1, "type": "qcm", "title": "Appliquer les priorités", "lang": "fr"},
            {"id": 2, "position": 2, "type": "reponse_courte", "title": "Simplifier une fraction", "lang": "fr"},
         ]},
        {"position": 2, "title": "Puissances et pourcentages", "lang": "fr",
         "lessons": [
            {"slug": "les-puissances", "position": 1, "title": "Les puissances", "lang": "fr"},
            {"slug": "les-pourcentages", "position": 2, "title": "Les pourcentages", "lang": "fr"},
         ],
         "exercises": [
            {"id": 3, "position": 1, "type": "qcm", "title": "Vrai ou faux sur les puissances", "lang": "fr"},
            {"id": 4, "position": 2, "type": "reponse_courte", "title": "Calculer un pourcentage", "lang": "fr"},
            {"id": 5, "position": 3, "type": "reponse_libre", "title": "Le piège des pourcentages", "lang": "fr"},
         ]},
    ]);
    assert_eq!(body, json!({"data": expected}));
}

#[tokio::test]
async fn english_falls_back_to_french_per_object_and_field() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, headers, body) = get_json(&app, "/v1/courses/les-bases-du-calcul?lang=en").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_LANGUAGE], "en");
    let course = &body["data"];
    assert_eq!(course["lang"], "en");
    assert_eq!(course["title"], "The basics of the calculation");
    assert_eq!(course["subject"]["name"], "Mathematics");
    // Never translated.
    assert_eq!(course["theme"], "Mathématiques");
    assert_eq!(course["tags"], json!(["maths", "calcul"]));
    assert_eq!(course["chapters"][0]["lang"], "en");
    assert_eq!(course["chapters"][0]["title"], "The order of operations");
    assert_eq!(
        course["chapters"][0]["exercises"][0]["title"],
        "Set priorities"
    );
    assert_eq!(course["chapters"][0]["exercises"][1]["lang"], "fr");
    assert_eq!(course["chapters"][1]["lang"], "fr");
    assert_eq!(course["chapters"][1]["title"], "Puissances et pourcentages");

    // A course without any translation stays French.
    let (_, _, body) = get_json(&app, "/v1/courses/chimie-le-tableau-periodique?lang=en").await;
    assert_eq!(body["data"]["lang"], "fr");
    assert_eq!(body["data"]["subject"]["name"], "Chemistry");
}

#[tokio::test]
async fn lessons_link_across_chapters() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(
        &app,
        "/v1/courses/les-bases-du-calcul/lessons/priorites-operatoires",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lesson = &body["data"];
    assert_eq!(lesson["slug"], "priorites-operatoires");
    assert_eq!(lesson["lang"], "fr");
    assert_eq!(lesson["title"], "Les priorités opératoires");
    assert_eq!(lesson["position"], 1);
    assert!(
        lesson["content_md"]
            .as_str()
            .unwrap()
            .starts_with("Combien vaut `2 + 3 x 4` ?")
    );
    assert_eq!(
        lesson["chapter"],
        json!({"position": 1, "title": "L'ordre des opérations", "lang": "fr"})
    );
    assert_eq!(
        lesson["course"],
        json!({"slug": "les-bases-du-calcul", "title": "Les bases du calcul", "lang": "fr"})
    );
    assert_eq!(lesson["previous"], Value::Null);
    assert_eq!(
        lesson["next"],
        json!({"slug": "les-fractions", "title": "Les fractions"})
    );
    assert_eq!(lesson["updated_at"], "2026-08-13T18:59:10Z");
    assert_eq!(
        lesson["web_url"],
        "https://pulsars.science/cours/les-bases-du-calcul/lecon/priorites-operatoires"
    );

    // The last lesson of chapter 1 leads to the first of chapter 2.
    let (_, _, body) = get_json(
        &app,
        "/v1/courses/les-bases-du-calcul/lessons/les-fractions",
    )
    .await;
    assert_eq!(body["data"]["next"]["slug"], "les-puissances");
    let (_, _, body) = get_json(
        &app,
        "/v1/courses/les-bases-du-calcul/lessons/les-pourcentages",
    )
    .await;
    assert_eq!(body["data"]["next"], Value::Null);
    assert_eq!(body["data"]["previous"]["slug"], "les-puissances");
}

#[tokio::test]
async fn lesson_in_english_matches_the_openapi_example() {
    let Some(app) = content_app().await else {
        return;
    };
    let (_, _, body) = get_json(
        &app,
        "/v1/courses/les-bases-du-calcul/lessons/priorites-operatoires?lang=en",
    )
    .await;
    let lesson = &body["data"];
    assert_eq!(lesson["lang"], "en");
    assert_eq!(lesson["title"], "Operational priorities");
    assert!(
        lesson["content_md"]
            .as_str()
            .unwrap()
            .starts_with("What is `2 + 3 x 4` equal to?")
    );
    assert_eq!(
        lesson["chapter"],
        json!({"position": 1, "title": "The order of operations", "lang": "en"})
    );
    assert_eq!(
        lesson["course"],
        json!({"slug": "les-bases-du-calcul", "title": "The basics of the calculation", "lang": "en"})
    );
    assert_eq!(
        lesson["next"],
        json!({"slug": "les-fractions", "title": "Fractions"})
    );

    // Translated title, empty translated content: the content falls back.
    let (_, _, body) = get_json(
        &app,
        "/v1/courses/les-bases-du-calcul/lessons/les-fractions?lang=en",
    )
    .await;
    assert_eq!(body["data"]["lang"], "en");
    assert_eq!(body["data"]["title"], "Fractions");
    assert!(
        body["data"]["content_md"]
            .as_str()
            .unwrap()
            .starts_with("Une fraction")
    );
}

#[tokio::test]
async fn exercises_never_carry_their_answer() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(&app, "/v1/exercises/1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"data": {
            "id": 1,
            "lang": "fr",
            "type": "qcm",
            "title": "Appliquer les priorités",
            "question_md": "Combien vaut `12 - 2 x 3 + 1` ?",
            "options": [
                {"id": "a", "text": "31"},
                {"id": "b", "text": "7"},
                {"id": "c", "text": "11"},
                {"id": "d", "text": "4"},
            ],
            "multiple": false,
            "position": 1,
            "chapter": {"position": 1, "title": "L'ordre des opérations", "lang": "fr"},
            "course": {"slug": "les-bases-du-calcul", "title": "Les bases du calcul",
                       "lang": "fr", "difficulty": "debutant"},
            "web_url": "https://pulsars.science/cours/les-bases-du-calcul/exercice/1",
        }})
    );

    let (_, _, body) = get_json(&app, "/v1/exercises/2").await;
    assert_eq!(body["data"]["type"], "reponse_courte");
    assert_eq!(
        body["data"]["question_md"],
        "Simplifie la fraction `18/24` au maximum.\n\nÉcris ta réponse sous la forme `a/b`."
    );
    assert_eq!(body["data"]["options"], Value::Null);
    assert_eq!(body["data"]["multiple"], Value::Null);

    let (_, _, body) = get_json(&app, "/v1/exercises/3").await;
    assert_eq!(body["data"]["multiple"], true);
    assert_eq!(body["data"]["chapter"]["position"], 2);
    assert_eq!(body["data"]["options"].as_array().unwrap().len(), 5);

    // English: title and question translated, never the solution.
    let (_, _, body) = get_json(&app, "/v1/exercises/1?lang=en").await;
    assert_eq!(body["data"]["lang"], "en");
    assert_eq!(body["data"]["title"], "Set priorities");
    assert_eq!(
        body["data"]["question_md"],
        "What is the value of `12 - 2 x 3 + 1`?"
    );
    assert_eq!(
        body["data"]["course"]["title"],
        "The basics of the calculation"
    );
    assert_eq!(body["data"]["options"][1]["text"], "7");

    // Translated options keep the original ids and order; a missing or empty
    // translation falls back to the original text, option by option.
    let (_, _, body) = get_json(&app, "/v1/exercises/3?lang=en").await;
    assert_eq!(
        body["data"]["options"],
        json!([
            {"id": "a", "text": "OPTION-EN-A"},
            {"id": "b", "text": "2^3 x 2^4 = 2^12"},
            {"id": "c", "text": "OPTION-EN-C"},
            {"id": "d", "text": "-3^2 = 9"},
            {"id": "e", "text": "5^0 = 1"},
        ])
    );
    assert!(!body.to_string().contains("correctes"), "{body}");
}

#[tokio::test]
async fn exercises_paginate_by_id() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(&app, "/v1/exercises?limit=2").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(field(&body, "id"), vec![json!(1), json!(2)]);
    assert!(body["next_cursor"].is_string());

    let (items, pages) = all_pages(&app, "/v1/exercises?limit=2").await;
    let ids: Vec<Value> = items.iter().map(|e| e["id"].clone()).collect();
    // Exercise 6 belongs to a draft course.
    assert_eq!(ids, vec![json!(1), json!(2), json!(3), json!(4), json!(5)]);
    assert_eq!(pages, 3);

    // A page that exactly fills the limit has no next cursor.
    let (_, _, body) = get_json(&app, "/v1/exercises?limit=5").await;
    assert_eq!(field(&body, "id").len(), 5);
    assert_eq!(body["next_cursor"], Value::Null);
    let (_, _, body) = get_json(&app, "/v1/exercises?limit=100").await;
    assert_eq!(body["next_cursor"], Value::Null);

    for (query, expected) in [
        ("type=qcm", vec![1, 3]),
        ("type=reponse_libre", vec![5]),
        ("course=les-bases-du-calcul", vec![1, 2, 3, 4, 5]),
        ("course=demo-draft-course", vec![]),
        ("subject=chimie", vec![]),
        (
            "subject=mathematiques&difficulty=debutant",
            vec![1, 2, 3, 4, 5],
        ),
        ("difficulty=avance", vec![]),
    ] {
        let (status, _, body) = get_json(&app, &format!("/v1/exercises?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let expected: Vec<Value> = expected.into_iter().map(Value::from).collect();
        assert_eq!(field(&body, "id"), expected, "{query}");
    }
}

#[tokio::test]
async fn cursors_are_bound_to_their_filters() {
    let Some(app) = content_app().await else {
        return;
    };
    let (_, _, body) = get_json(&app, "/v1/exercises?limit=1&type=qcm").await;
    let cursor = body["next_cursor"].as_str().unwrap().to_owned();

    let (status, _, body) = get_json(
        &app,
        &format!("/v1/exercises?limit=1&type=qcm&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(field(&body, "id"), vec![json!(3)]);

    for path in [
        format!("/v1/exercises?limit=1&cursor={cursor}"),
        format!("/v1/exercises?type=qcm&lang=en&cursor={cursor}"),
        format!("/v1/courses?cursor={cursor}"),
        "/v1/exercises?cursor=not-a-cursor".to_owned(),
        "/v1/exams?cursor=e30".to_owned(),
    ] {
        let (status, headers, body) = get_json(&app, &path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
        assert_eq!(
            body["type"], "https://api.pulsars.science/problems/invalid-cursor",
            "{path}"
        );
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    }
}

#[tokio::test]
async fn courses_exams_and_resources_paginate() {
    let Some(app) = content_app().await else {
        return;
    };
    let (courses, pages) = all_pages(&app, "/v1/courses?limit=1").await;
    assert_eq!(pages, 2);
    let (all, _) = all_pages(&app, "/v1/courses").await;
    assert_eq!(courses, all);

    // Newest first, then id descending.
    let (exams, pages) = all_pages(&app, "/v1/exams?limit=1").await;
    assert_eq!(pages, 2);
    let ids: Vec<Value> = exams.iter().map(|e| e["id"].clone()).collect();
    assert_eq!(ids, vec![json!(5), json!(2)]);

    // Resources 1 and 2 were created in the same second: id breaks the tie.
    let (resources, pages) = all_pages(&app, "/v1/resources?limit=1").await;
    assert_eq!(pages, 3);
    let ids: Vec<Value> = resources.iter().map(|r| r["id"].clone()).collect();
    assert_eq!(ids, vec![json!(3), json!(2), json!(1)]);
}

#[tokio::test]
async fn exams_show_statements_only() {
    let Some(app) = content_app().await else {
        return;
    };
    let slug = "baccalaureat-general-2026-mathematiques-specialite-jour-1";
    let (status, _, body) = get_json(&app, &format!("/v1/exams/{slug}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"data": {
            "id": 2,
            "slug": slug,
            "lang": "fr",
            "title": "Baccalauréat général 2026 — Mathématiques, spécialité (jour 1)",
            "type": "bac",
            "competition": null,
            "subject": "mathematiques",
            "country": "FR",
            "level": "Terminale",
            "tracks": [],
            "session": "Session 2026 — 16 juin 2026",
            "duration_minutes": 240,
            "total_points": 20,
            "part_count": 2,
            "pdf_url": format!("https://pulsars.science/examens/{slug}/telecharger"),
            "official_url": null,
            "source_url": null,
            "published_at": "2026-08-13T09:32:04Z",
            "web_url": format!("https://pulsars.science/examens/{slug}"),
            "description": "Sujet de spécialité mathématiques du baccalauréat général, session 2026, épreuve du 16 juin (sujet 26-MATJ1ME1).",
            "parts": [
                {"id": 46, "position": 1, "lang": "fr",
                 "title": "Exercice 1 — Probabilités : traversée en navire", "points": 5,
                 "statement_md": "Un navire assure la liaison entre deux ports de la Méditerranée."},
                {"id": 47, "position": 2, "lang": "fr",
                 "title": "Exercice 2 — Vrai/faux : géométrie dans l'espace et dénombrement", "points": 4,
                 "statement_md": "Pour chacune des affirmations suivantes, indiquer si elle est vraie ou fausse."},
            ],
        }})
    );

    // Parts are translated, the exam itself never.
    let (_, _, body) = get_json(&app, &format!("/v1/exams/{slug}?lang=en")).await;
    assert_eq!(body["data"]["lang"], "fr");
    assert_eq!(body["data"]["parts"][0]["lang"], "en");
    assert_eq!(
        body["data"]["parts"][0]["statement_md"],
        "A ship links two Mediterranean ports."
    );
    assert_eq!(body["data"]["parts"][1]["lang"], "fr");

    // The external competitive paper: no hosted PDF, the allowed official URL.
    let (_, _, body) = get_json(
        &app,
        "/v1/exams/centrale-supelec-2026-mathematiques-1-mp-mpi",
    )
    .await;
    let exam = &body["data"];
    assert_eq!(exam["tracks"], json!(["MP", "MPI"]));
    assert_eq!(exam["pdf_url"], Value::Null);
    assert_eq!(
        exam["official_url"],
        "https://www.concours-centrale-supelec.fr/sites/default/files/documents/M098.pdf"
    );
    assert_eq!(exam["part_count"], 0);
    assert_eq!(exam["parts"], json!([]));
    assert_eq!(exam["duration_minutes"], Value::Null);
}

#[tokio::test]
async fn exams_filters() {
    let Some(app) = content_app().await else {
        return;
    };
    for (query, expected) in [
        ("type=concours", vec![5]),
        ("type=bac", vec![2]),
        ("type=brevet", vec![]),
        ("subject=mathematiques", vec![5, 2]),
        ("level=Terminale", vec![2]),
        ("level=terminale", vec![]),
        ("session=2026", vec![5]),
        ("country=FR", vec![5, 2]),
        ("competition=centrale-supelec", vec![5]),
        ("q=centrale", vec![5]),
        ("q=26-MATJ1ME1", vec![2]),
        ("q=relecture", vec![]),
    ] {
        let (status, _, body) = get_json(&app, &format!("/v1/exams?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let expected: Vec<Value> = expected.into_iter().map(Value::from).collect();
        assert_eq!(field(&body, "id"), expected, "{query}");
    }
    let (_, _, body) = get_json(&app, "/v1/exams").await;
    // Summaries carry neither description nor parts.
    assert!(body["data"][0].get("description").is_none());
    assert!(body["data"][0].get("parts").is_none());
}

#[tokio::test]
async fn resources_match_the_openapi_examples() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(&app, "/v1/resources/misselbooks-abstract-algebra").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"data": {
            "id": 2,
            "slug": "misselbooks-abstract-algebra",
            "lang": "fr",
            "title": "Abstract Algebra",
            "description": "Un manuel d’algèbre abstraite en anglais (220 pages) : groupes, anneaux, corps, avec des applications à la cryptographie. Écrit et publié en accès libre par Andrew Misseldine.",
            "category": "Mathématiques",
            "doc_lang": "en",
            "mode": "host",
            "attribution": "Andrew Misseldine",
            "license": {
                "key": "cc0-1.0",
                "name": "CC0 1.0 (domaine public)",
                "url": "https://creativecommons.org/publicdomain/zero/1.0/",
            },
            "download_url": "https://pulsars.science/ressources/misselbooks-abstract-algebra/telecharger",
            "source_url": "https://github.com/emisseldine/MisselBooks/blob/main/AbstractAlgebra/Abstract%20Algebra.pdf",
            "size_bytes": 1543021,
            "created_at": "2026-08-07T14:44:26Z",
            "updated_at": "2026-08-07T14:44:26Z",
            "web_url": "https://pulsars.science/ressources/misselbooks-abstract-algebra",
        }})
    );

    let (_, _, body) = get_json(&app, "/v1/resources/article-chimie-inorganique-ic901972h").await;
    assert_eq!(
        body,
        json!({"data": {
            "id": 1,
            "slug": "article-chimie-inorganique-ic901972h",
            "lang": "fr",
            "title": "Tripodal 4-Pyridyl-Derived Host Ligands and Their Metallo-Supramolecular Chemistry: Stella Octangula and Bowl-Shaped Assemblies",
            "description": "Un article de recherche en chimie inorganique, à télécharger en PDF.",
            "category": "Chimie",
            "doc_lang": "en",
            "mode": "host",
            "attribution": null,
            "license": null,
            "download_url": "https://pulsars.science/ressources/article-chimie-inorganique-ic901972h/telecharger",
            "source_url": null,
            "size_bytes": 3451711,
            "created_at": "2026-08-07T14:44:26Z",
            "updated_at": "2026-08-07T14:44:26Z",
            "web_url": "https://pulsars.science/ressources/article-chimie-inorganique-ic901972h",
        }})
    );

    // A link: no download, no size.
    let (_, _, body) = get_json(&app, "/v1/resources/misselbooks-calculus-i").await;
    assert_eq!(body["data"]["mode"], "link");
    assert_eq!(body["data"]["download_url"], Value::Null);
    assert_eq!(body["data"]["size_bytes"], Value::Null);

    // English: translated title, description and category.
    let (_, _, body) = get_json(&app, "/v1/resources/misselbooks-abstract-algebra?lang=en").await;
    assert_eq!(body["data"]["lang"], "en");
    assert_eq!(body["data"]["category"], "Mathematics");
    assert!(
        body["data"]["description"]
            .as_str()
            .unwrap()
            .starts_with("A 220-page")
    );
}

#[tokio::test]
async fn resources_filters() {
    let Some(app) = content_app().await else {
        return;
    };
    for (query, expected) in [
        ("mode=link", vec![3]),
        ("mode=host", vec![2, 1]),
        ("license=cc0-1.0", vec![3, 2]),
        // The original category, whatever `lang` says.
        ("category=Math%C3%A9matiques", vec![3, 2]),
        ("category=Mathematics&lang=en", vec![]),
        ("doc_lang=en", vec![3, 2, 1]),
        ("doc_lang=fr", vec![]),
        ("q=algebre", vec![]),
        ("q=alg%C3%A8bre", vec![2]),
        ("q=textbook", vec![]),
        ("q=textbook&lang=en", vec![2]),
        ("q=attente", vec![]),
    ] {
        let (status, _, body) = get_json(&app, &format!("/v1/resources?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let expected: Vec<Value> = expected.into_iter().map(Value::from).collect();
        assert_eq!(field(&body, "id"), expected, "{query}");
    }
}

#[tokio::test]
async fn news_lists_published_announcements_only() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, headers, body) = get_json(&app, "/v1/news").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(field(&body, "slug"), [json!("le-russe-arrive-sur-pulsars")]);
    assert_eq!(body["next_cursor"], Value::Null);
    assert_eq!(
        body["data"][0],
        json!({
            "id": 1,
            "slug": "le-russe-arrive-sur-pulsars",
            "lang": "fr",
            "title": "Le russe arrive sur Pulsars",
            "summary": "L'interface parle désormais trois langues.",
            "author": "jikuji",
            "published_at": "2026-10-20T08:00:00Z",
            "updated_at": "2026-10-20T08:00:00Z",
            "web_url": "https://pulsars.science/actualite/le-russe-arrive-sur-pulsars"
        })
    );
    // The list carries no content.
    assert!(body["data"][0].get("content_md").is_none());

    let (_, _, body) = get_json(&app, "/v1/news?q=russe").await;
    assert_eq!(field(&body, "slug").len(), 1);
    let (_, _, body) = get_json(&app, "/v1/news?q=pr%C3%A9paration").await;
    assert_eq!(field(&body, "slug").len(), 0, "drafts are not searchable");
}

#[tokio::test]
async fn news_detail_falls_back_to_french_per_field() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, _, body) = get_json(&app, "/v1/news/le-russe-arrive-sur-pulsars").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"]["content_md"],
        "Après le français et l'anglais, **le russe**.\n\nBonne lecture !"
    );

    let (_, _, body) = get_json(&app, "/v1/news/le-russe-arrive-sur-pulsars?lang=en").await;
    let data = &body["data"];
    assert_eq!(data["lang"], "en");
    assert_eq!(data["title"], "Russian comes to Pulsars");
    // An empty translated field keeps the French text.
    assert_eq!(
        data["summary"],
        "L'interface parle désormais trois langues."
    );
    assert_eq!(
        data["content_md"],
        "After French and English, **Russian**.\n\nEnjoy!"
    );
}

#[tokio::test]
async fn unpublished_content_answers_404() {
    let Some(app) = content_app().await else {
        return;
    };
    for path in [
        "/v1/courses/demo-draft-course",
        "/v1/courses/demo-future-course",
        "/v1/courses/les-bases-du-calcul-2",
        "/v1/courses/demo-draft-course/lessons/brouillon",
        "/v1/courses/les-bases-du-calcul/lessons/brouillon",
        // A lesson of another course.
        "/v1/courses/les-bases-du-calcul/lessons/mendeleiev",
        "/v1/exercises/6",
        "/v1/exercises/999999",
        "/v1/exercises/abc",
        "/v1/exercises/0",
        "/v1/exams/diplome-national-du-brevet-2026-mathematiques-serie-generale",
        "/v1/resources/demo-pending-resource",
        "/v1/news/demo-draft-news",
        "/v1/news/demo-future-news",
    ] {
        let (status, headers, body) = get_json(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
        // Public caches may keep a 404 for a minute.
        assert_eq!(
            headers[header::CACHE_CONTROL],
            "public, max-age=60",
            "{path}"
        );
        assert_eq!(
            body["type"],
            "https://api.pulsars.science/problems/not-found"
        );
        assert_eq!(body["instance"], path);
    }
    let (_, _, body) = get_json(&app, "/v1/courses/les-bases-du-calcul-2").await;
    assert_eq!(
        body["detail"],
        "No published course has the slug 'les-bases-du-calcul-2'."
    );
}

#[tokio::test]
async fn invalid_parameters_are_all_reported() {
    let Some(app) = content_app().await else {
        return;
    };
    let (status, headers, body) =
        get_json(&app, "/v1/courses?limit=500&lang=de&topic=analyse&q=x").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        body,
        json!({
            "type": "https://api.pulsars.science/problems/invalid-parameter",
            "title": "Invalid parameter",
            "status": 400,
            "detail": "One or more query parameters are invalid.",
            "errors": [
                {"parameter": "lang", "detail": "must be one of: fr, en"},
                {"parameter": "limit", "detail": "must be between 1 and 100"},
                {"parameter": "topic", "detail": "requires `subject`"},
                {"parameter": "q", "detail": "must be between 2 and 100 characters"},
            ],
        })
    );

    for (path, parameter) in [
        ("/v1/catalog?lang=EN", "lang"),
        ("/v1/courses?limit=0", "limit"),
        ("/v1/courses?limit=101", "limit"),
        ("/v1/courses?limit=ten", "limit"),
        ("/v1/courses?difficulty=facile", "difficulty"),
        ("/v1/courses?level=lyc%C3%A9e", "level"),
        ("/v1/courses?available_in=de", "available_in"),
        ("/v1/courses?subject=", "subject"),
        ("/v1/courses?lang=fr&lang=en", "lang"),
        ("/v1/exercises?type=vrai_faux", "type"),
        ("/v1/exercises?topic=analyse", "topic"),
        ("/v1/exams?type=examen", "type"),
        ("/v1/exams?subject=histoire", "subject"),
        ("/v1/exams?country=fr", "country"),
        ("/v1/exams?country=FRA", "country"),
        ("/v1/resources?mode=file", "mode"),
        ("/v1/resources?doc_lang=fr-FR-x-long", "doc_lang"),
        ("/v1/courses/les-bases-du-calcul?lang=de", "lang"),
        ("/v1/exercises/1?lang=de", "lang"),
    ] {
        let (status, _, body) = get_json(&app, path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
        assert_eq!(body["errors"][0]["parameter"], parameter, "{path}");
    }

    // Edges that are valid, and unknown parameters, which are ignored.
    for path in [
        "/v1/courses?limit=1",
        "/v1/courses?limit=100",
        "/v1/courses?q=ab",
        "/v1/courses?unknown=1&utm_source=x",
    ] {
        let (status, _, body) = get_json(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
    }
}

#[tokio::test]
async fn conditional_requests_answer_304() {
    let Some(app) = content_app().await else {
        return;
    };
    let (_, headers, _) = get_json(&app, "/v1/courses/les-bases-du-calcul").await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    assert!(etag.starts_with('"'), "strong ETag: {etag}");

    let request = Request::get("/v1/courses/les-bases-du-calcul")
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()[header::ETAG], etag.as_str());
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=300, stale-while-revalidate=60"
    );
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert!(body.is_empty());

    // Another language is another representation.
    let (_, headers, _) = get_json(&app, "/v1/courses/les-bases-du-calcul?lang=en").await;
    assert_ne!(headers[header::ETAG].to_str().unwrap(), etag);
    let request = Request::get("/v1/courses/les-bases-du-calcul?lang=en")
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn content_allows_any_origin_and_requires_json() {
    let Some(app) = content_app().await else {
        return;
    };
    let request = Request::get("/v1/exams")
        .header(header::ORIGIN, "https://anyone.example")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS));

    let preflight = Request::options("/v1/courses")
        .header(header::ORIGIN, "https://anyone.example")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(&app, preflight).await;
    assert!(status.is_success(), "{status}");
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");

    let request = Request::get("/v1/exams")
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/not-acceptable"
    );
}

/// Every public response the demo data can produce, in both languages.
async fn crawl(app: &Router) -> String {
    let mut paths: Vec<String> = vec![
        "/v1/catalog".into(),
        "/v1/courses?limit=100".into(),
        "/v1/exercises?limit=100".into(),
        "/v1/exams?limit=100".into(),
        "/v1/resources?limit=100".into(),
        "/v1/news?limit=100".into(),
    ];
    for (list, detail) in [
        ("/v1/courses?limit=100", "/v1/courses/"),
        ("/v1/exams?limit=100", "/v1/exams/"),
        ("/v1/resources?limit=100", "/v1/resources/"),
        ("/v1/news?limit=100", "/v1/news/"),
    ] {
        let (_, _, body) = get_json(app, list).await;
        for slug in field(&body, "slug") {
            paths.push(format!("{detail}{}", slug.as_str().unwrap()));
        }
    }
    for id in 1..=10 {
        paths.push(format!("/v1/exercises/{id}"));
    }
    let (_, _, course) = get_json(app, "/v1/courses/les-bases-du-calcul").await;
    for chapter in course["data"]["chapters"].as_array().unwrap() {
        for lesson in chapter["lessons"].as_array().unwrap() {
            paths.push(format!(
                "/v1/courses/les-bases-du-calcul/lessons/{}",
                lesson["slug"].as_str().unwrap()
            ));
        }
    }
    paths.push("/v1/courses/chimie-le-tableau-periodique/lessons/mendeleiev".into());

    let mut dump = String::new();
    for path in paths {
        for lang in ["fr", "en"] {
            let separator = if path.contains('?') { '&' } else { '?' };
            let (_, _, body) = get_json(app, &format!("{path}{separator}lang={lang}")).await;
            dump.push_str(&body.to_string());
            dump.push('\n');
        }
    }
    dump
}

#[tokio::test]
async fn no_draft_answer_or_personal_data_is_ever_served() {
    let Some(app) = content_app().await else {
        return;
    };
    let dump = crawl(&app).await;
    assert!(
        dump.contains("les-bases-du-calcul"),
        "the crawl saw content"
    );
    assert!(dump.contains("Mendeleïev"), "the crawl saw every lesson");
    assert!(
        dump.contains("Russian comes to Pulsars"),
        "the crawl saw the news"
    );

    // Markers planted in dev/db/30-demo-data.sql, keys of answer fields,
    // unpublished slugs and storage paths.
    for marker in [
        "SOLUTION-MARKER",
        "CORRIGE-MARKER",
        "PROMPT-MARKER",
        "solution",
        "correctes",
        "acceptees",
        "corrige",
        "\"correct\"",
        "example.invalid",
        "demofriendtoken",
        "not-a-real-hash",
        "Terminale B (private)",
        "\"email\"",
        "demo-draft-course",
        "demo-future-course",
        "demo-pending-resource",
        "demo-draft-news",
        "demo-future-news",
        "NEWS-DRAFT-MARKER",
        "NEWS-FUTURE-MARKER",
        "diplome-national-du-brevet",
        "Brouillon",
        "Invisible",
        "Exercice brouillon",
        "examens/bac-2026-maths-jour-1",
        "ressources/ic901972h.pdf",
        "ressources/pending.pdf",
        "sujet.pdf",
        "\"statut\"",
        "\"vues\"",
        "\"telechargements\"",
        "auteur_id",
        "ajoute_par",
    ] {
        assert!(!dump.contains(marker), "`{marker}` was served");
    }
}

// --- Rate limits (real Redis) ---------------------------------------------

fn test_redis_url() -> Option<String> {
    match std::env::var("TEST_REDIS_URL")
        .ok()
        .filter(|v| !v.is_empty())
    {
        Some(url) => Some(url),
        None if std::env::var("REQUIRE_INTEGRATION").as_deref() == Ok("1") => {
            panic!("REQUIRE_INTEGRATION=1 but TEST_REDIS_URL is not set")
        }
        None => {
            eprintln!("skipped: TEST_REDIS_URL is not set");
            None
        }
    }
}

async fn wait_for_redis(state: &AppState) {
    use fred::prelude::ClientLike as _;
    tokio::time::timeout(Duration::from_secs(5), state.redis.wait_for_connect())
        .await
        .expect("redis connects within 5 s")
        .expect("redis connects");
}

/// A client IP no other test (or earlier run) uses.
fn fresh_ip() -> String {
    let id = uuid::Uuid::new_v4().as_u128();
    format!("2001:db8::{:x}:{:x}", (id >> 16) & 0xffff, id & 0xffff)
}

fn from_ip(path: &str, ip: &str) -> Request<Body> {
    Request::get(path)
        .header("x-forwarded-for", format!("203.0.113.9, {ip}"))
        .body(Body::empty())
        .unwrap()
}

fn header_u64(headers: &axum::http::HeaderMap, name: &str) -> u64 {
    headers[name].to_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn anonymous_clients_are_limited_per_ip() {
    let Some(redis_url) = test_redis_url() else {
        return;
    };
    // `limit=500` answers 400 before any database access: no PostgreSQL
    // needed, and the request is still counted.
    let (app, state) = app(config(
        DEAD_POSTGRES,
        &redis_url,
        &[("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "3")],
    ))
    .await;
    wait_for_redis(&state).await;
    let ip = fresh_ip();
    let path = "/v1/courses?limit=500";

    for remaining in [2, 1, 0] {
        let (status, headers, _) = send(&app, from_ip(path, &ip)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(headers["ratelimit-limit"], "3");
        assert_eq!(header_u64(&headers, "ratelimit-remaining"), remaining);
        let reset = header_u64(&headers, "ratelimit-reset");
        assert!((1..=60).contains(&reset), "{reset}");
    }

    let request = Request::get(path)
        .header("x-forwarded-for", format!("203.0.113.9, {ip}"))
        .header(header::ORIGIN, "https://anyone.example")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let retry_after = header_u64(&headers, "retry-after");
    assert!((1..=60).contains(&retry_after), "{retry_after}");
    assert_eq!(header_u64(&headers, "ratelimit-remaining"), 0);
    // Browsers can read the problem.
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert_eq!(
        body,
        json!({
            "type": "https://api.pulsars.science/problems/rate-limited",
            "title": "Too many requests",
            "status": 429,
            "detail": "Limit of 3 requests per minute reached.",
        })
    );

    // Another IP has its own counter; the forged first entry is ignored.
    let (status, _, _) = send(&app, from_ip(path, &fresh_ip())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // /healthz is never limited.
    let (status, headers, _) = send(&app, from_ip("/healthz", &ip)).await;
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(!headers.contains_key("ratelimit-limit"));
}

#[tokio::test]
async fn authenticated_clients_are_limited_per_sub() {
    let Some(redis_url) = test_redis_url() else {
        return;
    };
    let server = JwksServer::start().await;
    let (app, state) = app(config(
        DEAD_POSTGRES,
        &redis_url,
        &[
            ("JWKS_URL", server.url.as_str()),
            ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1000"),
            ("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "2"),
        ],
    ))
    .await;
    state.jwks.refresh().await.expect("JWKS loads");
    wait_for_redis(&state).await;

    // A `sub` naming no account answers 404 without touching PostgreSQL.
    let sub = format!("rate-{}", uuid::Uuid::new_v4().simple());
    let token = server.token(&sub);
    let ip = fresh_ip();
    let request = |path: &str, ip: &str| {
        Request::get(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header("x-forwarded-for", ip.to_owned())
            .body(Body::empty())
            .unwrap()
    };

    let (status, headers, body) = send(&app, request("/v1/me", &ip)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(headers["ratelimit-limit"], "2");
    assert_eq!(headers["ratelimit-remaining"], "1");
    // The counter follows the member across IPs and endpoints.
    let (status, headers, _) = send(&app, request("/v1/courses?limit=500", &fresh_ip())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(headers["ratelimit-remaining"], "0");
    let (status, headers, body) = send(&app, request("/v1/me", &ip)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(headers.contains_key(header::RETRY_AFTER));
    assert_eq!(body["detail"], "Limit of 2 requests per minute reached.");

    // Without a valid token, the same IP is still under its own limit.
    let (status, headers, body) = send(&app, from_ip("/v1/me", &ip)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(headers["ratelimit-limit"], "1000");
    let forged = testing::token(&testing::signing_key(), KID, &sub, now());
    let bad = Request::get("/v1/me")
        .header(header::AUTHORIZATION, format!("Bearer {forged}"))
        .header("x-forwarded-for", ip.clone())
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&app, bad).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["reason"], "bad_signature");
    assert_eq!(headers["ratelimit-limit"], "1000");
}

#[tokio::test]
async fn limits_fail_open_when_redis_is_down() {
    let (app, _) = app(config(
        DEAD_POSTGRES,
        DEAD_REDIS,
        &[("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1")],
    ))
    .await;
    let ip = fresh_ip();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let (status, headers, _) = send(&app, from_ip("/v1/courses?limit=500", &ip)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!headers.contains_key("ratelimit-limit"));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "never waits for Redis"
        );
    }
}
