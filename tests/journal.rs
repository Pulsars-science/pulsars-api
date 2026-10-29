//! The admin journal end to end: the page view and consent beacons, the
//! security events of the auth routes, the linking of a visitor to the
//! member who logs in, the purge, and `/v1/admin/people/*` and
//! `/v1/admin/activity` (`docs/privacy.md`).
//!
//! Every test uses its own members and visitor ids, so they run in
//! parallel and again on the same database.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::{StatusCode, header};
use common::{Call, Env, TestMember, unique};
use serde_json::{Value, json};
use uuid::Uuid;

/// A row of `site.consents`: visitor, member, measure, fingerprint.
type ConsentRow = (Option<Uuid>, Option<i64>, bool, Option<String>);

fn visitor() -> String {
    Uuid::new_v4().to_string()
}

async fn admin(env: &Env) -> TestMember {
    env.member().verified().role("admin").create().await
}

async fn views_of(env: &Env, visitor: &str) -> Vec<(Option<i64>, String, Option<String>)> {
    sqlx::query_as(
        "SELECT user_id, path, referrer FROM site.page_views WHERE visitor_id = $1::uuid ORDER BY id",
    )
    .bind(visitor)
    .fetch_all(&env.db)
    .await
    .unwrap()
}

async fn events_of(env: &Env, user_id: i64) -> Vec<(String, Value)> {
    sqlx::query_as("SELECT kind, detail FROM site.security_events WHERE user_id = $1 ORDER BY id")
        .bind(user_id)
        .fetch_all(&env.db)
        .await
        .unwrap()
}

// --- Beacons ------------------------------------------------------------------

