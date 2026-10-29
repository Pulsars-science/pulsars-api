//! The languages section (`GET /v1/languages…`), against the starting
//! content of the `langues` migration (Mandarin, HSK 1 and HSK 2).
//!
//! Needs `TEST_DATABASE_URL` and `TEST_REDIS_URL` (skipped without them,
//! unless `REQUIRE_INTEGRATION=1`).
//!
//! The statistics are global: the test that posts events compares the
//! numbers before and after, with a learner id of its own, and deletes its
//! rows at the end.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use pulsars_api::auth::token::testing;
use pulsars_api::config::{Config, TokenSigner};
use pulsars_api::game::{Game, GameSettings, store};
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes};
use serde_json::{Value, json};
use tower::ServiceExt as _;

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

const KID: &str = "2026-10-ed25519-1";
const ORIGIN: &str = "http://localhost:5174";

/// The API's signing key, as the test application holds it.
struct Keys {
    signer: TokenSigner,
}

impl Keys {
    fn token(&self, roles: &[&str]) -> String {
        let mut claims = testing::claims("1", Utc::now().timestamp());
        claims["roles"] = json!(roles);
        testing::sign(
            &self.signer.key,
            &json!({"alg": "EdDSA", "typ": "JWT", "kid": KID}),
            &claims,
        )
    }
}

/// The application on a migrated database, or `None` when skipped.
async fn languages_app() -> Option<Router> {
    Some(languages_app_with_keys().await?.0)
}

/// The same, with its signing key, and the pool.
async fn languages_app_with_keys() -> Option<(Router, Keys, sqlx::PgPool)> {
    let (database_url, redis_url) = integration_urls()?;
    let owner_url = std::env::var("TEST_MIGRATION_DATABASE_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| database_url.clone());
    let owner = sqlx::PgPool::connect(&owner_url).await.unwrap();
    store::migrate(&owner).await.expect("migrations apply");
    owner.close().await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let keys = Keys {
        signer: testing::signer(),
    };

    let mut config = Config::from_vars([
        ("PULSARS_ENV".to_owned(), "development".to_owned()),
        ("DATABASE_URL".to_owned(), database_url),
        ("REDIS_URL".to_owned(), redis_url),
        ("CORS_ALLOWED_ORIGINS".to_owned(), ORIGIN.to_owned()),
        (
            "RATE_LIMIT_ANONYMOUS_PER_MINUTE".to_owned(),
            "1000000".to_owned(),
        ),
    ])
    .expect("valid test config");
    config.token_signer = Some(keys.signer.clone());
    let db = db::connect_lazy(&config).unwrap();
    let state = AppState {
        http_daily: Default::default(),
        mailer: pulsars_api::mail::Mailer::recording(),
        game: Game::start(db.clone(), GameSettings::default()),
        db,
        redis: redis::connect(&config).unwrap(),
        config: Arc::new(config),
    };
    Some((routes::router(state), keys, pool))
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Value) {
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

fn post_event(body: &Value) -> Request<Body> {
    Request::post("/v1/languages/events")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, ORIGIN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn stats(token: Option<&str>) -> Request<Body> {
    let mut request = Request::get("/v1/languages/admin/stats?days=7");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::empty()).unwrap()
}

async fn get(app: &Router, path: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let request = Request::get(path).body(Body::empty()).unwrap();
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

#[tokio::test]
async fn languages_list_mandarin() {
    let Some(app) = languages_app().await else {
        return;
    };
    let (status, headers, body) = get(&app, "/v1/languages").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_LANGUAGE], "fr");
    assert!(headers.contains_key(header::ETAG));
    let zh = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["code"] == "zh")
        .expect("Mandarin is published");
    assert_eq!(
        zh,
        &json!({
            "code": "zh",
            "name": "Mandarin",
            "lang": "fr",
            "native_name": "中文",
            "script": "Hans",
            "reading_system": "pinyin",
            "countries": ["CN", "TW", "SG"],
            "level_count": 2,
            "word_count": 38,
        })
    );

    let (_, headers, body) = get(&app, "/v1/languages?lang=en").await;
    assert_eq!(headers[header::CONTENT_LANGUAGE], "en");
    let zh = &body["data"][0];
    assert_eq!(zh["name"], "Mandarin Chinese");
    assert_eq!(zh["lang"], "en");
}

