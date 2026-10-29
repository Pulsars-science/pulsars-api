//! End-to-end tests of the game server: the WebSocket protocol over a real
//! listener and the `/v1/game` HTTP endpoints.
//!
//! Like `tests/http.rs`, tests that need PostgreSQL and Redis read
//! `TEST_DATABASE_URL` and `TEST_REDIS_URL` and are skipped without them
//! (unless `REQUIRE_INTEGRATION=1`). They clean up the rows they write.
//! Matches use a 100 ms countdown and target mode, so nothing waits for a
//! 60-second clock.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use futures_util::{SinkExt as _, StreamExt as _};
use pulsars_api::auth::token::testing;
use pulsars_api::config::{Config, TokenSigner};
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

const DEAD_POSTGRES: &str = "postgres://pulsars_api:x@127.0.0.1:1/pulsars";
const DEAD_REDIS: &str = "redis://127.0.0.1:1";
const ORIGIN: &str = "http://localhost:5173";
const KID: &str = "2026-10-ed25519-1";

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

/// Short timings: the countdown, the auth delay and the queue tick.
fn fast_settings() -> GameSettings {
    GameSettings {
        countdown: Duration::from_millis(100),
        auth_timeout: Duration::from_millis(500),
        queue_tick: Duration::from_millis(100),
        waiting_every: Duration::from_millis(300),
        ghost_offer_after: Duration::from_millis(600),
        ..GameSettings::default()
    }
}

/// The API's signing key, as the test application holds it.
struct Keys {
    signer: TokenSigner,
}

