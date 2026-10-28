//! `GET /v1/game/admin/stats` end to end: the role check, the live counters
//! of a running match and the daily rows of finished ones.
//!
//! Like `tests/game.rs`, the database test reads `TEST_DATABASE_URL` and
//! `TEST_REDIS_URL` and is skipped without them (unless
//! `REQUIRE_INTEGRATION=1`). The statistics are global, so this binary has a
//! single database test that compares the numbers before and after its own
//! matches (cargo runs test binaries one after the other). It deletes the
//! rows it writes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::{Days, NaiveTime, Utc};
use ed25519_dalek::SigningKey;
use futures_util::{SinkExt as _, StreamExt as _};
use pulsars_api::auth::token::testing;
use pulsars_api::auth::{Jwks, JwksSettings};
use pulsars_api::config::Config;
use pulsars_api::flash;
use pulsars_api::game::rules::SubjectKey;
use pulsars_api::game::{Game, GameSettings, store};
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tower::ServiceExt as _;
use uuid::Uuid;

const DEAD_POSTGRES: &str = "postgres://pulsars_api:x@127.0.0.1:1/pulsars";
const DEAD_REDIS: &str = "redis://127.0.0.1:1";
const ORIGIN: &str = "http://localhost:5173";
const KID: &str = "2026-10-ed25519-1";
const STATS: &str = "/v1/game/admin/stats";

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

fn fast_settings() -> GameSettings {
    let mut settings = GameSettings {
        countdown: Duration::from_millis(100),
        auth_timeout: Duration::from_millis(500),
        queue_tick: Duration::from_millis(100),
        ..GameSettings::default()
    };
    // A slow correct answer is anything over 50 ms here.
    settings.engine.suspicious_ms = 50;
    settings
}

/// Laravel's JWKS endpoint, in-process.
struct Keys {
    url: String,
    key: SigningKey,
}

