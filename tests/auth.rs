//! `/v1/auth/*` against the real test database (`docs/contract/auth-sessions.md`).
//!
//! Same services as `tests/http.rs`: `TEST_DATABASE_URL`, `TEST_REDIS_URL`,
//! and `TEST_MIGRATION_DATABASE_URL` to plant accounts the API cannot write
//! itself (one signed up through Google, without a password). Tests that
//! change an account create their own, so they can run in any order and
//! again on the same database.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use pulsars_api::auth::Jwks;
use pulsars_api::config::Config;
use pulsars_api::game::{Game, GameSettings, store};
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const DEAD_JWKS: &str = "http://127.0.0.1:1/.well-known/jwks.json";
const SERVICE_KEY: &str = "test-service-key-0123456789abcdef-0123";
/// ada's password in dev/db/30-demo-data.sql, hashed by PHP.
const ADA_PASSWORD: &str = "secret-pass";

struct Env {
    app: Router,
    state: AppState,
    database_url: String,
}

async fn env() -> Option<Env> {
    env_with(&[]).await
}

/// The test environment, with more variables (a signing key).
async fn env_with(extra: &[(&str, &str)]) -> Option<Env> {
    let db = std::env::var("TEST_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty());
    let redis_url = std::env::var("TEST_REDIS_URL")
        .ok()
        .filter(|v| !v.is_empty());
    let (Some(database_url), Some(redis_url)) = (db, redis_url) else {
        if std::env::var("REQUIRE_INTEGRATION").as_deref() == Ok("1") {
            panic!("REQUIRE_INTEGRATION=1 but TEST_DATABASE_URL or TEST_REDIS_URL is not set");
        }
        eprintln!("skipped: TEST_DATABASE_URL and TEST_REDIS_URL are not set");
        return None;
    };
    migrate(&database_url).await;
    let config = Config::from_vars(
        [
            ("PULSARS_ENV", "development"),
            ("DATABASE_URL", database_url.as_str()),
            ("REDIS_URL", redis_url.as_str()),
            ("JWKS_URL", DEAD_JWKS),
            ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1000000"),
            ("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "1000000"),
            ("SERVICE_KEY", SERVICE_KEY),
            ("BCRYPT_COST", "4"),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(|(k, v)| (k.to_owned(), v.to_owned())),
    )
    .unwrap();
    let jwks = Jwks::new(&config).unwrap();
    let db = db::connect_lazy(&config).unwrap();
    let redis = redis::connect(&config).unwrap();
    {
        use fred::prelude::ClientLike as _;
        tokio::time::timeout(std::time::Duration::from_secs(5), redis.wait_for_connect())
            .await
            .expect("redis connects within 5 s")
            .expect("redis connects");
    }
    let state = AppState {
        game: Game::start(db.clone(), GameSettings::default()),
        db,
        redis,
        config: Arc::new(config),
        jwks,
    };
    Some(Env {
        app: routes::router(state.clone()),
        state,
        database_url,
    })
}

/// The migrations, as `pulsars_migrate`, as `tests/http.rs` does.
async fn migrate(database_url: &str) {
    let pool = sqlx::PgPool::connect(&owner_url(database_url))
        .await
        .unwrap();
    store::migrate(&pool).await.expect("migrations apply");
    pool.close().await;
}

fn owner_url(database_url: &str) -> String {
    std::env::var("TEST_MIGRATION_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| database_url.to_owned())
}

fn unique() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    id[..10].to_owned()
}

/// A documentation address no other test uses: per-address limits do not
/// mix.
fn fresh_ip() -> String {
    let id = uuid::Uuid::new_v4().as_u128();
    format!("2001:db8::{:x}:{:x}", (id >> 16) & 0xffff, id & 0xffff)
}

struct Call<'a> {
    method: &'a str,
    path: &'a str,
    body: Option<Value>,
    session: Option<&'a str>,
    service: bool,
    ip: Option<&'a str>,
}

impl<'a> Call<'a> {
    fn new(method: &'a str, path: &'a str) -> Self {
        Self {
            method,
            path,
            body: None,
            session: None,
            service: false,
            ip: None,
        }
    }
    fn body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
    fn session(mut self, token: &'a str) -> Self {
        self.session = Some(token);
        self
    }
    fn service(mut self) -> Self {
        self.service = true;
        self
    }
    fn ip(mut self, ip: &'a str) -> Self {
        self.ip = Some(ip);
        self
    }
    async fn send(self, app: &Router) -> (StatusCode, HeaderMap, Value) {
        let mut request = Request::builder()
            .method(self.method)
            .uri(self.path)
            .header(header::USER_AGENT, "auth-tests");
        if let Some(token) = self.session {
            request = request.header("pulsars-session", token);
        }
        if self.service {
            request = request.header("pulsars-service-key", SERVICE_KEY);
        }
        let ip = self.ip.map_or_else(fresh_ip, str::to_owned);
        request = request.header("x-forwarded-for", ip);
        let request = match self.body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

fn token(body: &Value) -> String {
    body["data"]["session"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("no session token in {body}"))
        .to_owned()
}

fn codes(body: &Value) -> Vec<(String, String)> {
    body["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("no errors in {body}"))
        .iter()
        .map(|e| {
            (
                e["parameter"].as_str().unwrap().to_owned(),
                e["code"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Signs up a new account; its pseudo, e-mail, password and session.
async fn sign_up(app: &Router) -> (String, String, String, String) {
    let pseudo = format!("Test_{}", unique());
    let email = format!("{}@example.invalid", pseudo.to_lowercase());
    let password = format!("pass-{}", unique());
    let (status, _, body) = Call::new("POST", "/v1/auth/register")
        .body(json!({
            "pseudo": pseudo, "email": email, "password": password,
            "password_confirmation": password, "locale": "ru"
        }))
        .service()
        .send(app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (pseudo, email, password, token(&body))
}

async fn login(app: &Router, login: &str, password: &str) -> (StatusCode, Value) {
    let (status, _, body) = Call::new("POST", "/v1/auth/login")
        .body(json!({"login": login, "password": password}))
        .service()
        .send(app)
        .await;
    (status, body)
}

async fn session(app: &Router, token: &str) -> (StatusCode, Value) {
    let (status, _, body) = Call::new("GET", "/v1/auth/session")
        .session(token)
        .send(app)
        .await;
    (status, body)
}

#[tokio::test]
async fn members_log_in_by_email_or_pseudo_in_any_case() {
    let Some(env) = env().await else { return };
    for login_as in [
        "demo-ada@example.invalid",
        "DEMO-Ada@Example.invalid",
        "ada",
        "ADA",
    ] {
        let (status, body) = login(&env.app, login_as, ADA_PASSWORD).await;
        assert_eq!(status, StatusCode::OK, "{login_as}: {body}");
        assert_eq!(
            body["data"]["user"],
            json!({"id": "2", "pseudo": "ada", "email": "demo-ada@example.invalid",
                   "email_verified": true, "onboarded": true, "locale": "en"})
        );
        let session = &body["data"]["session"];
        assert_eq!(session["token"].as_str().unwrap().len(), 43);
        assert_eq!(session["remember"], false);
    }

    let (status, headers, body) = Call::new("POST", "/v1/auth/login")
        .body(json!({"login": "ada", "password": ADA_PASSWORD, "remember": true}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(body["data"]["session"]["remember"], true);
    let expires: chrono::DateTime<chrono::Utc> = body["data"]["session"]["expires_at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(expires > chrono::Utc::now() + chrono::Duration::days(300));
}

#[tokio::test]
async fn wrong_passwords_unknown_logins_and_social_accounts_look_the_same() {
    let Some(env) = env().await else { return };
    let social = plant_social_account(&env).await;
    let mut seen = Vec::new();
    for (login_as, password) in [
        ("ada", "wrong-password"),
        ("nobody-at-all", ADA_PASSWORD),
        (social.as_str(), ""),
        (social.as_str(), "anything-at-all"),
    ] {
        let (status, body) = login(&env.app, login_as, password).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{login_as}");
        seen.push(body);
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
    assert_eq!(
        seen[0],
        json!({
            "type": "https://api.pulsars.science/problems/invalid-credentials",
            "title": "Invalid credentials",
            "status": 422,
            "detail": "These credentials do not match any account."
        })
    );
}

#[tokio::test]
async fn login_attempts_are_limited_per_login_and_address_even_for_the_site() {
    let Some(env) = env().await else { return };
    let ip = fresh_ip();
    let who = format!("someone-{}", unique());
    for _ in 0..5 {
        let (status, _, _) = Call::new("POST", "/v1/auth/login")
            .body(json!({"login": who, "password": "x"}))
            .service()
            .ip(&ip)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (status, headers, body) = Call::new("POST", "/v1/auth/login")
        .body(json!({"login": who.to_uppercase(), "password": "x"}))
        .service()
        .ip(&ip)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/rate-limited"
    );
    assert!(headers.contains_key(header::RETRY_AFTER));
    // Another address is not held back.
    let (status, _, _) = Call::new("POST", "/v1/auth/login")
        .body(json!({"login": who, "password": "x"}))
        .service()
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn sign_up_reports_every_mistake_with_a_code() {
    let Some(env) = env().await else { return };
    let (status, _, body) = Call::new("POST", "/v1/auth/register")
        .body(
            json!({"pseudo": "a b", "email": "nope", "password": "short",
                     "password_confirmation": "short", "locale": "de"}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/invalid-body"
    );
    assert_eq!(
        codes(&body),
        [
            ("pseudo".to_owned(), "invalid_format".to_owned()),
            ("email".to_owned(), "invalid_email".to_owned()),
            ("password".to_owned(), "too_short".to_owned()),
            ("locale".to_owned(), "invalid".to_owned()),
        ]
    );

    let (_, _, body) = Call::new("POST", "/v1/auth/register")
        .body(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        [
            ("pseudo".to_owned(), "required".to_owned()),
            ("email".to_owned(), "required".to_owned()),
            ("password".to_owned(), "required".to_owned()),
        ]
    );

    // Taken, whatever the case; the confirmation is checked too.
    let (_, _, body) = Call::new("POST", "/v1/auth/register")
        .body(json!({"pseudo": "ADA", "email": "Demo-Ada@example.invalid",
                     "password": "long-enough", "password_confirmation": "long-enougH"}))
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        [
            ("pseudo".to_owned(), "taken".to_owned()),
            ("email".to_owned(), "taken".to_owned()),
            ("password".to_owned(), "mismatch".to_owned()),
        ]
    );
}

#[tokio::test]
async fn sign_up_creates_an_account_laravel_can_read_and_opens_a_session() {
    let Some(env) = env().await else { return };
    let (pseudo, email, password, token) = sign_up(&env.app).await;

    let (status, body) = session(&env.app, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let user = &body["data"]["user"];
    assert_eq!(user["pseudo"], pseudo);
    assert_eq!(user["email"], email);
    assert_eq!(user["email_verified"], false);
    assert_eq!(user["onboarded"], false);
    assert_eq!(user["locale"], "ru");
    assert!(body["data"]["session"].get("token").is_none());

    // As Laravel's CreateNewUser writes it: name = pseudo, a `$2y$` hash.
    let owner = sqlx::PgPool::connect(&owner_url(&env.database_url))
        .await
        .unwrap();
    let (name, hash): (String, String) =
        sqlx::query_as("SELECT name::text, password::text FROM site.users WHERE pseudo = $1")
            .bind(&pseudo)
            .fetch_one(&owner)
            .await
            .unwrap();
    assert_eq!(name, pseudo);
    assert!(hash.starts_with("$2y$04$"), "{hash}");
    assert!(bcrypt::verify(&password, &hash).unwrap());

    let (status, _) = login(&env.app, &pseudo.to_uppercase(), &password).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_end_on_logout_and_logging_out_twice_is_fine() {
    let Some(env) = env().await else { return };
    let (_, body) = login(&env.app, "ada", ADA_PASSWORD).await;
    let token = token(&body);
    assert_eq!(session(&env.app, &token).await.0, StatusCode::OK);

    for _ in 0..2 {
        let (status, headers, _) = Call::new("POST", "/v1/auth/logout")
            .session(&token)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    }
    let (status, body) = session(&env.app, &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/session-invalid"
    );

    for bad in ["", "short", &"x".repeat(43)] {
        let (status, headers, _) = Call::new("GET", "/v1/auth/session")
            .session(bad)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{bad}");
        assert!(headers.contains_key(header::WWW_AUTHENTICATE));
    }
    let (status, _, _) = Call::new("GET", "/v1/auth/session").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sessions_slide_and_expire() {
    let Some(env) = env().await else { return };
    let (_, _, _, token) = sign_up(&env.app).await;
    let owner = sqlx::PgPool::connect(&owner_url(&env.database_url))
        .await
        .unwrap();
    let hash = sha(&token);

    // Used five minutes ago: using it again moves the expiry.
    sqlx::query(
        "UPDATE site.auth_sessions SET last_used_at = now() - interval '5 minutes',
                expires_at = now() + interval '115 minutes' WHERE token_hash = $1",
    )
    .bind(&hash)
    .execute(&owner)
    .await
    .unwrap();
    let (status, body) = session(&env.app, &token).await;
    assert_eq!(status, StatusCode::OK);
    let expires: chrono::DateTime<chrono::Utc> = body["data"]["session"]["expires_at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        expires > chrono::Utc::now() + chrono::Duration::minutes(119),
        "{expires}"
    );

    sqlx::query("UPDATE site.auth_sessions SET expires_at = now() - interval '1 second' WHERE token_hash = $1")
        .bind(&hash)
        .execute(&owner)
        .await
        .unwrap();
    assert_eq!(session(&env.app, &token).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn only_the_site_opens_sessions_for_an_account() {
    let Some(env) = env().await else { return };
    let (status, _, body) = Call::new("POST", "/v1/auth/sessions")
        .body(json!({"user_id": "2", "remember": true}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, _, body) = Call::new("POST", "/v1/auth/sessions")
        .body(json!({"user_id": "2", "remember": true}))
        .service()
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["user"]["pseudo"], "ada");
    assert_eq!(body["data"]["session"]["remember"], true);
    assert_eq!(session(&env.app, &token(&body)).await.0, StatusCode::OK);

    let (status, _, _) = Call::new("POST", "/v1/auth/sessions")
        .body(json!({"user_id": "999999"}))
        .service()
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, body) = Call::new("POST", "/v1/auth/sessions")
        .body(json!({"user_id": "ada"}))
        .service()
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("user_id".to_owned(), "invalid".to_owned())]);
}

#[tokio::test]
async fn changing_the_password_keeps_this_session_and_ends_the_others() {
    let Some(env) = env().await else { return };
    let (pseudo, _, password, first) = sign_up(&env.app).await;
    let (_, body) = login(&env.app, &pseudo, &password).await;
    let second = token(&body);

    let change = |current: Value, new: &str| json!({"current_password": current, "password": new, "password_confirmation": new});
    let (status, _, body) = Call::new("PUT", "/v1/auth/password")
        .body(change(json!("not-it"), "brand-new-pass"))
        .session(&first)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [("current_password".to_owned(), "incorrect".to_owned())]
    );
    let (_, _, body) = Call::new("PUT", "/v1/auth/password")
        .body(change(Value::Null, "short"))
        .session(&first)
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        [
            ("current_password".to_owned(), "required".to_owned()),
            ("password".to_owned(), "too_short".to_owned()),
        ]
    );

    let (status, _, body) = Call::new("PUT", "/v1/auth/password")
        .body(change(json!(password), "brand-new-pass"))
        .session(&first)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(session(&env.app, &first).await.0, StatusCode::OK);
    assert_eq!(session(&env.app, &second).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login(&env.app, &pseudo, &password).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        login(&env.app, &pseudo, "brand-new-pass").await.0,
        StatusCode::OK
    );

    let (status, _, _) = Call::new("PUT", "/v1/auth/password")
        .body(change(json!("brand-new-pass"), "another-pass"))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a session is required");
}

#[tokio::test]
async fn an_account_made_through_google_sets_a_password_without_a_current_one() {
    let Some(env) = env().await else { return };
    let pseudo = plant_social_account(&env).await;
    let id = id_of(&env, &pseudo).await;
    let (_, _, body) = Call::new("POST", "/v1/auth/sessions")
        .body(json!({"user_id": id.to_string()}))
        .service()
        .send(&env.app)
        .await;
    let token = token(&body);

    let (status, _, body) = Call::new("PUT", "/v1/auth/password")
        .body(json!({"password": "first-password", "password_confirmation": "first-password"}))
        .session(&token)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        login(&env.app, &pseudo, "first-password").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_reset_by_the_site_ends_every_session() {
    let Some(env) = env().await else { return };
    let (pseudo, _, _, token) = sign_up(&env.app).await;
    let id = id_of(&env, &pseudo).await;
    let reset = json!({"user_id": id.to_string(), "password": "after-reset",
                       "password_confirmation": "after-reset"});

    let (status, _, _) = Call::new("POST", "/v1/auth/password/reset")
        .body(reset.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, body) = Call::new("POST", "/v1/auth/password/reset")
        .body(reset)
        .service()
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(session(&env.app, &token).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login(&env.app, &pseudo, "after-reset").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn bodies_must_be_json_and_fields_known() {
    let Some(env) = env().await else { return };
    let request = Request::post("/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("login=ada&password=x"))
        .unwrap();
    let response = env.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let (status, _, _) = Call::new("POST", "/v1/auth/login")
        .body(json!({"login": "ada", "password": "x", "admin": true}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let _ = &env.state;
}

/// An account with no password, as Google sign-up leaves it.
async fn plant_social_account(env: &Env) -> String {
    let pseudo = format!("Google_{}", unique());
    let owner = sqlx::PgPool::connect(&owner_url(&env.database_url))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO site.users (pseudo, name, email, password, email_verified_at, created_at, updated_at)
         VALUES ($1, $1, $2, NULL, now(), now(), now())",
    )
    .bind(&pseudo)
    .bind(format!("{}@example.invalid", pseudo.to_lowercase()))
    .execute(&owner)
    .await
    .unwrap();
    pseudo
}

async fn id_of(env: &Env, pseudo: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM site.users WHERE pseudo = $1")
        .bind(pseudo)
        .fetch_one(&env.state.db)
        .await
        .unwrap()
}

fn sha(token: &str) -> Vec<u8> {
    use sha2::Digest as _;
    sha2::Sha256::digest(token.as_bytes()).to_vec()
}

// --- Game tokens (`POST /v1/auth/token`) ------------------------------------

const GAME_ORIGIN: &str = "http://localhost:5173";
const TOKEN_KID: &str = "2026-10-ed25519-7";
const TOKEN_SEED: [u8; 32] = [9; 32];

/// A key file in the format of `jwt-keygen`, with a fixed seed.
fn key_file() -> &'static str {
    use base64::Engine as _;
    let dir = std::env::temp_dir().join(format!("pulsars-token-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join(format!("{TOKEN_KID}.key"));
    std::fs::write(
        &path,
        base64::engine::general_purpose::STANDARD.encode(TOKEN_SEED),
    )
    .unwrap();
    Box::leak(path.to_str().unwrap().to_owned().into_boxed_str())
}

async fn token_env() -> Option<Env> {
    env_with(&[
        ("JWT_SIGNING_KEY_FILE", key_file()),
        ("JWT_SIGNING_KID", TOKEN_KID),
    ])
    .await
}

/// A request to `/v1/auth/token` with these headers.
async fn ask_token(
    app: &Router,
    method: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri("/v1/auth/token")
        .header("x-forwarded-for", fresh_ip());
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from("{}")).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn browser(cookie: &str) -> Vec<(&str, &str)> {
    vec![
        ("origin", GAME_ORIGIN),
        ("content-type", "application/json"),
        ("accept", "application/json"),
        ("sec-fetch-site", "same-site"),
        ("cookie", cookie),
    ]
}

/// A new account, verified and onboarded, with the `moderateur` role; its
/// id and session.
async fn player(env: &Env) -> (i64, String, String) {
    let (pseudo, _, _, session) = sign_up(&env.app).await;
    let id = id_of(env, &pseudo).await;
    let owner = sqlx::PgPool::connect(&owner_url(&env.database_url))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE site.users SET email_verified_at = now(), onboarding_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&owner)
    .await
    .unwrap();
    sqlx::query("INSERT INTO site.role_user (role_id, user_id) SELECT id, $1 FROM site.roles WHERE slug = 'moderateur'")
        .bind(id)
        .execute(&owner)
        .await
        .unwrap();
    (id, pseudo, session)
}

fn claims_of(token: &str) -> Value {
    use base64::Engine as _;
    let part = token.split('.').nth(1).unwrap();
    serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(part)
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn the_api_issues_game_tokens_from_the_session_cookie() {
    let Some(env) = token_env().await else { return };
    let (id, pseudo, session) = player(&env).await;
    let cookie = format!("theme=dark; pulsars_auth={session}");

    let (status, headers, body) = ask_token(&env.app, "POST", &browser(&cookie)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], GAME_ORIGIN);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
    assert!(headers.get_all(header::VARY).iter().any(|v| v == "Origin"));
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 300);
    let token = body["token"].as_str().unwrap();

    let claims = claims_of(token);
    assert_eq!(claims["iss"], "https://api.pulsars.science");
    assert_eq!(claims["aud"], "api.pulsars.science");
    assert_eq!(claims["sub"], id.to_string());
    assert_eq!(claims["name"], pseudo);
    assert_eq!(claims["locale"], "ru");
    assert_eq!(claims["roles"], json!(["moderateur"]));
    assert_eq!(
        claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap(),
        300
    );
    assert_eq!(claims["iat"], claims["nbf"]);
    assert!(uuid::Uuid::parse_str(claims["jti"].as_str().unwrap()).is_ok());
    let expected = chrono::DateTime::from_timestamp(claims["exp"].as_i64().unwrap(), 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    assert_eq!(body["expires_at"], expected);

    // The API's own verifier takes it, as every member endpoint does.
    let header = format!("Bearer {token}");
    let verified = pulsars_api::auth::authenticate(
        Some(header.as_bytes()),
        &env.state.jwks,
        pulsars_api::auth::Expected {
            issuer: &env.state.config.jwt_issuer,
            audience: &env.state.config.jwt_audience,
        },
        env.state.config.token_signer.as_ref(),
        chrono::Utc::now().timestamp(),
    )
    .await
    .expect("verified");
    assert_eq!(verified.roles, ["moderateur"]);
    let request = Request::get("/v1/me")
        .header(header::AUTHORIZATION, &header)
        .body(Body::empty())
        .unwrap();
    let response = env.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // From a server: the session header, no `Origin`.
    let (status, headers, body) = ask_token(
        &env.app,
        "POST",
        &[
            ("content-type", "application/json"),
            ("pulsars-session", &session),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
}

#[tokio::test]
async fn game_tokens_are_refused_to_other_sites_and_unfinished_accounts() {
    let Some(env) = token_env().await else { return };
    let (_, _, session) = player(&env).await;
    let cookie = format!("pulsars_auth={session}");

    // Preflight: allowed origin only.
    let (status, headers, _) = ask_token(
        &env.app,
        "OPTIONS",
        &[
            ("origin", GAME_ORIGIN),
            ("access-control-request-method", "POST"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], GAME_ORIGIN);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_METHODS], "POST");
    assert_eq!(
        headers[header::ACCESS_CONTROL_ALLOW_HEADERS],
        "Content-Type, Accept"
    );
    assert_eq!(headers[header::ACCESS_CONTROL_MAX_AGE], "600");
    let (status, headers, _) = ask_token(
        &env.app,
        "OPTIONS",
        &[
            ("origin", "https://evil.example"),
            ("access-control-request-method", "POST"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));

    let mut evil = browser(&cookie);
    evil[0] = ("origin", "https://evil.example");
    let (status, headers, _) = ask_token(&env.app, "POST", &evil).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));

    let no_origin: Vec<_> = browser(&cookie).into_iter().skip(1).collect();
    let (status, _, _) = ask_token(&env.app, "POST", &no_origin).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a cookie needs an Origin");

    let mut form = browser(&cookie);
    form[1] = ("content-type", "text/plain");
    let (status, headers, _) = ask_token(&env.app, "POST", &form).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], GAME_ORIGIN);

    let mut cross = browser(&cookie);
    cross[3] = ("sec-fetch-site", "cross-site");
    let (status, _, _) = ask_token(&env.app, "POST", &cross).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, headers, body) = ask_token(&env.app, "POST", &browser("pulsars_auth=nope")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/session-invalid"
    );
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], GAME_ORIGIN);

    // A fresh sign-up: e-mail not verified, onboarding not done.
    let (_, _, _, fresh) = sign_up(&env.app).await;
    let fresh_cookie = format!("pulsars_auth={fresh}");
    let (status, _, body) = ask_token(&env.app, "POST", &browser(&fresh_cookie)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["detail"], "Onboarding required.");
}

#[tokio::test]
async fn game_tokens_are_limited_per_member() {
    let Some(env) = token_env().await else { return };
    let (_, _, session) = player(&env).await;
    let cookie = format!("pulsars_auth={session}");
    for _ in 0..30 {
        let (status, _, body) = ask_token(&env.app, "POST", &browser(&cookie)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, headers, _) = ask_token(&env.app, "POST", &browser(&cookie)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(headers.contains_key(header::RETRY_AFTER));
}

#[tokio::test]
async fn without_a_key_the_api_issues_no_token_and_publishes_none() {
    let Some(env) = env().await else { return };
    let (_, _, session) = player(&env).await;
    let cookie = format!("pulsars_auth={session}");
    let (status, _, body) = ask_token(&env.app, "POST", &browser(&cookie)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/token-unavailable"
    );

    let (status, headers, body) = Call::new("GET", "/.well-known/jwks.json")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/jwk-set+json");
    assert_eq!(body, json!({"keys": []}));
}

#[tokio::test]
async fn the_api_publishes_its_signing_key() {
    use base64::Engine as _;
    let Some(env) = token_env().await else { return };
    let (status, headers, body) = Call::new("GET", "/.well-known/jwks.json")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/jwk-set+json");
    assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let public = ed25519_dalek::SigningKey::from_bytes(&TOKEN_SEED).verifying_key();
    assert_eq!(
        body,
        json!({"keys": [{
            "kty": "OKP", "crv": "Ed25519",
            "x": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.as_bytes()),
            "kid": TOKEN_KID, "use": "sig", "alg": "EdDSA",
        }]})
    );
}
