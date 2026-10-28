//! Rooms and their invitation links (`src/routes/social/rooms.rs`). Ported
//! from the site's `SalonTest` and `InvitationSalonTest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, TestMember, codes, unique};
use serde_json::{Value, json};

/// A room made through the API; its slug.
async fn room(env: &Env, owner: &TestMember, kind: &str) -> String {
    let (status, body) = Call::post("/v1/rooms")
        .member(owner)
        .json(json!({"name": format!("Salon {}", unique()), "type": kind}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["data"]["slug"].as_str().unwrap().to_owned()
}

async fn is_member(env: &Env, slug: &str, member: &TestMember) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.room_members m JOIN site.rooms r ON r.id = m.room_id
                         WHERE r.slug = $1 AND m.user_id = $2)",
    )
    .bind(slug)
    .bind(member.id)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

/// An invitation link made through the API; its token.
async fn invite(env: &Env, slug: &str, owner: &TestMember, body: Value) -> String {
    let (status, body) = Call::post(format!("/v1/rooms/{slug}/invitations"))
        .member(owner)
        .json(body)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["data"]["token"].as_str().unwrap().to_owned()
}

async fn uses(env: &Env, token: &str) -> i32 {
    sqlx::query_scalar("SELECT usages FROM site.room_invites WHERE token = $1")
        .bind(token)
        .fetch_one(&env.owner)
        .await
        .unwrap()
}

fn listed(body: &Value, slug: &str) -> bool {
    body["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["slug"] == slug)
}

#[tokio::test]
async fn creating_a_room_makes_its_creator_the_owner() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let tag = unique();
    let (status, body) = Call::post("/v1/rooms")
        .member(&owner)
        .json(json!({"name": format!("  Mon Salon {tag} "), "type": "public", "description": "  "}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let data = &body["data"];
    assert_eq!(data["slug"], format!("mon-salon-{tag}"));
    assert_eq!(data["name"], format!("Mon Salon {tag}"));
    assert_eq!(data["description"], Value::Null);
    assert_eq!(
        (data["type"].clone(), data["is_private"].clone()),
        (json!("public"), json!(false))
    );
    assert_eq!(data["member_count"], 1);
    assert_eq!(
        data["viewer"],
        json!({"joined": true, "can_write": true, "can_manage": true})
    );
    let role: String = sqlx::query_scalar(
        "SELECT m.role FROM site.room_members m JOIN site.rooms r ON r.id = m.room_id
          WHERE r.slug = $1 AND m.user_id = $2",
    )
    .bind(format!("mon-salon-{tag}"))
    .bind(owner.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(role, "proprietaire");
}

#[tokio::test]
async fn a_slug_stays_unique_on_a_collision() {
    let Some(env) = Env::new().await else { return };
    let a = env.member().verified().create().await;
    let b = env.member().verified().create().await;
    let name = format!("Général {}", unique());
    let mut slugs = Vec::new();
    for owner in [&a, &b] {
        let (_, body) = Call::post("/v1/rooms")
            .member(owner)
            .json(json!({"name": name, "type": "public"}))
            .send(&env.app)
            .await;
        slugs.push(body["data"]["slug"].as_str().unwrap().to_owned());
    }
    assert!(slugs[0].starts_with("general-"), "{slugs:?}");
    assert_ne!(slugs[0], slugs[1]);
    assert!(slugs[1].starts_with(&format!("{}-", slugs[0])), "{slugs:?}");
    assert_eq!(slugs[1].len(), slugs[0].len() + 5);
}

#[tokio::test]
async fn a_room_is_validated_as_laravel_did() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let (status, body) = Call::post("/v1/rooms")
        .member(&owner)
        .json(json!({"name": "x", "type": "private", "description": "d".repeat(281)}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("name".to_owned(), "too_short".to_owned()),
            ("description".to_owned(), "too_long".to_owned()),
            ("type".to_owned(), "invalid".to_owned()),
        ]
    );
    let (_, body) = Call::post("/v1/rooms")
        .member(&owner)
        .json(json!({"name": "n".repeat(51)}))
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        [
            ("name".to_owned(), "too_long".to_owned()),
            ("type".to_owned(), "required".to_owned()),
        ]
    );
    let (status, _) = Call::post("/v1/rooms")
        .member(&owner)
        .json(json!({"name": "ok", "type": "public", "owner": 1}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn who_sees_a_room() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let other = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let public = room(&env, &owner, "public").await;
    let private = room(&env, &owner, "prive").await;

    let (status, body) = Call::get(format!("/v1/rooms/{public}"))
        .member(&other)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"]["viewer"],
        json!({"joined": false, "can_write": true, "can_manage": false})
    );

    let (status, _) = Call::get(format!("/v1/rooms/{private}"))
        .member(&other)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, body) = Call::get("/v1/rooms").member(&other).send(&env.app).await;
    assert!(listed(&body, &public));
    assert!(!listed(&body, &private));

    // A moderator reads it and manages it, without it being listed.
    let (status, body) = Call::get(format!("/v1/rooms/{private}"))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["viewer"]["can_manage"], true);
    assert_eq!(body["data"]["viewer"]["can_write"], false);
    let (_, body) = Call::get("/v1/rooms")
        .member(&moderator)
        .send(&env.app)
        .await;
    assert!(!listed(&body, &private));

    let (_, body) = Call::get("/v1/rooms").member(&owner).send(&env.app).await;
    let mine = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["slug"] == private)
        .unwrap();
    assert_eq!(mine["type"], "prive");
    assert_eq!(mine["is_private"], true);
    assert_eq!(mine["joined"], true);
    assert_eq!(mine["member_count"], 1);
}

