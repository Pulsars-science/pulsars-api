//! What the integration tests of the site's routes share: the app on the
//! real test database, requests as a member, and members made to measure.
//!
//! Services: `TEST_DATABASE_URL`, `TEST_REDIS_URL`, and
//! `TEST_MIGRATION_DATABASE_URL` (migrations, and rows the tests plant).
//! Without them the tests skip, or fail under `REQUIRE_INTEGRATION=1`.
//! Every test makes its own members, so tests run in any order, in
//! parallel, and again on the same database.
//!
//!     mod common;
//!     use common::{Env, Call};
//!
//!     let Some(env) = Env::new().await else { return };
//!     let ada = env.member().verified().create().await;
//!     let (status, body) = Call::get("/v1/me").session(&ada.token).send(&env.app).await;

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use pulsars_api::auth::token::testing;
use pulsars_api::config::Config;
use pulsars_api::game::{Game, GameSettings, store};
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tower::ServiceExt as _;

pub const SERVICE_KEY: &str = "test-service-key-0123456789abcdef-0123";
pub const ORIGIN: &str = "https://pulsars.test";

pub struct Env {
    pub app: Router,
    pub state: AppState,
    /// As `pulsars_api`, like the service.
    pub db: sqlx::PgPool,
    /// As `pulsars_migrate`: to plant rows a test needs.
    pub owner: sqlx::PgPool,
}

impl Env {
    pub async fn new() -> Option<Self> {
        Self::with(&[]).await
    }

    /// The environment with more configuration variables.
    pub async fn with(extra: &[(&str, &str)]) -> Option<Self> {
        let var = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
        let (Some(database_url), Some(redis_url)) =
            (var("TEST_DATABASE_URL"), var("TEST_REDIS_URL"))
        else {
            if std::env::var("REQUIRE_INTEGRATION").as_deref() == Ok("1") {
                panic!("REQUIRE_INTEGRATION=1 but TEST_DATABASE_URL or TEST_REDIS_URL is not set");
            }
            eprintln!("skipped: TEST_DATABASE_URL and TEST_REDIS_URL are not set");
            return None;
        };
        let owner_url = var("TEST_MIGRATION_DATABASE_URL").unwrap_or_else(|| database_url.clone());
        let owner = sqlx::PgPool::connect(&owner_url).await.unwrap();
        store::migrate(&owner).await.expect("migrations apply");

        let mut config = Config::from_vars(
            [
                ("PULSARS_ENV", "development"),
                ("DATABASE_URL", database_url.as_str()),
                ("REDIS_URL", redis_url.as_str()),
                ("CORS_ALLOWED_ORIGINS", ORIGIN),
                ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1000000"),
                ("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "1000000"),
                ("SERVICE_KEY", SERVICE_KEY),
                ("ARGON2_MEMORY_KIB", "64"),
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
        )
        .unwrap();
        // The key a production server reads from `JWT_SIGNING_KEY_FILE`.
        config.token_signer = Some(testing::signer());
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
            http_daily: Default::default(),
            mailer: pulsars_api::mail::Mailer::recording(),
            game: Game::start(db.clone(), GameSettings::default()),
            db: db.clone(),
            redis,
            config: Arc::new(config),
        };
        Some(Self {
            app: routes::router(state.clone()),
            state,
            db,
            owner,
        })
    }

    /// A game token for `member`, signed with the app's key, with `roles`
    /// (`POST /v1/auth/token` reads them from the database).
    pub fn game_token(&self, member: &TestMember, roles: &[&str]) -> String {
        let signer = self.state.config.token_signer.as_ref().unwrap();
        let mut claims = testing::claims(&member.id.to_string(), chrono::Utc::now().timestamp());
        claims["name"] = Value::from(member.pseudo.as_str());
        claims["roles"] = Value::from(roles.to_vec());
        testing::sign(
            &signer.key,
            &serde_json::json!({"alg": "EdDSA", "typ": "JWT", "kid": signer.kid}),
            &claims,
        )
    }

    /// A new member, to shape before `create()`.
    pub fn member(&self) -> MemberBuilder<'_> {
        MemberBuilder {
            env: self,
            verified: false,
            onboarded: false,
            roles: Vec::new(),
            password: None,
        }
    }
}