#[tokio::test]
async fn a_language_lists_its_levels_in_order() {
    let Some(app) = languages_app().await else {
        return;
    };
    let (status, _, body) = get(&app, "/v1/languages/zh").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["code"], "zh");
    let levels = data["levels"].as_array().unwrap();
    let slugs: Vec<&str> = levels.iter().map(|l| l["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, ["hsk-1", "hsk-2"]);
    assert_eq!(levels[0]["framework"], "HSK");
    assert_eq!(levels[0]["word_count"], 38);
    assert_eq!(levels[1]["word_count"], 0);
}

#[tokio::test]
async fn a_level_serves_its_words_and_neighbours() {
    let Some(app) = languages_app().await else {
        return;
    };
    let (status, _, body) = get(&app, "/v1/languages/zh/levels/hsk-1?lang=en").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["language"]["name"], "Mandarin Chinese");
    assert_eq!(data["language"]["script"], "Hans");
    assert_eq!(data["title"], "HSK 1");
    assert_eq!(data["previous"], Value::Null);
    assert_eq!(data["next"]["slug"], "hsk-2");
    let words = data["words"].as_array().unwrap();
    assert_eq!(words.len(), 38);
    assert_eq!(words[0]["text"], "我");
    assert_eq!(words[0]["reading"], "wǒ");
    assert_eq!(words[0]["meaning"], "I, me");
    assert_eq!(words[0]["lang"], "en");

    let (_, _, body) = get(&app, "/v1/languages/zh/levels/hsk-2").await;
    assert_eq!(body["data"]["previous"]["slug"], "hsk-1");
    assert_eq!(body["data"]["next"], Value::Null);
    assert_eq!(body["data"]["words"], json!([]));
}

#[tokio::test]
async fn unknown_languages_and_levels_are_404() {
    let Some(app) = languages_app().await else {
        return;
    };
    for path in [
        "/v1/languages/xx",
        "/v1/languages/xx/levels/hsk-1",
        "/v1/languages/zh/levels/hsk-9",
    ] {
        let (status, headers, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=60");
        assert_eq!(body["instance"], path);
    }
    let (status, _, _) = get(&app, "/v1/languages?lang=de").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn events_feed_the_admin_statistics() {
    let Some((app, keys, pool)) = languages_app_with_keys().await else {
        return;
    };
    let learner = uuid::Uuid::new_v4();
    let admin = keys.token(&["admin"]);

    // Only admins read the statistics.
    let (status, _, _) = send(&app, stats(None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, body) = send(&app, stats(Some(&keys.token(&["membre"])))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["reason"], "insufficient_scope");

    let (status, headers, before) = send(&app, stats(Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(before["days"], 7);
    assert_eq!(before["daily"].as_array().unwrap().len(), 7);

    let (_, _, level) = get(&app, "/v1/languages/zh/levels/hsk-1").await;
    let wo = level["data"]["words"][0]["id"].as_i64().unwrap();

    // A lesson, with one real missed word and one that is not in the level.
    let (status, headers, _) = send(
        &app,
        post_event(&json!({
            "kind": "lesson.completed", "learner": learner.to_string(), "language": "zh", "level": "hsk-1",
            "score": 70, "correct": 4, "questions": 5, "duration_ms": 42000,
            "missed_words": [wo, 999_999_999],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
    let (status, _, _) = send(
        &app,
        post_event(&json!({
            "kind": "writing.completed", "learner": learner.to_string(), "language": "zh", "level": "hsk-1",
            "character": "我", "mode": "memory", "mistakes": 0,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let missed: Vec<i64> = sqlx::query_scalar(
        "SELECT unnest(missed_words) FROM langues.events WHERE learner = $1 AND kind = 'lesson.completed'",
    )
    .bind(learner)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(missed, [wo], "ids outside the level are dropped");

    let (_, _, after) = send(&app, stats(Some(&admin))).await;
    let delta = |path: &[&str]| {
        let pick = |v: &Value| {
            path.iter()
                .fold(v.clone(), |v, k| v[*k].clone())
                .as_i64()
                .unwrap()
        };
        pick(&after) - pick(&before)
    };
    assert_eq!(delta(&["totals", "learners"]), 1);
    assert_eq!(delta(&["totals", "lessons"]), 1);
    assert_eq!(delta(&["totals", "questions"]), 5);
    assert_eq!(delta(&["totals", "writings"]), 1);
    assert_eq!(delta(&["totals", "from_memory"]), 1);
    assert_eq!(delta(&["all_time", "lessons"]), 1);
    let today = after["daily"].as_array().unwrap().last().unwrap().clone();
    assert!(today["lessons"].as_i64().unwrap() >= 1);
    let hsk1 = after["levels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["level"] == "hsk-1")
        .unwrap()
        .clone();
    assert_eq!(hsk1["title"], "HSK 1");
    assert!(hsk1["best_score"].as_i64().unwrap() >= 70);
    assert!(
        after["missed_words"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["id"] == wo)
    );

    sqlx::query("DELETE FROM langues.events WHERE learner = $1")
        .bind(learner)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn bad_events_are_refused() {
    let Some((app, _, _)) = languages_app_with_keys().await else {
        return;
    };
    let (status, _, body) = send(
        &app,
        post_event(&json!({"kind": "lesson.completed", "learner": "x", "language": "zh", "level": "hsk-1",
                           "score": -1, "correct": 6, "questions": 5, "duration_ms": 1, "missed_words": []})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["type"],
        "https://api.pulsars.science/problems/invalid-body"
    );
    let fields: Vec<&str> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["parameter"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["learner", "score", "correct"]);

    let (status, _, body) = send(&app, post_event(&json!("pas un objet"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, _, body) = send(
        &app,
        post_event(&json!({"kind": "writing.completed", "learner": uuid::Uuid::new_v4().to_string(),
                           "language": "zh", "level": "hsk-9", "character": "我", "mode": "memory", "mistakes": 0})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