#[tokio::test]
async fn a_page_view_needs_a_visitor_or_a_member() {
    let Some(env) = Env::new().await else { return };
    let path = format!("/cours/{}", unique());

    // Without consent the site sends no visitor id: nothing is written.
    let (status, headers, body) = Call::post("/v1/telemetry/page-view")
        .json(json!({"path": path, "referrer": null}))
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let written: i64 = sqlx::query_scalar("SELECT count(*) FROM site.page_views WHERE path = $1")
        .bind(&path)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(written, 0);
    // A malformed id is no id.
    Call::post("/v1/telemetry/page-view")
        .visitor("not-a-uuid")
        .json(json!({"path": path}))
        .send(&env.app)
        .await;
    Call::post("/v1/telemetry/page-view")
        .visitor(&Uuid::now_v7().to_string())
        .json(json!({"path": path}))
        .send(&env.app)
        .await;
    let written: i64 = sqlx::query_scalar("SELECT count(*) FROM site.page_views WHERE path = $1")
        .bind(&path)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(written, 0);

    // With one: the path without its query, the referrer's origin only.
    let vid = visitor();
    let (status, body) = Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .json(json!({"path": format!("{path}?tab=2#ex"), "referrer": "https://www.qwant.com/?q=pulsars"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .json(json!({"path": "/forum", "referrer": "https://pulsars.science/cours"}))
        .send(&env.app)
        .await;
    // A robot is not counted.
    Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .user_agent("Googlebot/2.1")
        .json(json!({"path": "/robots"}))
        .send(&env.app)
        .await;
    assert_eq!(
        views_of(&env, &vid).await,
        [
            (None, path.clone(), Some("https://www.qwant.com".to_owned())),
            (None, "/forum".to_owned(), None),
        ]
    );

    // A member's own, without a visitor id.
    let ada = env.member().verified().create().await;
    Call::post("/v1/telemetry/page-view")
        .member(&ada)
        .json(json!({"path": "/tableau-de-bord"}))
        .send(&env.app)
        .await;
    let mine: Vec<String> =
        sqlx::query_scalar("SELECT path FROM site.page_views WHERE user_id = $1")
            .bind(ada.id)
            .fetch_all(&env.db)
            .await
            .unwrap();
    assert_eq!(mine, ["/tableau-de-bord"]);

    // Not a path of the site, or unknown fields.
    for body in [
        json!({"path": "https://evil.example/"}),
        json!({"path": "//evil.example/x"}),
        json!({"path": format!("/{}", "a".repeat(600))}),
    ] {
        let (status, body) = Call::post("/v1/telemetry/page-view")
            .visitor(&vid)
            .json(body)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (status, _) = Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .json(json!({"path": "/", "title": "x"}))
        .send(&env.app)
        .await;
    assert!(status.is_client_error());
    assert_eq!(views_of(&env, &vid).await.len(), 2);
}

#[tokio::test]
async fn consents_are_kept_as_proofs() {
    let Some(env) = Env::new().await else { return };
    let vid = visitor();
    let version = i64::from(rand_version());
    let (status, _) = Call::post("/v1/consent")
        .visitor(&vid)
        .json(json!({"measure": true, "version": version}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // A refusal without a visitor id nor a member is kept too.
    let (status, _) = Call::post("/v1/consent")
        .json(json!({"measure": false, "version": version}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let rows: Vec<ConsentRow> = sqlx::query_as(
        "SELECT visitor_id, user_id, measure, ip_hash FROM site.consents WHERE version = $1 ORDER BY id",
    )
    .bind(i32::try_from(version).unwrap())
    .fetch_all(&env.db)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0.map(|u| u.to_string()), Some(vid));
    assert!(rows[0].2);
    assert!(!rows[1].2);
    assert_eq!(rows[1].0, None);
    // The fingerprint, never the address.
    assert_eq!(rows[0].3.as_deref().map(str::len), Some(64));
    let (status, _) = Call::post("/v1/consent")
        .json(json!({"measure": true, "version": 0}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

fn rand_version() -> i32 {
    // A version no other run uses, to find this test's rows.
    i32::try_from(Uuid::new_v4().as_u128() % 900_000).unwrap() + 100_000
}

// --- Security events and linking ---------------------------------------------

#[tokio::test]
async fn logging_in_records_events_and_links_the_visitor() {
    let Some(env) = Env::new().await else { return };
    let ada = env
        .member()
        .verified()
        .password("correct horse 1")
        .create()
        .await;
    let vid = visitor();
    Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .json(json!({"path": "/connexion"}))
        .send(&env.app)
        .await;

    let (status, _) = Call::post("/v1/auth/login")
        .service()
        .visitor(&vid)
        .json(json!({"login": ada.pseudo, "password": "wrong password"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // A failed login does not link anything.
    assert_eq!(views_of(&env, &vid).await[0].0, None);
    let (status, body) = Call::post("/v1/auth/login")
        .service()
        .visitor(&vid)
        .user_agent("Firefox/140")
        .ip("2001:db8::77")
        .json(json!({"login": ada.pseudo, "password": "correct horse 1"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["data"]["session"]["token"]
        .as_str()
        .unwrap()
        .to_owned();
    // The guest's history is the member's now.
    assert_eq!(views_of(&env, &vid).await[0].0, Some(ada.id));

    let (status, body) = Call::put("/v1/auth/password")
        .session(&token)
        .json(
            json!({"current_password": "correct horse 1", "password": "correct horse 2",
                     "password_confirmation": "correct horse 2"}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    Call::post("/v1/auth/logout")
        .session(&token)
        .send(&env.app)
        .await;

    let events = events_of(&env, ada.id).await;
    let kinds: Vec<&str> = events.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        kinds,
        ["login_failed", "login", "password_changed", "logout"]
    );
    // The member's earlier session (made by the test harness) was revoked.
    assert_eq!(events[2].1, json!({"sessions_revoked": 1}));
    let (ip, agent): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT ip, user_agent FROM site.security_events WHERE user_id = $1 AND kind = 'login'",
    )
    .bind(ada.id)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(ip.as_deref(), Some("2001:db8::77"));
    assert_eq!(agent.as_deref(), Some("Firefox/140"));

    // An unknown account: no user, the identifier typed.
    let nobody = format!("nobody-{}", unique());
    Call::post("/v1/auth/login")
        .service()
        .json(json!({"login": nobody, "password": "whatever 123"}))
        .send(&env.app)
        .await;
    let detail: Value = sqlx::query_scalar(
        "SELECT detail FROM site.security_events
          WHERE user_id IS NULL AND kind = 'login_failed' AND detail ->> 'identifier' = $1",
    )
    .bind(&nobody)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(detail, json!({"identifier": nobody}));
}

// --- Purge ------------------------------------------------------------------

#[tokio::test]
async fn the_purge_keeps_each_table_for_its_retention() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().create().await;
    let vid = Uuid::new_v4();
    let at = |interval: &str| format!("now() - interval '{interval}'");
    for (age, path) in [("13 months 1 day", "/old"), ("12 months 25 days", "/kept")] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO site.page_views (at, visitor_id, path) VALUES ({}, $1, $2)",
            at(age)
        )))
        .bind(vid)
        .bind(path)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    for (age, kind) in [("12 months 1 day", "login"), ("11 months", "logout")] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO site.security_events (at, user_id, kind) VALUES ({}, $1, $2)",
            at(age)
        )))
        .bind(ada.id)
        .bind(kind)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    for (age, measure) in [("25 months 1 day", false), ("24 months", true)] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO site.consents (at, visitor_id, measure, version) VALUES ({}, $1, $2, 1)",
            at(age)
        )))
        .bind(vid)
        .bind(measure)
        .execute(&env.owner)
        .await
        .unwrap();
    }

    pulsars_api::journal::purge(&env.db).await.unwrap();

    let paths: Vec<String> =
        sqlx::query_scalar("SELECT path FROM site.page_views WHERE visitor_id = $1")
            .bind(vid)
            .fetch_all(&env.db)
            .await
            .unwrap();
    assert_eq!(paths, ["/kept"]);
    let kinds: Vec<String> = events_of(&env, ada.id)
        .await
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(kinds, ["logout"]);
    let consents: Vec<bool> =
        sqlx::query_scalar("SELECT measure FROM site.consents WHERE visitor_id = $1")
            .bind(vid)
            .fetch_all(&env.db)
            .await
            .unwrap();
    assert_eq!(consents, [true]);
}

// --- Admin -------------------------------------------------------------------

#[tokio::test]
async fn the_journal_is_for_admins_only() {
    let Some(env) = Env::new().await else { return };
    let member = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let admin = admin(&env).await;
    let vid = visitor();
    Call::post("/v1/telemetry/page-view")
        .visitor(&vid)
        .json(json!({"path": "/"}))
        .send(&env.app)
        .await;
    for path in [
        "/v1/admin/people?q=ab".to_owned(),
        format!("/v1/admin/people/members/{}", member.id),
        format!("/v1/admin/people/members/{}/events", member.id),
        format!("/v1/admin/people/visitors/{vid}"),
        format!("/v1/admin/people/visitors/{vid}/events"),
        "/v1/admin/activity".to_owned(),
    ] {
        let (status, _) = Call::get(&path).send(&env.app).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        for who in [&member, &moderator] {
            let (status, _) = Call::get(&path).member(who).send(&env.app).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
        let (status, headers, body) = Call::get(&path).member(&admin).send_full(&env.app).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store", "{path}");
    }
}

#[tokio::test]
async fn people_are_found_by_email_pseudo_or_visitor_id() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let ada = env.member().verified().create().await;
    let get = |path: String| {
        let app = env.app.clone();
        let admin = admin.clone();
        async move {
            let (status, body) = Call::get(path).member(&admin).send(&app).await;
            (status, body)
        }
    };
    // Part of the pseudo, in another case.
    let part = ada.pseudo[2..8].to_uppercase();
    let (status, body) = get(format!("/v1/admin/people?q={part}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found = &body["data"]["members"];
    assert_eq!(found.as_array().unwrap().len(), 1, "{body}");
    assert_eq!(found[0]["id"], ada.id);
    assert_eq!(found[0]["pseudo"], ada.pseudo);
    assert_eq!(found[0]["email"], ada.email);
    assert_eq!(found[0]["verified"], true);
    assert_eq!(found[0]["roles"], json!([]));
    // The start of the e-mail address.
    let (_, body) = get(format!("/v1/admin/people?q={}", &ada.email[..12])).await;
    assert!(
        body["data"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == ada.id)
    );
    // `%` and `_` are literal.
    let (_, body) = get("/v1/admin/people?q=%25%25".to_owned()).await;
    assert_eq!(body["data"]["members"], json!([]));

    // A visitor id.
    let vid = visitor();
    for path in ["/", "/cours"] {
        Call::post("/v1/telemetry/page-view")
            .visitor(&vid)
            .json(json!({"path": path}))
            .send(&env.app)
            .await;
    }
    let (_, body) = get(format!("/v1/admin/people?q={vid}")).await;
    let visitors = &body["data"]["visitors"];
    assert_eq!(visitors[0]["visitor_id"], vid);
    assert_eq!(visitors[0]["page_views"], 2);
    assert_eq!(visitors[0]["member_id"], Value::Null);

    let (status, body) = get("/v1/admin/people?q=a".to_owned()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["errors"][0]["parameter"], "q");
}

/// A room of `owner` with a message whose content must never show.
async fn room_message(env: &Env, owner: i64, secret: &str) {
    let slug = format!("salon-{}", unique());
    let room: i64 = sqlx::query_scalar(
        "INSERT INTO site.rooms (nom, slug, type, created_by, created_at, updated_at)
         VALUES ('Salon des tests', $1, 'public', $2, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(&slug)
    .bind(owner)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO site.room_messages (room_id, user_id, contenu, created_at)
         VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC' - interval '2 minutes')",
    )
    .bind(room)
    .bind(owner)
    .bind(secret)
    .execute(&env.owner)
    .await
    .unwrap();
}

#[tokio::test]
async fn a_member_has_a_summary_and_a_timeline() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let ada = env.member().verified().create().await;
    let vid = Uuid::new_v4();
    // Two page views a minute apart, linked, and one more as a member.
    for (minutes, path) in [(10, "/cours"), (9, "/forum")] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO site.page_views (at, user_id, visitor_id, path)
             VALUES (now() - interval '{minutes} minutes', $1, $2, $3)"
        )))
        .bind(ada.id)
        .bind(vid)
        .bind(path)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO site.security_events (at, user_id, kind, ip) VALUES (now() - interval '5 minutes', $1, 'login', '2001:db8::1')",
    )
    .bind(ada.id)
    .execute(&env.owner)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO site.consents (at, user_id, visitor_id, measure, version)
         VALUES (now() - interval '11 minutes', $1, $2, true, 3)",
    )
    .bind(ada.id)
    .bind(vid)
    .execute(&env.owner)
    .await
    .unwrap();
    let secret = format!("secret-{}", unique());
    room_message(&env, ada.id, &secret).await;

    let (status, body) = Call::get(format!("/v1/admin/people/members/{}", ada.id))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["member"]["id"], ada.id);
    assert_eq!(data["member"]["onboarded"], true);
    assert_eq!(data["member"]["visitor_ids"], json!([vid.to_string()]));
    assert!(data["member"]["last_seen_at"].is_string());
    assert_eq!(
        data["counts"],
        json!({"page_views": 2, "lessons_completed": 0, "exercise_attempts": 0, "games": 0,
               "forum_posts": 0, "room_messages": 1, "downloads": 0, "logins": 1})
    );
    assert_eq!(data["consent"]["measure"], true);
    let (status, _) = Call::get("/v1/admin/people/members/999999999")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The timeline, two by two, newest first.
    let mut items = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let path = match &cursor {
            Some(c) => format!(
                "/v1/admin/people/members/{}/events?limit=2&cursor={c}",
                ada.id
            ),
            None => format!("/v1/admin/people/members/{}/events?limit=2", ada.id),
        };
        let (status, body) = Call::get(path).member(&admin).send(&env.app).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!body.to_string().contains(&secret), "{body}");
        assert!(body["data"].as_array().unwrap().len() <= 2);
        items.extend(body["data"].as_array().unwrap().iter().cloned());
        match body["next_cursor"].as_str() {
            Some(c) => cursor = Some(c.to_owned()),
            None => break,
        }
    }
    let kinds: Vec<&str> = items.iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "signup",
            "room_message",
            "security",
            "page_view",
            "page_view",
            "consent"
        ]
    );
    let ats: Vec<&str> = items.iter().map(|i| i["at"].as_str().unwrap()).collect();
    assert!(ats.windows(2).all(|w| w[0] >= w[1]), "{ats:?}");
    assert_eq!(items[1]["data"], json!({"room_name": "Salon des tests"}));
    assert!(
        items[1]["link"]
            .as_str()
            .unwrap()
            .starts_with("/salons/salon-")
    );
    assert_eq!(items[2]["data"]["event"], "login");
    assert_eq!(items[2]["data"]["ip"], "2001:db8::1");
    assert_eq!(
        items[3]["data"],
        json!({"path": "/forum", "referrer": null})
    );
    assert_eq!(items[3]["link"], "/forum");
    assert!(items[3]["id"].as_str().unwrap().starts_with("page_view:"));
    assert!(items[0].get("who").is_none());

    // Some kinds only; an unknown one is refused.
    let (_, body) = Call::get(format!(
        "/v1/admin/people/members/{}/events?kinds=page_view,consent",
        ada.id
    ))
    .member(&admin)
    .send(&env.app)
    .await;
    let kinds: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["page_view", "page_view", "consent"]);
    let (status, body) = Call::get(format!(
        "/v1/admin/people/members/{}/events?kinds=page_view,nope",
        ada.id
    ))
    .member(&admin)
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["errors"][0]["parameter"], "kinds");
    // A cursor of other filters is refused.
    let (status, _) = Call::get(format!(
        "/v1/admin/people/members/{}/events?kinds=signup&cursor={}",
        ada.id,
        cursor_of_first_page(&env, &admin, ada.id).await
    ))
    .member(&admin)
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The visitor's own timeline, and their summary.
    let (_, body) = Call::get(format!("/v1/admin/people/visitors/{vid}/events"))
        .member(&admin)
        .send(&env.app)
        .await;
    let kinds: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["page_view", "page_view", "consent"]);
    let (_, body) = Call::get(format!("/v1/admin/people/visitors/{vid}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["visitor"]["member_id"], ada.id);
    assert_eq!(body["data"]["counts"]["page_views"], 2);
    assert_eq!(body["data"]["consent"]["measure"], true);
    let (status, _) = Call::get(format!("/v1/admin/people/visitors/{}", Uuid::new_v4()))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Everybody's activity says who.
    let (_, body) = Call::get("/v1/admin/activity?kinds=room_message&limit=100")
        .member(&admin)
        .send(&env.app)
        .await;
    assert!(!body.to_string().contains(&secret));
    let ours = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["who"]["member_id"] == ada.id)
        .unwrap();
    assert_eq!(ours["who"]["pseudo"], ada.pseudo);
}

async fn cursor_of_first_page(env: &Env, admin: &TestMember, id: i64) -> String {
    let (_, body) = Call::get(format!("/v1/admin/people/members/{id}/events?limit=1"))
        .member(admin)
        .send(&env.app)
        .await;
    body["next_cursor"].as_str().unwrap().to_owned()
}