impl Keys {
    /// A token for `sub`, named `name`, in `locale`.
    fn token(&self, sub: &str, name: &str, locale: &str) -> String {
        let mut claims = testing::claims(sub, chrono::Utc::now().timestamp());
        claims["name"] = json!(name);
        claims["locale"] = json!(locale);
        testing::sign(
            &self.signer.key,
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

async fn server(database_url: &str, redis_url: &str, settings: GameSettings) -> Server {
    server_with(database_url, redis_url, settings, &[]).await
}

async fn server_with(
    database_url: &str,
    redis_url: &str,
    settings: GameSettings,
    extra: &[(&str, &str)],
) -> Server {
    let mut config = Config::from_vars(
        [
            ("PULSARS_ENV", "development"),
            ("DATABASE_URL", database_url),
            ("REDIS_URL", redis_url),
            ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "1000000"),
            ("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "1000000"),
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .expect("valid test config");
    // A key from `extra` (`JWT_SIGNING_KEY_FILE`), else a fresh one.
    let keys = Keys {
        signer: config
            .token_signer
            .get_or_insert_with(testing::signer)
            .clone(),
    };
    let db = db::connect_lazy(&config).unwrap();
    let state = AppState {
        http_daily: Default::default(),
        mailer: pulsars_api::mail::Mailer::recording(),
        game: Game::start(db.clone(), settings),
        db,
        redis: redis::connect(&config).unwrap(),
        config: Arc::new(config),
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

async fn migrated(database_url: &str) -> sqlx::PgPool {
    apply_migrations(database_url).await;
    sqlx::PgPool::connect(database_url).await.unwrap()
}

async fn wait_for_redis(state: &AppState) {
    use fred::prelude::ClientLike as _;
    tokio::time::timeout(Duration::from_secs(5), state.redis.wait_for_connect())
        .await
        .expect("redis connects within 5 s")
        .expect("redis connects");
}

/// An account id no other test (or earlier run) uses.
fn fresh_user() -> i64 {
    900_000_000 + (uuid::Uuid::new_v4().as_u128() % 99_000_000) as i64
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Client {
    ws: Socket,
}

impl Client {
    async fn connect(server: &Server, origin: Option<&str>) -> Result<Client, StatusCode> {
        let mut request = format!("ws://{}/v1/game/ws", server.addr)
            .into_client_request()
            .unwrap();
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert(header::ORIGIN, origin.parse().unwrap());
        }
        match tokio_tungstenite::connect_async(request).await {
            Ok((ws, _)) => Ok(Client { ws }),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => Err(response.status()),
            Err(error) => panic!("connection failed: {error}"),
        }
    }

    async fn account(server: &Server, sub: &str, name: &str) -> (Client, Value) {
        let mut client = Client::connect(server, Some(ORIGIN)).await.unwrap();
        let token = server.keys.token(sub, name, "fr");
        client.send(json!({"type": "auth", "token": token})).await;
        let ok = client.until("auth.ok").await;
        (client, ok)
    }

    async fn guest(server: &Server, locale: &str, resume: Option<&str>) -> (Client, Value) {
        let mut client = Client::connect(server, Some(ORIGIN)).await.unwrap();
        client
            .send(json!({"type": "auth", "guest": true, "locale": locale, "resume": resume}))
            .await;
        let ok = client.until("auth.ok").await;
        (client, ok)
    }

    async fn send(&mut self, value: Value) {
        self.ws
            .send(Message::text(value.to_string()))
            .await
            .unwrap();
    }

    /// The next JSON frame.
    async fn recv(&mut self) -> Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(15), self.ws.next())
                .await
                .expect("a frame within 15 s")
                .expect("the socket is open")
                .expect("a valid frame");
            match message {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                Message::Close(frame) => panic!("closed: {frame:?}"),
                _ => continue,
            }
        }
    }

    /// Skips frames until one of type `kind`.
    async fn until(&mut self, kind: &str) -> Value {
        loop {
            let frame = self.recv().await;
            if frame["type"] == kind {
                return frame;
            }
            assert_ne!(frame["type"], "error", "unexpected error: {frame}");
        }
    }

    /// The close code and reason, after skipping other frames.
    async fn closed(&mut self) -> (u16, String) {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(15), self.ws.next())
                .await
                .expect("a close within 15 s");
            match message {
                Some(Ok(Message::Close(Some(frame)))) => {
                    return (u16::from(frame.code), frame.reason.to_string());
                }
                Some(Ok(Message::Close(None))) | None | Some(Err(_)) => return (0, String::new()),
                Some(Ok(_)) => continue,
            }
        }
    }
}

/// The right choices of a match, from its seed (the test peeks at it).
async fn answers(server: &Server, start: &Value) -> Vec<usize> {
    let match_id = uuid::Uuid::parse_str(start["match_id"].as_str().unwrap()).unwrap();
    let seed = server
        .state
        .game
        .match_seed(match_id)
        .await
        .expect("live match");
    let subject = SubjectKey::parse(start["subject"].as_str().unwrap()).unwrap();
    flash::duel_questions(subject.0, seed, 240)
        .iter()
        .map(|q| q.correct)
        .collect()
}

/// Answers the question in `frame` (right or wrong), returns the result.
async fn answer(client: &mut Client, question: &Value, correct: &[usize], right: bool) -> Value {
    let index = question["index"].as_u64().unwrap() as usize;
    let good = correct[index];
    let choice = if right { good } else { (good + 1) % 4 };
    client
        .send(json!({"type": "answer", "match_id": question["match_id"], "index": index, "choice": choice}))
        .await;
    let result = client.until("answer.result").await;
    assert_eq!(result["index"], index);
    assert_eq!(result["correct"], right);
    assert_eq!(result["correct_choice"], good);
    result
}

fn target_rules() -> Value {
    json!({"duration": null, "target": 10, "penalty": true})
}

// --- Without services ------------------------------------------------------

#[tokio::test]
async fn the_websocket_refuses_other_origins_before_the_upgrade() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    assert_eq!(
        Client::connect(&server, Some("https://evil.example"))
            .await
            .err(),
        Some(StatusCode::FORBIDDEN)
    );
    assert_eq!(
        Client::connect(&server, None).await.err(),
        Some(StatusCode::FORBIDDEN)
    );

    let plain = Request::get("/v1/game/ws")
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    let response = server.app.clone().oneshot(plain).await.unwrap();
    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
}

#[tokio::test]
async fn sessions_must_authenticate_first_and_in_time() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;