/// Ten random characters, for pseudos, slugs and titles no other test uses.
pub fn unique() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    id[..10].to_owned()
}

/// A documentation address no other test uses: per-address limits do not mix.
pub fn fresh_ip() -> String {
    let id = uuid::Uuid::new_v4().as_u128();
    format!("2001:db8::{:x}:{:x}", (id >> 16) & 0xffff, id & 0xffff)
}

/// A member made for one test, with a live session.
#[derive(Debug, Clone)]
pub struct TestMember {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    /// The `Pulsars-Session` token.
    pub token: String,
}

pub struct MemberBuilder<'a> {
    env: &'a Env,
    verified: bool,
    onboarded: bool,
    roles: Vec<String>,
    password: Option<String>,
}

impl MemberBuilder<'_> {
    /// Address verified and welcome steps done: what most routes need.
    pub fn verified(mut self) -> Self {
        self.verified = true;
        self.onboarded = true;
        self
    }

    /// Address verified, welcome steps not done.
    pub fn verified_only(mut self) -> Self {
        self.verified = true;
        self
    }

    /// A role of `site.roles` by slug (`moderateur`, `admin`, `super-admin`,
    /// `mathematiques`…).
    pub fn role(mut self, slug: &str) -> Self {
        self.roles.push(slug.to_owned());
        self
    }

    /// A password hash (argon2id, test memory cost) for login tests.
    pub fn password(mut self, password: &str) -> Self {
        self.password = Some(password.to_owned());
        self
    }

    pub async fn create(self) -> TestMember {
        let pseudo = format!("m{}", unique());
        let email = format!("{pseudo}@example.invalid");
        let hash = match self.password {
            Some(p) => Some(pulsars_api::auth::password::hash(64, p).await.unwrap()),
            None => None,
        };
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO site.users (pseudo, name, email, password, email_verified_at, onboarding_at,
                                     locale, friend_token, created_at, updated_at)
             VALUES ($1, $1, $2, $3,
                     CASE WHEN $4 THEN now() AT TIME ZONE 'UTC' END,
                     CASE WHEN $5 THEN now() AT TIME ZONE 'UTC' END,
                     'fr', $6, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
             RETURNING id",
        )
        .bind(&pseudo)
        .bind(&email)
        .bind(hash)
        .bind(self.verified)
        .bind(self.onboarded)
        .bind(unique() + &unique())
        .fetch_one(&self.env.owner)
        .await
        .unwrap();
        for slug in &self.roles {
            let added = sqlx::query(
                "INSERT INTO site.role_user (role_id, user_id) SELECT id, $2 FROM site.roles WHERE slug = $1",
            )
            .bind(slug)
            .bind(id)
            .execute(&self.env.owner)
            .await
            .unwrap();
            assert_eq!(added.rows_affected(), 1, "no role `{slug}`");
        }
        let token = session_for(self.env, id).await;
        TestMember {
            id,
            pseudo,
            email,
            token,
        }
    }
}

/// A live session for account `user_id`, planted as the API stores them.
pub async fn session_for(env: &Env, user_id: i64) -> String {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).unwrap();
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    sqlx::query(
        "INSERT INTO site.auth_sessions (id, user_id, token_hash, remember, expires_at)
         VALUES ($1, $2, $3, false, now() + interval '2 hours')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(user_id)
    .bind(hash)
    .execute(&env.owner)
    .await
    .unwrap();
    token
}

/// One request to the app.
pub struct Call {
    method: &'static str,
    path: String,
    body: Option<Value>,
    multipart: Option<Vec<Part>>,
    user_agent: Option<String>,
    session: Option<String>,
    service: bool,
    origin: Option<String>,
    ip: Option<String>,
}