impl Keys {
    async fn start() -> Self {
        let key = testing::signing_key();
        let document = json!({"keys": [testing::jwk(&key, KID)]}).to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/.well-known/jwks.json",
            listener.local_addr().unwrap()
        );
        let router = Router::new().route(
            "/.well-known/jwks.json",
            axum::routing::get(move || {
                let document = document.clone();
                async move {
                    (
                        [(header::CONTENT_TYPE, "application/jwk-set+json")],
                        document,
                    )
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { url, key }
    }

    fn token(&self, sub: &str, name: &str, roles: &[&str]) -> String {
        let mut claims = testing::claims(sub, Utc::now().timestamp());
        claims["name"] = json!(name);
        claims["roles"] = json!(roles);
        testing::sign(
            &self.key,
            &json!({"alg": "EdDSA", "typ": "JWT", "kid": KID}),
            &claims,
        )
    }
}

struct Server {
    addr: SocketAddr,
    app: Router,
    state: AppState,
    keys: Keys,
}

async fn server(database_url: &str, redis_url: &str) -> Server {
    let keys = Keys::start().await;
    let config = Config::from_vars([
        ("PULSARS_ENV", "development"),
        ("DATABASE_URL", database_url),
        ("REDIS_URL", redis_url),
        ("JWKS_URL", keys.url.as_str()),
        ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1000000"),
        ("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "1000000"),
    ])
    .expect("valid test config");
    let jwks = Jwks::with_settings(
        &config.jwks_url,
        false,
        JwksSettings {
            fetch_timeout: Duration::from_secs(2),
            ..JwksSettings::default()
        },
    )
    .unwrap();
    jwks.refresh().await.expect("JWKS loads");
    let db = db::connect_lazy(&config).unwrap();
    let state = AppState {
        game: Game::start(db.clone(), fast_settings()),
        db,
        redis: redis::connect(&config).unwrap(),
        config: Arc::new(config),
        jwks,
    };
    let app = routes::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = app
        .clone()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, service).await });
    Server {
        addr,
        app,
        state,
        keys,
    }
}

/// `GET path` with an optional bearer token and `Origin`: status, headers
/// and JSON body.
async fn get(
    server: &Server,
    path: &str,
    token: Option<&str>,
    origin: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::get(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let response = server
        .app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, body)
}

/// The statistics, as an admin, server to server (no `Origin`).
async fn stats(server: &Server, days: u32) -> Value {
    let token = server.keys.token("1", "jikuji", &["admin"]);
    let (status, _, body) = get(server, &format!("{STATS}?days={days}"), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Polls the statistics until `done` holds (5 s at most).
async fn stats_until(server: &Server, days: u32, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let body = stats(server, days).await;
        if done(&body) {
            return body;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "statistics never matched: {body}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn fresh_user() -> i64 {
    900_000_000 + (Uuid::new_v4().as_u128() % 99_000_000) as i64
}

fn n(value: &Value) -> i64 {
    value
        .as_i64()
        .unwrap_or_else(|| panic!("not an integer: {value}"))
}

/// The `daily` entry of `date`.
fn day<'a>(stats: &'a Value, date: &str) -> &'a Value {
    stats["daily"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["date"] == date)
        .unwrap_or_else(|| panic!("no daily row for {date}: {stats}"))
}

fn subject_count(stats: &Value, subject: &str) -> i64 {
    let row = stats["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["subject"] == subject)
        .unwrap();
    n(&row["matches"])
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Client {
    ws: Socket,
}

impl Client {
    async fn open(server: &Server) -> Client {
        let mut request = format!("ws://{}/v1/game/ws", server.addr)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert(header::ORIGIN, ORIGIN.parse().unwrap());
        let (ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        Client { ws }
    }

    async fn account(server: &Server, user_id: i64, name: &str) -> Client {
        let mut client = Client::open(server).await;
        let token = server.keys.token(&user_id.to_string(), name, &[]);
        client.send(json!({"type": "auth", "token": token})).await;
        client.until("auth.ok").await;
        client
    }

    async fn guest(server: &Server) -> Client {
        let mut client = Client::open(server).await;
        client.send(json!({"type": "auth", "guest": true})).await;
        client.until("auth.ok").await;
        client
    }

    async fn send(&mut self, value: Value) {
        self.ws
            .send(Message::text(value.to_string()))
            .await
            .unwrap();
    }

    async fn until(&mut self, kind: &str) -> Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(15), self.ws.next())
                .await
                .expect("a frame within 15 s")
                .expect("the socket is open")
                .expect("a valid frame");
            let frame: Value = match message {
                Message::Text(text) => serde_json::from_str(&text).unwrap(),
                Message::Close(frame) => panic!("closed: {frame:?}"),
                _ => continue,
            };
            if frame["type"] == kind {
                return frame;
            }
            assert_ne!(frame["type"], "error", "unexpected error: {frame}");
        }
    }
}

#[tokio::test]
async fn admin_stats_need_a_token_with_an_admin_role() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS).await;

    let (status, headers, body) = get(&server, STATS, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["reason"], "missing");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");