    let mut silent = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    assert_eq!(silent.closed().await.0, 4000);

    let mut early = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    early.send(json!({"type": "ping"})).await;
    assert_eq!(early.closed().await.0, 4000);

    let mut forged = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    let token = testing::token(
        &testing::signing_key(),
        KID,
        "5",
        chrono::Utc::now().timestamp(),
    );
    forged.send(json!({"type": "auth", "token": token})).await;
    assert_eq!(forged.closed().await, (4001, "bad_signature".to_owned()));
}

#[tokio::test]
async fn tokens_signed_with_the_key_file_open_a_game_session() {
    use base64::Engine as _;
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    apply_migrations(&database_url).await;
    let dir = std::env::temp_dir().join(format!("pulsars-ws-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("key");
    std::fs::write(
        &path,
        base64::engine::general_purpose::STANDARD.encode([5u8; 32]),
    )
    .unwrap();
    let file = path.to_str().unwrap().to_owned();
    let own_kid = "2026-10-ed25519-8";
    let server = server_with(
        &database_url,
        &redis_url,
        fast_settings(),
        &[
            ("JWT_SIGNING_KEY_FILE", &file),
            ("JWT_SIGNING_KID", own_kid),
        ],
    )
    .await;
    wait_for_redis(&server.state).await;

    let signer = server.state.config.token_signer.clone().expect("signer");
    let sub = fresh_user().to_string();
    let mut claims = testing::claims(&sub, chrono::Utc::now().timestamp());
    claims["name"] = json!("api-signed");
    let token = pulsars_api::routes::token::sign(&signer, &claims);

    let mut client = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    client.send(json!({"type": "auth", "token": token})).await;
    let ok = client.until("auth.ok").await;
    assert_eq!(ok["player"]["user_id"], json!(sub));
    assert_eq!(ok["player"]["guest"], false);

    // Single use.
    let mut again = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    again.send(json!({"type": "auth", "token": token})).await;
    assert_eq!(again.closed().await.0, 4003);

    // The old site's issuer with the API's key: refused.
    claims["iss"] = json!("https://pulsars.science");
    claims["jti"] = json!(uuid::Uuid::now_v7().to_string());
    let forged = pulsars_api::routes::token::sign(&signer, &claims);
    let mut client = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    client.send(json!({"type": "auth", "token": forged})).await;
    assert_eq!(client.closed().await, (4001, "bad_issuer".to_owned()));

    // Another kid: refused.
    claims["iss"] = json!("https://api.pulsars.science");
    claims["jti"] = json!(uuid::Uuid::now_v7().to_string());
    let other = testing::sign(
        &signer.key,
        &json!({"alg": "EdDSA", "typ": "JWT", "kid": KID}),
        &claims,
    );
    let mut client = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    client.send(json!({"type": "auth", "token": other})).await;
    assert_eq!(client.closed().await, (4001, "unknown_kid".to_owned()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn guests_get_a_name_and_ping_works() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let (mut fr, ok) = Client::guest(&server, "fr", None).await;
    assert_eq!(ok["player"]["guest"], true);
    assert_eq!(ok["player"]["user_id"], Value::Null);
    assert_eq!(ok["player"]["locale"], "fr");
    let name = ok["player"]["name"].as_str().unwrap();
    assert!(
        name.starts_with("Invité ") && name.len() == "Invité 1234".len(),
        "{name}"
    );
    assert_eq!(
        ok["resume"].as_str().unwrap().len(),
        43,
        "32 bytes, base64url"
    );

    let (_, ok) = Client::guest(&server, "en", None).await;
    assert!(ok["player"]["name"].as_str().unwrap().starts_with("Guest "));

    let (_, ok) = Client::guest(&server, "ru-RU", None).await;
    assert_eq!(ok["player"]["locale"], "ru");
    assert!(ok["player"]["name"].as_str().unwrap().starts_with("Гость "));

    fr.send(json!({"type": "ping"})).await;
    let pong = fr.until("pong").await;
    assert!(pong["server_time"].as_i64().unwrap() > 1_700_000_000_000);

    fr.send(json!({"type": "dance"})).await;
    let error = fr.recv().await;
    assert_eq!(error["type"], "error");
    assert_eq!(error["code"], "bad_request");

    fr.send(json!({"type": "answer", "match_id": "x", "index": 0, "choice": 1}))
        .await;
    assert_eq!(fr.recv().await["code"], "not_allowed");
}

#[tokio::test]
async fn more_than_20_frames_a_second_closes_with_4029() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let (mut client, _) = Client::guest(&server, "fr", None).await;
    for _ in 0..25 {
        client.send(json!({"type": "ping"})).await;
    }
    assert_eq!(client.closed().await.0, 4029);
}

#[tokio::test]
async fn private_rooms_gather_players_and_the_host_starts() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let (mut host, _) = Client::guest(&server, "fr", None).await;
    let (mut friend, _) = Client::guest(&server, "en", None).await;
    let (mut third, _) = Client::guest(&server, "fr", None).await;

    host.send(json!({"type": "room.start"})).await;
    assert_eq!(host.recv().await["code"], "not_allowed");

    host.send(json!({"type": "room.create", "subject": "physics", "rules": {"duration": 60, "penalty": false}}))
        .await;
    let state = host.until("room.state").await;
    let code = state["code"].as_str().unwrap().to_owned();
    assert_eq!(code.len(), 6);
    assert_eq!(state["host_seat"], 0);
    assert_eq!(state["subject"], "physics");
    assert_eq!(
        state["rules"],
        json!({"duration": 60, "target": null, "penalty": false})
    );

    friend
        .send(json!({"type": "room.join", "code": "NOPE00"}))
        .await;
    assert_eq!(friend.recv().await["code"], "room_not_found");
    friend
        .send(json!({"type": "room.join", "code": code.to_lowercase()}))
        .await;
    let state = friend.until("room.state").await;
    assert_eq!(state["players"].as_array().unwrap().len(), 2);
    assert_eq!(host.until("room.state").await, state);

    friend.send(json!({"type": "room.start"})).await;
    assert_eq!(friend.recv().await["code"], "not_allowed", "host only");

    host.send(json!({"type": "room.start"})).await;
    let start = host.until("match.start").await;
    assert_eq!(start["mode"], "room");
    assert_eq!(start["ranked"], false);
    assert_eq!(start["seat"], 0);
    assert_eq!(start["players"][1]["guest"], true);
    let start_friend = friend.until("match.start").await;
    assert_eq!(start_friend["seat"], 1);
    assert_eq!(start_friend["match_id"], start["match_id"]);
    assert!(start["ends_at"].as_i64().unwrap() - start["starts_at"].as_i64().unwrap() == 60_000);

    third.send(json!({"type": "room.join", "code": code})).await;
    assert_eq!(third.recv().await["code"], "room_started");

    // Each player sees the question in their own language.
    let q_host = host.until("question").await;
    let q_friend = friend.until("question").await;
    assert_eq!(q_host["index"], 0);
    let match_id = uuid::Uuid::parse_str(start["match_id"].as_str().unwrap()).unwrap();
    let seed = server
        .state
        .game
        .match_seed(match_id)
        .await
        .expect("live match");
    let expected = &flash::duel_questions(Some(flash::Subject::Physics), seed, 1)[0];
    assert_eq!(q_host["question"], expected.fr.question.as_str());
    assert_eq!(q_friend["question"], expected.en.question.as_str());
    assert_eq!(q_host["choices"].as_array().unwrap().len(), 4);
    assert!(q_host.get("correct").is_none(), "the answer is never sent");

    // The friend leaves: in a two-player room the match ends.
    friend
        .send(json!({"type": "match.leave", "match_id": start["match_id"]}))
        .await;
    let end = host.until("match.end").await;
    assert_eq!(end["end_reason"], "void", "nobody answered");
    // Back in the room, without the leaver.
    let state = host.until("room.state").await;
    assert_eq!(state["players"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_russian_guest_gets_the_questions_in_russian() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let (mut host, _) = Client::guest(&server, "ru", None).await;
    let (mut friend, _) = Client::guest(&server, "fr", None).await;

    host.send(json!({"type": "room.create", "subject": "chemistry", "rules": {"duration": 60, "penalty": false}}))
        .await;
    let code = host.until("room.state").await["code"]
        .as_str()
        .unwrap()
        .to_owned();
    friend
        .send(json!({"type": "room.join", "code": code}))
        .await;
    friend.until("room.state").await;
    host.send(json!({"type": "room.start"})).await;
    let start = host.until("match.start").await;

    let q_host = host.until("question").await;
    let q_friend = friend.until("question").await;
    let match_id = uuid::Uuid::parse_str(start["match_id"].as_str().unwrap()).unwrap();
    let seed = server
        .state
        .game
        .match_seed(match_id)
        .await
        .expect("live match");
    let expected = &flash::duel_questions(Some(flash::Subject::Chemistry), seed, 1)[0];
    assert_eq!(q_host["question"], expected.ru.question.as_str());
    assert_eq!(q_host["series"], "Обозначения химических элементов");
    assert_eq!(
        q_host["choices"],
        json!(expected.ru.choices.as_slice()),
        "the right answer at the same index as in French"
    );
    assert_eq!(q_friend["question"], expected.fr.question.as_str());
    assert_eq!(q_friend["choices"], json!(expected.fr.choices.as_slice()));
}

#[tokio::test]
async fn guests_play_casual_duels_to_the_end() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let (mut a, _) = Client::guest(&server, "fr", None).await;
    let (mut b, ok_b) = Client::guest(&server, "fr", None).await;

    let join = json!({"type": "queue.join", "subject": "maths", "rules": target_rules()});
    a.send(join.clone()).await;
    let joined = a.until("queue.joined").await;
    assert_eq!(joined["ranked"], false);
    assert_eq!(joined["subject"], "maths");
    let waiting = a.until("queue.waiting").await;
    assert_eq!(waiting["ghost_available"], false);
    b.send(join).await;

    let start_a = a.until("match.start").await;
    let start_b = b.until("match.start").await;
    assert_eq!(start_a["mode"], "duel");
    assert_eq!(start_a["ranked"], false);
    assert_eq!(start_a["players"][0]["rating"], Value::Null);
    assert_eq!(
        start_a["ends_at"].as_i64().unwrap() - start_a["starts_at"].as_i64().unwrap(),
        300_000
    );
    let correct = answers(&server, &start_a).await;

    // `b` drops and comes back with its resume token.
    let resume = ok_b["resume"].as_str().unwrap().to_owned();
    drop(b);
    let disconnected = a.until("player.disconnected").await;
    assert_eq!(disconnected["seat"], start_b["seat"]);
    let (mut b, ok) = Client::guest(&server, "fr", Some(&resume)).await;
    assert_eq!(
        ok["player"]["name"],
        start_b["players"][start_b["seat"].as_u64().unwrap() as usize]["name"]
    );
    let resumed = b.until("match.start").await;
    assert_eq!(resumed["resumed"], true);
    assert_eq!(resumed["match_id"], start_a["match_id"]);
    a.until("player.reconnected").await;

    // `a` answers one wrong (−1), then eleven right: reaching the target
    // ends the match.
    let mut question = a.until("question").await;
    answer(&mut a, &question, &correct, false).await;
    for k in 0..11 {
        question = a.until("question").await;
        let result = answer(&mut a, &question, &correct, true).await;
        assert_eq!(result["score"], k);
    }
    let end = a.until("match.end").await;
    assert_eq!(end["end_reason"], "completed");
    assert_eq!(end["ranked"], false);
    let seat_a = start_a["seat"].as_u64().unwrap() as usize;
    let me = &end["results"][seat_a];
    assert_eq!(me["outcome"], "win");
    assert_eq!(me["rank"], 1);
    assert_eq!(me["score"], 10, "11 right, 1 wrong with the penalty");
    assert_eq!(me["answered"], 12);
    assert_eq!(me["correct"], 11);
    assert_eq!(me["rating_after"], Value::Null);
    let end_b = b.until("match.end").await;
    assert_eq!(end_b, end);
}

// --- With PostgreSQL and Redis ---------------------------------------------

#[tokio::test]
async fn a_ranked_duel_updates_ratings_and_credits_the_players() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let pool = migrated(&database_url).await;
    let mut settings = fast_settings();
    // A slow correct answer is anything over 50 ms here.
    settings.engine.suspicious_ms = 50;
    let server = server(&database_url, &redis_url, settings).await;
    wait_for_redis(&server.state).await;
    let (ida, idb) = (fresh_user(), fresh_user());
    // Accounts of the site: they get a result and brains.
    for (id, pseudo) in [(ida, "alice"), (idb, "bob")] {
        sqlx::query(
            "INSERT INTO site.users (id, pseudo, name, email, created_at, updated_at)
             VALUES ($1, $2 || $1, $2 || $1, $2 || $1 || '@example.invalid',
                     now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')",
        )
        .bind(id)
        .bind(pseudo)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (mut a, ok) = Client::account(&server, &ida.to_string(), "alice").await;
    assert_eq!(
        ok["player"],
        json!({"user_id": ida.to_string(), "name": "alice", "guest": false, "locale": "fr"})
    );
    assert_eq!(ok["resume"], Value::Null);
    let (mut b, _) = Client::account(&server, &idb.to_string(), "bob").await;

    let join = json!({"type": "queue.join", "subject": "chemistry", "rules": target_rules()});
    a.send(join.clone()).await;
    assert_eq!(a.until("queue.joined").await["ranked"], true);
    b.send(join).await;
    let start = a.until("match.start").await;
    b.until("match.start").await;
    assert_eq!(start["ranked"], true);
    assert_eq!(start["players"][0]["rating"], 1500);
    let correct = answers(&server, &start).await;
    let seat_a = start["seat"].as_u64().unwrap() as usize;

    // `b` answers once, `a` reaches 10, slowly enough not to be suspicious.
    let qb = b.until("question").await;
    answer(&mut b, &qb, &correct, true).await;
    for _ in 0..10 {
        let q = a.until("question").await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        answer(&mut a, &q, &correct, true).await;
    }
    let end = a.until("match.end").await;
    assert_eq!(end["ranked"], true, "{end}");
    assert_eq!(end["end_reason"], "completed");
    let winner = &end["results"][seat_a];
    let loser = &end["results"][1 - seat_a];
    assert_eq!(winner["outcome"], "win");
    assert_eq!(loser["outcome"], "loss");
    assert_eq!(winner["rating_before"], 1500);
    assert!(winner["rating_after"].as_i64().unwrap() > 1600, "{winner}");
    assert!(loser["rating_after"].as_i64().unwrap() < 1400, "{loser}");

    let match_id = uuid::Uuid::parse_str(start["match_id"].as_str().unwrap()).unwrap();
    let (games, wins): (i32, i32) = sqlx::query_as(
        "SELECT games, wins FROM game.ratings WHERE user_id = $1 AND subject = 'chemistry'",
    )
    .bind(ida)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((games, wins), (1, 1));
    let (end_reason, ranked): (String, bool) =
        sqlx::query_as("SELECT end_reason, ranked FROM game.matches WHERE id = $1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((end_reason.as_str(), ranked), ("completed", true));
    let log: Value = sqlx::query_scalar(
        "SELECT answers FROM game.match_players WHERE match_id = $1 AND user_id = $2",
    )
    .bind(match_id)
    .bind(ida)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(log.as_array().unwrap().len(), 10);
    assert_eq!(log[0]["i"], 0);
    assert_eq!(log[0]["ok"], true);

    // The site's results, from which XP is computed, and the brains: 2 a
    // match, 3 more for the win (Laravel's `EvenementController`).
    let results: Vec<(i64, String, bool, String)> = sqlx::query_as(
        "SELECT user_id, mode::text, ranked, outcome::text FROM site.game_results
          WHERE match_id = $1 ORDER BY user_id",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let outcome = |id: i64| if id == ida { "win" } else { "loss" };
    let mut expected: Vec<(i64, String, bool, String)> = [ida, idb]
        .iter()
        .map(|&id| (id, "flash-duel".to_owned(), true, outcome(id).to_owned()))
        .collect();
    expected.sort();
    assert_eq!(results, expected);
    let brains: Vec<(i64, String, i32)> = sqlx::query_as(
        "SELECT user_id, cle::text, montant FROM site.cerveaux_mouvements
          WHERE user_id = ANY($1) ORDER BY user_id, montant",
    )
    .bind([ida, idb])
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut expected = vec![
        (ida, format!("partie:{match_id}"), 2),
        (ida, format!("victoire:{match_id}"), 3),
        (idb, format!("partie:{match_id}"), 2),
    ];
    expected.sort_by_key(|(id, _, amount)| (*id, *amount));
    assert_eq!(brains, expected);

    // `/v1/game/me` lists the ratings and the match.
    let request = Request::get("/v1/game/me")
        .header(
            header::AUTHORIZATION,
            format!(
                "Bearer {}",
                server.keys.token(&ida.to_string(), "alice", "fr")
            ),
        )
        .body(Body::empty())
        .unwrap();
    let response = server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let me: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(me["ratings"][0]["subject"], "chemistry");
    assert_eq!(me["ratings"][0]["games"], 1);
    assert_eq!(me["ratings"][0]["provisional"], true);
    assert_eq!(me["recent"][0]["match_id"], match_id.to_string());
    assert_eq!(me["recent"][0]["outcome"], "win");
    assert_eq!(me["recent"][0]["opponents"], json!(["bob"]));
    assert_eq!(me["recent"][0]["rules"], target_rules());

    // A third account can now play against one of these runs.
    let idc = fresh_user();
    let (mut c, _) = Client::account(&server, &idc.to_string(), "carol").await;
    c.send(json!({"type": "ghost.start", "subject": "chemistry", "rules": {"duration": 60, "penalty": true}}))
        .await;
    assert_eq!(c.recv().await["code"], "no_ghost", "other rules");
    c.send(json!({"type": "ghost.start", "subject": "chemistry", "rules": target_rules()}))
        .await;
    let ghost_start = c.until("match.start").await;
    assert_eq!(ghost_start["mode"], "ghost");
    assert_eq!(ghost_start["ranked"], false);
    assert_eq!(ghost_start["players"][1]["ghost"], true);
    let ghost_name = ghost_start["players"][1]["name"].as_str().unwrap();
    assert!(["alice", "bob"].contains(&ghost_name), "{ghost_name}");
    // Same seed: the same questions as the recorded run.
    assert_eq!(answers(&server, &ghost_start).await, correct);
    c.send(json!({"type": "match.leave", "match_id": ghost_start["match_id"]}))
        .await;
    let ghost_end = c.until("match.end").await;
    assert_eq!(ghost_end["end_reason"], "void");

    // The token's `jti` opened one session; a replay is refused.
    let token = server.keys.token(&idc.to_string(), "carol", "fr");
    let mut first = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    first.send(json!({"type": "auth", "token": token})).await;
    first.until("auth.ok").await;
    // ...and closed `c`, the previous session of the account.
    assert_eq!(c.closed().await.0, 4009);
    let mut replay = Client::connect(&server, Some(ORIGIN)).await.unwrap();
    replay.send(json!({"type": "auth", "token": token})).await;
    assert_eq!(replay.closed().await.0, 4003);

    let ghost_id = uuid::Uuid::parse_str(ghost_start["match_id"].as_str().unwrap()).unwrap();
    cleanup(&pool, &[match_id, ghost_id], &[ida, idb, idc]).await;
}

async fn cleanup(pool: &sqlx::PgPool, matches: &[uuid::Uuid], users: &[i64]) {
    for id in matches {
        sqlx::query("DELETE FROM game.matches WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM game.ratings WHERE user_id = ANY($1)")
        .bind(users)
        .execute(pool)
        .await
        .unwrap();
    // Their results and brains go with them.
    sqlx::query("DELETE FROM site.users WHERE id = ANY($1)")
        .bind(users)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_leaderboard_is_public_and_skips_deleted_accounts() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let pool = migrated(&database_url).await;
    let server = server(&database_url, &redis_url, fast_settings()).await;

    let member: (i64, String) =
        sqlx::query_as("SELECT id, pseudo FROM site.members ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let gone = fresh_user();
    // A subject key for this test only would be nicer; `computer-science`
    // is rarely played locally. Existing rows are left alone.
    let existed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM game.ratings WHERE user_id = $1 AND subject = 'computer-science')",
    )
    .bind(member.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    if !existed {
        sqlx::query(
            "INSERT INTO game.ratings (user_id, subject, rating, rd, volatility, games, wins, losses, draws)
             VALUES ($1, 'computer-science', 9000.4, 60, 0.06, 3, 2, 1, 0)",
        )
        .bind(member.0)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO game.ratings (user_id, subject, rating, rd, volatility, games, wins, losses, draws)
         VALUES ($1, 'computer-science', 9999, 60, 0.06, 1, 1, 0, 0)",
    )
    .bind(gone)
    .execute(&pool)
    .await
    .unwrap();

    let request = Request::get("/v1/game/leaderboard?subject=computer-science&limit=100")
        .header(header::ORIGIN, "https://anyone.example")
        .body(Body::empty())
        .unwrap();
    let response = server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=30"
    );
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(body["subject"], "computer-science");
    let data = body["data"].as_array().unwrap();
    assert!(
        data.iter()
            .all(|row| row["user_id"].as_str() != Some(gone.to_string().as_str())),
        "deleted accounts are skipped"
    );
    let row = data
        .iter()
        .find(|row| row["user_id"].as_str() == Some(member.0.to_string().as_str()))
        .expect("the member is listed");
    assert_eq!(row["pseudo"], member.1);
    if !existed {
        assert_eq!(row["rank"], 1);
        assert_eq!(row["rating"], 9000);
        assert_eq!(row["provisional"], false);
        assert_eq!(
            (row["games"].clone(), row["wins"].clone()),
            (json!(3), json!(2))
        );
    }
    for window in data.windows(2) {
        assert!(window[0]["rating"].as_i64() >= window[1]["rating"].as_i64());
    }

    let (status, problem) = {
        let request = Request::get("/v1/game/leaderboard?subject=biology&limit=0")
            .body(Body::empty())
            .unwrap();
        let response = server.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        (status, body)
    };
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["errors"].as_array().unwrap().len(), 2);

    if !existed {
        sqlx::query("DELETE FROM game.ratings WHERE user_id = $1 AND subject = 'computer-science'")
            .bind(member.0)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM game.ratings WHERE user_id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn game_me_needs_a_token_and_the_game_origin() {
    let server = server(DEAD_POSTGRES, DEAD_REDIS, fast_settings()).await;
    let request = Request::get("/v1/game/me")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let response = server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    let request = Request::get("/v1/game/me")
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    let response = server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        ORIGIN
    );
}

#[tokio::test]
async fn game_me_shows_the_brains_balance() {
    let Some((database_url, redis_url)) = integration_urls() else {
        return;
    };
    let server = server(&database_url, &redis_url, fast_settings()).await;
    wait_for_redis(&server.state).await;
    let me = |sub: String| {
        let token = server.keys.token(&sub, "x", "fr");
        let app = server.app.clone();
        async move {
            let request = Request::get("/v1/game/me")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap();
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap())
                    .unwrap();
            assert_eq!(status, StatusCode::OK, "{body}");
            body
        }
    };
    // `ada` (dev/db/30-demo-data.sql): three lessons, 5 brains each.
    assert_eq!(me("2".to_owned()).await["brains"], 15);
    // An account without movements, or unknown: 0.
    assert_eq!(me(fresh_user().to_string()).await["brains"], 0);
}