#[tokio::test]
async fn rooms_need_a_verified_member() {
    let Some(env) = Env::new().await else { return };
    let fresh = env.member().verified_only().create().await;
    let (status, _) = Call::get("/v1/rooms").member(&fresh).send(&env.app).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::get("/v1/rooms").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn public_rooms_are_joined_freely_private_ones_by_invitation() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let member = env.member().verified().create().await;
    let public = room(&env, &owner, "public").await;
    let private = room(&env, &owner, "prive").await;

    for _ in 0..2 {
        let (status, _) = Call::post(format!("/v1/rooms/{public}/members"))
            .member(&member)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    assert!(is_member(&env, &public, &member).await);
    let (status, _) = Call::post(format!("/v1/rooms/{private}/members"))
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!is_member(&env, &private, &member).await);

    // The owner cannot leave; a member can.
    let (status, body) = Call::delete(format!("/v1/rooms/{public}/members/me"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("room".to_owned(), "owner".to_owned())]);
    assert!(is_member(&env, &public, &owner).await);
    let (status, _) = Call::delete(format!("/v1/rooms/{public}/members/me"))
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!is_member(&env, &public, &member).await);
}

#[tokio::test]
async fn writing_in_a_public_room_joins_it() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let writer = env.member().verified().create().await;
    let public = room(&env, &owner, "public").await;
    let private = room(&env, &owner, "prive").await;

    let (status, body) = Call::post(format!("/v1/rooms/{public}/messages"))
        .member(&writer)
        .json(json!({"body": "  Bonjour tout le monde "}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["body"], "Bonjour tout le monde");
    assert_eq!(body["data"]["author"], writer.pseudo.as_str());
    assert_eq!(body["data"]["author_id"], writer.id.to_string());
    assert!(body["data"]["sent_at"].is_string());
    assert!(is_member(&env, &public, &writer).await);

    let (status, _) = Call::post(format!("/v1/rooms/{private}/messages"))
        .member(&writer)
        .json(json!({"body": "Coucou"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for (body, code) in [
        (json!({"body": ""}), "required"),
        (json!({}), "required"),
        (json!({"body": "x".repeat(2001)}), "too_long"),
    ] {
        let (status, problem) = Call::post(format!("/v1/rooms/{public}/messages"))
            .member(&writer)
            .json(body)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(codes(&problem), [("body".to_owned(), code.to_owned())]);
    }
}

#[tokio::test]
async fn a_moderator_outside_a_private_room_cannot_write_in_it() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let private = room(&env, &owner, "prive").await;
    let (status, _) = Call::post(format!("/v1/rooms/{private}/messages"))
        .member(&moderator)
        .json(json!({"body": "Bonjour"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_muted_member_cannot_write() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let public = room(&env, &owner, "public").await;
    sqlx::query("UPDATE site.users SET mute_jusqu_a = (now() + interval '1 hour') AT TIME ZONE 'UTC' WHERE id = $1")
        .bind(owner.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = Call::post(format!("/v1/rooms/{public}/messages"))
        .member(&owner)
        .json(json!({"body": "Bonjour"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // An expired mute no longer holds.
    sqlx::query("UPDATE site.users SET mute_jusqu_a = (now() - interval '1 hour') AT TIME ZONE 'UTC' WHERE id = $1")
        .bind(owner.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = Call::post(format!("/v1/rooms/{public}/messages"))
        .member(&owner)
        .json(json!({"body": "Bonjour"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn the_thread_pages_back_and_polls_forward() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let slug = room(&env, &owner, "public").await;
    let mut ids = Vec::new();
    for i in 0..5 {
        let (_, body) = Call::post(format!("/v1/rooms/{slug}/messages"))
            .member(&owner)
            .json(json!({"body": format!("m{i}")}))
            .send(&env.app)
            .await;
        ids.push(body["data"]["id"].as_i64().unwrap());
    }
    let bodies = |body: &Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["body"].as_str().unwrap().to_owned())
            .collect()
    };

    let (_, body) = Call::get(format!("/v1/rooms/{slug}"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["messages"][0]["body"], "m0");
    assert_eq!(body["data"]["messages"][4]["body"], "m4");
    assert_eq!(body["data"]["has_older"], false);

    let (status, body) = Call::get(format!("/v1/rooms/{slug}/messages?limit=2"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (bodies(&body), body["has_more"].clone()),
        (vec!["m3".into(), "m4".into()], json!(true))
    );
    let (_, body) = Call::get(format!(
        "/v1/rooms/{slug}/messages?limit=2&before={}",
        ids[3]
    ))
    .member(&owner)
    .send(&env.app)
    .await;
    assert_eq!(bodies(&body), ["m1", "m2"]);
    assert_eq!(body["has_more"], true);
    let (_, body) = Call::get(format!("/v1/rooms/{slug}/messages?after={}", ids[2]))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(bodies(&body), ["m3", "m4"]);
    assert_eq!(body["has_more"], false);
    let (_, body) = Call::get(format!("/v1/rooms/{slug}/messages?after={}", ids[4]))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(body, json!({"data": [], "has_more": false}));
    let (status, _) = Call::post(format!("/v1/rooms/{slug}/read"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = Call::get(format!("/v1/rooms/{slug}/messages?after=1&before=2"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn only_a_manager_deletes_a_room_with_its_messages() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let intruder = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let slug = room(&env, &owner, "public").await;
    Call::post(format!("/v1/rooms/{slug}/messages"))
        .member(&owner)
        .json(json!({"body": "Un message"}))
        .send(&env.app)
        .await;
    let room_id: i64 = sqlx::query_scalar("SELECT id FROM site.rooms WHERE slug = $1")
        .bind(&slug)
        .fetch_one(&env.owner)
        .await
        .unwrap();

    let (status, _) = Call::delete(format!("/v1/rooms/{slug}"))
        .member(&intruder)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::delete(format!("/v1/rooms/{slug}"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM site.room_messages WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&env.owner)
            .await
            .unwrap();
    assert_eq!(left, 0);
    let (status, _) = Call::get(format!("/v1/rooms/{slug}"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A moderator deletes any room.
    let other = room(&env, &owner, "prive").await;
    let (status, _) = Call::delete(format!("/v1/rooms/{other}"))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// --- Invitations -------------------------------------------------------------

#[tokio::test]
async fn an_invitation_link_lets_one_into_a_private_room() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let guest = env.member().verified().create().await;
    let slug = room(&env, &owner, "prive").await;
    let token = invite(&env, &slug, &owner, json!({})).await;

    // The landing page shows the room, without joining or counting a use.
    let (status, body) = Call::get(format!("/v1/invitations/{token}"))
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["room"]["slug"], slug.as_str());
    assert_eq!(body["data"]["room"]["is_private"], true);
    assert_eq!(body["data"]["already_member"], false);
    assert_eq!(body["data"]["valid"], true);
    assert_eq!(body["data"]["invited_by"], owner.pseudo.as_str());
    assert!(!is_member(&env, &slug, &guest).await);
    assert_eq!(uses(&env, &token).await, 0);

    let (status, body) = Call::post(format!("/v1/invitations/{token}/accept"))
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], json!({"room_slug": slug}));
    assert!(is_member(&env, &slug, &guest).await);
    assert_eq!(uses(&env, &token).await, 1);

    // Again, already in: no use counted.
    Call::post(format!("/v1/invitations/{token}/accept"))
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(uses(&env, &token).await, 1);
    let (status, _) = Call::get(format!("/v1/rooms/{slug}"))
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = Call::get(format!("/v1/invitations/{}", unique()))
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn expired_exhausted_and_revoked_links_are_refused() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let slug = room(&env, &owner, "prive").await;

    let expired = invite(&env, &slug, &owner, json!({})).await;
    sqlx::query("UPDATE site.room_invites SET expires_at = (now() - interval '1 minute') AT TIME ZONE 'UTC' WHERE token = $1")
        .bind(&expired)
        .execute(&env.owner)
        .await
        .unwrap();
    let once = invite(&env, &slug, &owner, json!({"max_uses": 1})).await;
    let revoked = invite(&env, &slug, &owner, json!({})).await;
    let (status, _) = Call::delete(format!("/v1/rooms/{slug}/invitations/{revoked}"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let first = env.member().verified().create().await;
    let (status, _) = Call::post(format!("/v1/invitations/{once}/accept"))
        .member(&first)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);

    let late = env.member().verified().create().await;
    for (token, code) in [
        (&expired, "expired"),
        (&once, "exhausted"),
        (&revoked, "revoked"),
    ] {
        let (status, body) = Call::post(format!("/v1/invitations/{token}/accept"))
            .member(&late)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}");
        assert_eq!(codes(&body), [("invitation".to_owned(), code.to_owned())]);
        let (_, body) = Call::get(format!("/v1/invitations/{token}"))
            .member(&late)
            .send(&env.app)
            .await;
        assert_eq!(body["data"]["valid"], false);
        assert_eq!(body["data"]["invalid_reason"], code);
    }
    assert!(!is_member(&env, &slug, &late).await);

    let (status, body) = Call::get(format!("/v1/rooms/{slug}/invitations"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let reasons: Vec<Value> = body["data"]["invitations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["invalid_reason"].clone())
        .collect();
    assert_eq!(
        reasons,
        [json!("revoked"), json!("exhausted"), json!("expired")]
    );
}

#[tokio::test]
async fn only_a_manager_handles_the_links_of_a_room() {
    let Some(env) = Env::new().await else { return };
    let owner = env.member().verified().create().await;
    let intruder = env.member().verified().create().await;
    let slug = room(&env, &owner, "prive").await;

    let (status, _) = Call::post(format!("/v1/rooms/{slug}/invitations"))
        .member(&intruder)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::get(format!("/v1/rooms/{slug}/invitations"))
        .member(&intruder)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = Call::post(format!("/v1/rooms/{slug}/invitations"))
        .member(&owner)
        .json(json!({"duration_days": 7, "max_uses": 5}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let link = &body["data"];
    assert_eq!(link["max_uses"], 5);
    assert_eq!(link["uses"], 0);
    assert_eq!(link["valid"], true);
    assert_eq!(link["creator"], owner.pseudo.as_str());
    let token = link["token"].as_str().unwrap();
    assert_eq!(token.len(), 24);
    assert_eq!(
        link["url"],
        format!("https://pulsars.science/salons/invite/{token}")
    );
    let expires =
        chrono::DateTime::parse_from_rfc3339(link["expires_at"].as_str().unwrap()).unwrap();
    let days = (expires.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_hours();
    assert!((167..=168).contains(&days), "{days}");

    let (status, body) = Call::post(format!("/v1/rooms/{slug}/invitations"))
        .member(&owner)
        .json(json!({"duration_days": 366, "max_uses": 0}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("duration_days".to_owned(), "out_of_range".to_owned()),
            ("max_uses".to_owned(), "out_of_range".to_owned()),
        ]
    );

    // A link of another room is not revoked through this one.
    let other = room(&env, &owner, "prive").await;
    let foreign = invite(&env, &other, &owner, json!({})).await;
    let (status, _) = Call::delete(format!("/v1/rooms/{slug}/invitations/{foreign}"))
        .member(&owner)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