    for roles in [&[][..], &["moderateur"][..]] {
        let token = server.keys.token("42", "member", roles);
        let (status, headers, body) = get(&server, STATS, Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{roles:?}");
        assert_eq!(
            headers[header::CONTENT_TYPE],
            "application/problem+json",
            "{roles:?}"
        );
        assert_eq!(
            headers[header::WWW_AUTHENTICATE],
            "Bearer realm=\"api.pulsars.science\", error=\"insufficient_scope\""
        );
        assert_eq!(
            body,
            json!({
                "type": "https://api.pulsars.science/problems/insufficient-scope",
                "title": "Insufficient scope",
                "status": 403,
                "detail": "This endpoint needs the admin or super-admin role.",
                "instance": STATS,
                "reason": "insufficient_scope",
            })
        );
    }

    // `days` is checked once the role is.
    let token = server.keys.token("1", "jikuji", &["super-admin"]);
    for bad in ["0", "91", "x"] {
        let (status, _, body) =
            get(&server, &format!("{STATS}?days={bad}"), Some(&token), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(body["errors"][0]["parameter"], "days");
    }

    // Restricted CORS, like `/v1/game/me`.
    let (_, headers, _) = get(&server, STATS, None, Some("https://evil.example")).await;
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    let (_, headers, _) = get(&server, STATS, None, Some(ORIGIN)).await;
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
}

/// A seat: `(user_id, guest_name, ghost_of, answered, correct)`.
type Seat<'a> = (Option<i64>, Option<&'a str>, Option<Uuid>, i32, i32);

/// Inserts a finished match started at `started_at`.
async fn finished_match(
    pool: &sqlx::PgPool,
    mode: &str,
    ranked: bool,
    end_reason: &str,
    started_at: chrono::DateTime<Utc>,
    seconds: i64,
    seats: &[Seat<'_>],
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO game.matches
                (id, mode, ranked, subject, rules, seed, created_at, started_at, ended_at, end_reason)
         VALUES ($1, $2, $3, 'physics', '{\"duration\": 60, \"target\": null, \"penalty\": false}',
                 1, $4, $4, $5, $6)",
    )
    .bind(id)
    .bind(mode)
    .bind(ranked)
    .bind(started_at)
    .bind(started_at + chrono::Duration::seconds(seconds))
    .bind(end_reason)
    .execute(pool)
    .await
    .unwrap();
    for (seat, (user_id, guest, ghost_of, answered, correct)) in seats.iter().enumerate() {
        sqlx::query(
            "INSERT INTO game.match_players
                    (match_id, seat, user_id, guest_name, ghost_of, name, score, correct,
                     answered, answer_time_ms, rank, outcome)
             VALUES ($1, $2, $3, $4, $5, 'x', $6, $6, $7, 0, 1, 'draw')",
        )
        .bind(id)
        .bind(seat as i16)
        .bind(user_id)
        .bind(guest)
        .bind(ghost_of)
        .bind(correct)
        .bind(answered)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

#[tokio::test]
async fn admin_stats_count_live_and_finished_matches() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    apply_migrations(&database_url).await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let server = server(&database_url, &redis_url).await;
    {
        use fred::prelude::ClientLike as _;
        tokio::time::timeout(
            Duration::from_secs(5),
            server.state.redis.wait_for_connect(),
        )
        .await
        .expect("redis connects within 5 s")
        .expect("redis connects");
    }

    let today = Utc::now().date_naive();
    let two_days_ago = today.checked_sub_days(Days::new(2)).unwrap();
    let today_key = today.format("%Y-%m-%d").to_string();
    let past_key = two_days_ago.format("%Y-%m-%d").to_string();

    // --- Shape, before anything ---------------------------------------
    let default = stats(&server, 30).await;
    let token = server.keys.token("1", "jikuji", &["super-admin"]);
    let (status, headers, by_default) = get(&server, STATS, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(by_default["daily"].as_array().unwrap().len(), 30);
    let keys: Vec<&str> = default
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for key in [
        "generated_at",
        "live",
        "today",
        "daily",
        "subjects",
        "rules",
        "quality",
        "ratings",
        "webhook",
        "seasons",
    ] {
        assert!(keys.contains(&key), "{key} missing: {default}");
    }
    assert_eq!(keys.len(), 10, "{keys:?}");
    assert_eq!(
        default["live"],
        json!({"sessions": 0, "guests": 0, "queued": 0, "rooms_open": 0,
               "matches": {"duel": 0, "room": 0, "ghost": 0}, "players_in_matches": 0})
    );
    assert!(default["generated_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(default["today"]["by_hour"].as_array().unwrap().len(), 24);
    let daily = default["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 30);
    assert_eq!(daily.last().unwrap()["date"], today_key.as_str());
    let first = today.checked_sub_days(Days::new(29)).unwrap();
    assert_eq!(daily[0]["date"], first.format("%Y-%m-%d").to_string());
    for entry in daily {
        let fields: Vec<&String> = entry.as_object().unwrap().keys().collect();
        assert_eq!(fields.len(), 9, "{entry}");
    }
    let subjects: Vec<&str> = default["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects.len(), 5);
    for s in ["maths", "physics", "chemistry", "computer-science", "all"] {
        assert!(subjects.contains(&s), "{subjects:?}");
    }
    let counts: Vec<i64> = default["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| n(&s["matches"]))
        .collect();
    assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{counts:?}");
    assert_eq!(
        default["ratings"]["by_subject"].as_array().unwrap().len(),
        5
    );
    for field in ["pending", "dead"] {
        assert!(default["webhook"][field].is_i64());
    }
    assert!(default["seasons"].as_array().unwrap().len() <= 12);

    let before = stats(&server, 3).await;
    assert_eq!(before["daily"].as_array().unwrap().len(), 3);
    assert_eq!(before["daily"][0]["date"], past_key.as_str());

    // --- Finished matches, two days ago --------------------------------
    let (u1, u2, u3) = (fresh_user(), fresh_user(), fresh_user());
    let ten = two_days_ago
        .and_time(NaiveTime::from_hms_opt(10, 0, 0).unwrap())
        .and_utc();
    let duel = finished_match(
        &pool,
        "duel",
        true,
        "completed",
        ten,
        60,
        &[(Some(u1), None, None, 10, 8), (Some(u2), None, None, 4, 2)],
    )
    .await;
    let room = finished_match(
        &pool,
        "room",
        false,
        "void",
        ten,
        30,
        &[
            (Some(u1), None, None, 0, 0),
            (None, Some("Invité 1234"), None, 0, 0),
        ],
    )
    .await;
    let ghost = finished_match(
        &pool,
        "ghost",
        false,
        "forfeit",
        ten,
        20,
        &[(Some(u3), None, None, 6, 3), (None, None, Some(duel), 9, 9)],
    )
    .await;

    let after_past = stats(&server, 3).await;
    let (old, new) = (day(&before, &past_key), day(&after_past, &past_key));
    for (field, delta) in [
        ("duel", 1),
        ("room", 1),
        ("ghost", 1),
        ("ranked", 1),
        ("void", 1),
        ("forfeit", 1),
        ("accounts", 3),
        ("guests", 1),
    ] {
        assert_eq!(
            n(&new[field]) - n(&old[field]),
            delta,
            "{field}: {old} → {new}"
        );
    }
    assert_eq!(
        subject_count(&after_past, "physics") - subject_count(&before, "physics"),
        3
    );
    let quality = &after_past["quality"];
    assert!(
        quality["avg_match_seconds"].as_f64().unwrap() > 0.0,
        "{quality}"
    );
    assert!(quality["avg_answers_per_player"].as_f64().unwrap() > 0.0);
    let accuracy = quality["accuracy"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&accuracy), "{quality}");
    let rules = after_past["rules"].as_array().unwrap();
    let sixty = rules
        .iter()
        .find(|r| r["rules"] == json!({"duration": 60, "target": null, "penalty": false}))
        .unwrap_or_else(|| panic!("no 60 s rules: {rules:?}"));
    assert!(n(&sixty["matches"]) >= 3);
    let counts: Vec<i64> = rules.iter().map(|r| n(&r["matches"])).collect();
    assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{counts:?}");

    // --- A live ranked duel, a room in its lobby, a guest in a queue ---
    let (ida, idb) = (fresh_user(), fresh_user());
    let mut a = Client::account(&server, ida, "alice").await;
    let mut b = Client::account(&server, idb, "bob").await;
    let rules = json!({"duration": null, "target": 10, "penalty": false});
    let join = json!({"type": "queue.join", "subject": "computer-science", "rules": rules});
    a.send(join.clone()).await;
    a.until("queue.joined").await;
    b.send(join).await;
    let start = a.until("match.start").await;
    b.until("match.start").await;
    let match_id = Uuid::parse_str(start["match_id"].as_str().unwrap()).unwrap();

    let mut host = Client::guest(&server).await;
    host.send(json!({"type": "room.create", "subject": "maths", "rules": rules}))
        .await;
    host.until("room.state").await;
    let mut waiting = Client::guest(&server).await;
    waiting
        .send(json!({"type": "queue.join", "subject": "physics", "rules": rules}))
        .await;
    waiting.until("queue.joined").await;

    // The start row is written by the runner, just after `match.start`.
    let during = stats_until(&server, 3, |s| {
        n(&s["today"]["matches_started"]) > n(&before["today"]["matches_started"])
    })
    .await;
    assert_eq!(
        during["live"],
        json!({"sessions": 4, "guests": 2, "queued": 1, "rooms_open": 1,
               "matches": {"duel": 1, "room": 0, "ghost": 0}, "players_in_matches": 2}),
        "{during}"
    );
    let (old, now) = (day(&before, &today_key), day(&during, &today_key));
    assert_eq!(n(&now["duel"]) - n(&old["duel"]), 1);
    assert_eq!(n(&now["ranked"]) - n(&old["ranked"]), 1);
    // Its seats are not in the database yet: they come from the hub.
    assert_eq!(
        n(&now["accounts"]) - n(&old["accounts"]),
        2,
        "{old} → {now}"
    );
    assert_eq!(during["today"]["accounts"], now["accounts"]);
    assert_eq!(during["today"]["guests"], now["guests"]);
    assert_eq!(
        n(&during["today"]["matches_started"]) - n(&before["today"]["matches_started"]),
        1
    );
    let by_hour: i64 = during["today"]["by_hour"]
        .as_array()
        .unwrap()
        .iter()
        .map(n)
        .sum();
    assert_eq!(by_hour, n(&during["today"]["matches_started"]));
    assert_eq!(
        n(&during["today"]["matches_started"]),
        n(&now["duel"]) + n(&now["room"]) + n(&now["ghost"])
    );

    // --- `a` wins, slowly enough to stay ranked ------------------------
    let seed = server.state.game.match_seed(match_id).await.unwrap();
    let correct: Vec<usize> =
        flash::duel_questions(SubjectKey::parse("computer-science").unwrap().0, seed, 240)
            .iter()
            .map(|q| q.correct)
            .collect();
    for _ in 0..10 {
        let q = a.until("question").await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        let index = q["index"].as_u64().unwrap() as usize;
        a.send(
            json!({"type": "answer", "match_id": q["match_id"], "index": index,
                      "choice": correct[index]}),
        )
        .await;
        a.until("answer.result").await;
    }
    let end = a.until("match.end").await;
    assert_eq!(end["ranked"], true, "{end}");
    b.until("match.end").await;

    let after = stats_until(&server, 3, |s| s["live"]["matches"]["duel"] == 0).await;
    assert_eq!(after["live"]["players_in_matches"], 0);
    assert_eq!(after["live"]["sessions"], 4);
    let (old, done) = (day(&before, &today_key), day(&after, &today_key));
    for (field, delta) in [
        ("duel", 1),
        ("ranked", 1),
        ("void", 0),
        ("forfeit", 0),
        ("accounts", 2),
        ("guests", 0),
    ] {
        assert_eq!(
            n(&done[field]) - n(&old[field]),
            delta,
            "{field}: {old} → {done}"
        );
    }
    assert_eq!(
        subject_count(&after, "computer-science") - subject_count(&before, "computer-science"),
        1
    );
    // The two days without our rows did not move.
    let yesterday = today.checked_sub_days(Days::new(1)).unwrap();
    let key = yesterday.format("%Y-%m-%d").to_string();
    assert_eq!(day(&before, &key), day(&after, &key));
    assert!(n(&after["ratings"]["players"]) >= 2);
    let cs = after["ratings"]["by_subject"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["subject"] == "computer-science")
        .unwrap();
    assert!(n(&cs["players"]) >= 2);

    // Cleanup.
    sqlx::query("DELETE FROM game.webhook_outbox WHERE body->'data'->>'match_id' = $1")
        .bind(match_id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game.matches WHERE id = ANY($1)")
        .bind([match_id, ghost, room, duel].as_slice())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game.ratings WHERE user_id = ANY($1)")
        .bind([ida, idb].as_slice())
        .execute(&pool)
        .await
        .unwrap();
}
