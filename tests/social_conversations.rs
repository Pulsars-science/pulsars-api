//! Private messages (`src/routes/social/conversations.rs`), sealed with
//! `MESSAGES_KEY`. Ported from the site's `AmitieMessagerieTest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, TestMember, codes};
use serde_json::{Value, json};

const KEY: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

async fn env() -> Option<Env> {
    Env::with(&[("MESSAGES_KEY", KEY)]).await
}

async fn befriend(env: &Env, a: &TestMember, b: &TestMember) -> i64 {
    let (one, two) = if a.id < b.id {
        (a.id, b.id)
    } else {
        (b.id, a.id)
    };
    sqlx::query_scalar(
        "INSERT INTO site.friendships (user_one_id, user_two_id, requested_by_id, accepted_at,
                                       created_at, updated_at)
         VALUES ($1, $2, $1, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(one)
    .bind(two)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

async fn open(env: &Env, me: &TestMember, other: &TestMember) -> (StatusCode, Value) {
    Call::post("/v1/conversations")
        .member(me)
        .json(json!({"pseudo": other.pseudo}))
        .send(&env.app)
        .await
}

/// Two friends and their conversation.
async fn pair(env: &Env) -> (TestMember, TestMember, i64) {
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    befriend(env, &alice, &bob).await;
    let (status, body) = open(env, &alice, &bob).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (alice, bob, body["data"]["id"].as_i64().unwrap())
}

async fn send(env: &Env, from: &TestMember, id: i64, body: &str) -> (StatusCode, Value) {
    Call::post(format!("/v1/conversations/{id}/messages"))
        .member(from)
        .json(json!({"body": body}))
        .send(&env.app)
        .await
}

async fn message_count(env: &Env, id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM site.direct_messages WHERE direct_conversation_id = $1",
    )
    .bind(id)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

#[tokio::test]
async fn only_friends_start_a_conversation() {
    let Some(env) = env().await else { return };
    let alice = env.member().verified().create().await;
    let bob = env.member().verified().create().await;

    let (status, _) = open(&env, &alice, &bob).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = open(&env, &alice, &alice).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("pseudo".to_owned(), "self".to_owned())]);

    befriend(&env, &alice, &bob).await;
    let (status, body) = open(&env, &alice, &bob).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["data"]["id"].as_i64().unwrap();
    let (status, body) = open(&env, &bob, &alice).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"], id);

    let (status, _) = Call::get("/v1/conversations").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_conversation_stays_strictly_private() {
    let Some(env) = env().await else { return };
    let (_, _, id) = pair(&env).await;
    let charlie = env.member().verified().create().await;
    for call in [
        Call::get(format!("/v1/conversations/{id}")),
        Call::get(format!("/v1/conversations/{id}/messages")),
        Call::post(format!("/v1/conversations/{id}/read")),
    ] {
        let (status, _) = call.member(&charlie).send(&env.app).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _) = send(&env, &charlie, id, "Intrusion").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(message_count(&env, id).await, 0);
    let (status, _) = Call::get("/v1/conversations/abc")
        .member(&charlie)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_message_is_sent_then_read_when_opened() {
    let Some(env) = env().await else { return };
    let (alice, bob, id) = pair(&env).await;
    let text = "Salut Bob, peux-tu m’aider ?";

    let (status, body) = send(&env, &alice, id, &format!("  {text} ")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["body"], text);
    assert_eq!(body["data"]["author"], alice.pseudo.as_str());
    assert_eq!(body["data"]["author_id"], alice.id.to_string());
    assert_eq!(body["data"]["read"], false);
    let last: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT last_message_at FROM site.direct_conversations WHERE id = $1")
            .bind(id)
            .fetch_one(&env.owner)
            .await
            .unwrap();
    assert!(last.is_some());

    let (status, body) = Call::get("/v1/conversations")
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let line = &body["data"]["conversations"][0];
    assert_eq!(line["id"], id);
    assert_eq!(line["unread_count"], 1);
    assert_eq!(line["last_message"], text);
    assert_eq!(line["last_message_from_me"], false);
    assert_eq!(line["with"]["pseudo"], alice.pseudo.as_str());
    assert_eq!(body["data"]["friends"][0]["pseudo"], alice.pseudo.as_str());
    let (_, body) = Call::get("/v1/conversations")
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["conversations"][0]["unread_count"], 0);
    assert_eq!(
        body["data"]["conversations"][0]["last_message_from_me"],
        true
    );

    let (status, body) = Call::get(format!("/v1/conversations/{id}"))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["with"]["pseudo"], alice.pseudo.as_str());
    assert_eq!(body["data"]["is_friend"], true);
    assert_eq!(body["data"]["messages"][0]["body"], text);
    assert_eq!(body["data"]["has_older"], false);

    let (_, body) = Call::get(format!("/v1/conversations/{id}/messages"))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(body["data"][0]["read"], true);
    let (_, body) = Call::get("/v1/conversations")
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["conversations"][0]["unread_count"], 0);
}