impl Call {
    pub fn new(method: &'static str, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            body: None,
            multipart: None,
            user_agent: None,
            session: None,
            service: false,
            origin: None,
            ip: None,
        }
    }
    pub fn get(path: impl Into<String>) -> Self {
        Self::new("GET", path)
    }
    pub fn post(path: impl Into<String>) -> Self {
        Self::new("POST", path)
    }
    pub fn put(path: impl Into<String>) -> Self {
        Self::new("PUT", path)
    }
    pub fn patch(path: impl Into<String>) -> Self {
        Self::new("PATCH", path)
    }
    pub fn delete(path: impl Into<String>) -> Self {
        Self::new("DELETE", path)
    }
    /// A JSON body.
    pub fn json(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
    /// A `multipart/form-data` body.
    pub fn multipart(mut self, parts: Vec<Part>) -> Self {
        self.multipart = Some(parts);
        self
    }
    /// Another browser than the tests' default one.
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_owned());
        self
    }
    /// As `member` (`Pulsars-Session`).
    pub fn session(mut self, token: &str) -> Self {
        self.session = Some(token.to_owned());
        self
    }
    pub fn member(self, member: &TestMember) -> Self {
        self.session(&member.token)
    }
    /// With the service key, as pulsars-web sends it.
    pub fn service(mut self) -> Self {
        self.service = true;
        self
    }
    pub fn origin(mut self, origin: &str) -> Self {
        self.origin = Some(origin.to_owned());
        self
    }
    pub fn ip(mut self, ip: &str) -> Self {
        self.ip = Some(ip.to_owned());
        self
    }

    pub async fn send(self, app: &Router) -> (StatusCode, Value) {
        let (status, _, body) = self.send_full(app).await;
        (status, body)
    }

    pub async fn send_full(self, app: &Router) -> (StatusCode, HeaderMap, Value) {
        let mut request = Request::builder()
            .method(self.method)
            .uri(&self.path)
            .header(
                header::USER_AGENT,
                self.user_agent.as_deref().unwrap_or("integration-tests"),
            )
            .header("x-forwarded-for", self.ip.unwrap_or_else(fresh_ip));
        if let Some(token) = &self.session {
            request = request.header("pulsars-session", token);
        }
        if self.service {
            request = request.header("pulsars-service-key", SERVICE_KEY);
        }
        if let Some(origin) = &self.origin {
            request = request.header(header::ORIGIN, origin);
        }
        let request = match (self.body, self.multipart) {
            (_, Some(parts)) => {
                let boundary = "pulsars-test-boundary-7f3a";
                let mut body = Vec::new();
                for part in parts {
                    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                    match &part.file_name {
                        Some(file) => body.extend_from_slice(
                            format!(
                                "Content-Disposition: form-data; name=\"{}\"; filename=\"{file}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                                part.name
                            )
                            .as_bytes(),
                        ),
                        None => body.extend_from_slice(
                            format!(
                                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                                part.name
                            )
                            .as_bytes(),
                        ),
                    }
                    body.extend_from_slice(&part.bytes);
                    body.extend_from_slice(b"\r\n");
                }
                body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
                request
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
            }
            (Some(body), None) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            (None, None) => request.body(Body::empty()),
        }
        .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

/// A part of a multipart body: a text field, or a file.
pub struct Part {
    pub name: String,
    pub file_name: Option<String>,
    pub bytes: Vec<u8>,
}

impl Part {
    pub fn text(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            file_name: None,
            bytes: value.as_bytes().to_vec(),
        }
    }
    pub fn file(name: &str, file_name: &str, bytes: &[u8]) -> Self {
        Self {
            name: name.to_owned(),
            file_name: Some(file_name.to_owned()),
            bytes: bytes.to_vec(),
        }
    }
}

/// A PNG header of `width` x `height`: enough for the API's checks.
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

/// The `code`s of a 422's `errors`, as (parameter, code) pairs.
pub fn codes(body: &Value) -> Vec<(String, String)> {
    body["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            (
                e["parameter"].as_str().unwrap_or_default().to_owned(),
                e["code"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}