#[tokio::test]
async fn reading_can_be_marked_on_its_own_and_polled() {
    let Some(env) = env().await else { return };
    let (alice, bob, id) = pair(&env).await;
    let (_, first) = send(&env, &alice, id, "un").await;
    send(&env, &alice, id, "deux").await;

    let (status, _) = Call::post(format!("/v1/conversations/{id}/read"))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = Call::get("/v1/conversations")
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["conversations"][0]["unread_count"], 0);

    let after = first["data"]["id"].as_i64().unwrap();
    let (_, body) = Call::get(format!("/v1/conversations/{id}/messages?after={after}"))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["body"], "deux");
    assert_eq!(body["has_more"], false);
    let (_, body) = Call::get(format!("/v1/conversations/{id}/messages?limit=1"))
        .member(&bob)
        .send(&env.app)
        .await;
    assert_eq!(body["data"][0]["body"], "deux");
    assert_eq!(body["has_more"], true);
}

#[tokio::test]
async fn bodies_are_sealed_at_rest() {
    let Some(env) = env().await else { return };
    let (alice, _, id) = pair(&env).await;
    let secret = "Rendez-vous à 14h, code portail 4791.";
    let (status, body) = send(&env, &alice, id, secret).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["body"], secret);

    let (raw, version): (String, i16) = sqlx::query_as(
        "SELECT contenu, key_version FROM site.direct_messages WHERE direct_conversation_id = $1",
    )
    .bind(id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(version, 1);
    assert_ne!(raw, secret);
    assert!(!raw.contains("portail") && !raw.contains("4791"));
}

#[tokio::test]
async fn removing_a_friend_closes_the_conversation() {
    let Some(env) = env().await else { return };
    let (alice, bob, id) = pair(&env).await;
    send(&env, &bob, id, "avant").await;
    let (_, body) = Call::get("/v1/friends").member(&alice).send(&env.app).await;
    let friendship = body["data"]["friends"][0]["friendship_id"]
        .as_i64()
        .unwrap();
    let (status, _) = Call::delete(format!("/v1/friends/{friendship}"))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(&env, &alice, id, "Message bloqué").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message_count(&env, id).await, 1);
    // Still readable, marked as no longer friends.
    let (status, body) = Call::get(format!("/v1/conversations/{id}"))
        .member(&alice)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["is_friend"], false);
    assert_eq!(body["data"]["messages"][0]["body"], "avant");
}

#[tokio::test]
async fn muted_members_cannot_write_and_bodies_are_checked() {
    let Some(env) = env().await else { return };
    let (alice, _, id) = pair(&env).await;
    for (body, code) in [
        ("", "required"),
        ("   ", "required"),
        (&"x".repeat(2001) as &str, "too_long"),
    ] {
        let (status, problem) = send(&env, &alice, id, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(codes(&problem), [("body".to_owned(), code.to_owned())]);
    }
    sqlx::query("UPDATE site.users SET mute_jusqu_a = (now() + interval '1 hour') AT TIME ZONE 'UTC' WHERE id = $1")
        .bind(alice.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = send(&env, &alice, id, "Bonjour").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message_count(&env, id).await, 0);
}

#[tokio::test]
async fn private_messages_need_the_key() {
    let Some(keyed) = env().await else { return };
    let (alice, _, id) = pair(&keyed).await;
    let Some(env) = Env::new().await else { return };
    for call in [
        Call::get("/v1/conversations"),
        Call::get(format!("/v1/conversations/{id}")),
        Call::post(format!("/v1/conversations/{id}/messages")).json(json!({"body": "x"})),
    ] {
        let (status, body) = call.member(&alice).send(&env.app).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body["type"],
            "https://api.pulsars.science/problems/messages-unavailable"
        );
        assert!(body["detail"].as_str().unwrap().contains("MESSAGES_KEY"));
    }
}

#[tokio::test]
async fn private_messages_are_rate_limited() {
    let Some(env) = env().await else { return };
    let (alice, _, id) = pair(&env).await;
    let mut last = StatusCode::CREATED;
    for i in 0..61 {
        (last, _) = send(&env, &alice, id, &format!("m{i}")).await;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}
