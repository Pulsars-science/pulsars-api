//! The forum, its moderation and the translation of its posts
//! (`src/routes/forum.rs`, `src/routes/forum_moderation.rs`), ported from
//! Laravel's `ForumTest`, `ModerationForumTest` and `TraductionForumTest`.
//!
//! Each test plants its own theme, so lists only show its questions. DeepL
//! is a local fake server, never the real service.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use common::{Call, Env, TestMember, codes, fresh_ip, unique};
use serde_json::{Value, json};

/// A theme of its own, `(id, slug)`.
async fn theme(env: &Env) -> (i64, String) {
    let slug = format!("t-{}", unique());
    let id = sqlx::query_scalar(
        "INSERT INTO site.forum_themes (nom, slug, couleur, ordre) VALUES ($1, $1, '#4f7df0', 99)
         RETURNING id",
    )
    .bind(&slug)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    (id, slug)
}

struct Question {
    id: i64,
    slug: String,
}

/// A question in `theme`, by `author`, as Laravel's tests create them.
async fn question(env: &Env, theme: i64, author: &TestMember) -> Question {
    let slug = format!("question-test-{}", unique());
    let id = sqlx::query_scalar(
        "INSERT INTO site.questions (forum_theme_id, user_id, titre, slug, corps, created_at, updated_at)
         VALUES ($1, $2, 'Une question de test bien formulee', $3,
                 'Le corps original de la question, bien visible.', now(), now())
         RETURNING id",
    )
    .bind(theme)
    .bind(author.id)
    .bind(&slug)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    Question { id, slug }
}

async fn answer(env: &Env, question: i64, author: &TestMember, parent: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO site.answers (question_id, user_id, parent_id, corps, created_at, updated_at)
         VALUES ($1, $2, $3, 'Une reponse de test.', now(), now()) RETURNING id",
    )
    .bind(question)
    .bind(author.id)
    .bind(parent)
    .fetch_one(&env.owner)
    .await
    .unwrap()
}

async fn count(env: &Env, sql: &str, id: i64) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(id)
        .fetch_one(&env.owner)
        .await
        .unwrap()
}

fn list(theme: &str, query: &str) -> String {
    format!("/v1/forum/questions?theme={theme}{query}")
}

// --- Reading ------------------------------------------------------------------

#[tokio::test]
async fn the_forum_is_readable_without_an_account() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, slug) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;

    let (status, body) = Call::get("/v1/forum/themes").send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mine = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["slug"] == slug.as_str())
        .unwrap();
    assert_eq!(mine["question_count"], 1);
    // The reference themes are there too, in their order.
    assert_eq!(body["data"][0]["slug"], "mathematiques");

    let (status, body) = Call::get(format!("/v1/forum/themes/{slug}"))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["color"], "#4f7df0");

    let (status, headers, body) = Call::get(list(&slug, "")).send_full(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(body["total"], 1);
    let item = &body["data"][0];
    assert_eq!(item["slug"], q.slug.as_str());
    assert_eq!(item["author"], ada.pseudo.as_str());
    assert_eq!(item["author_id"], ada.id.to_string());
    assert_eq!(
        item["excerpt"],
        "Le corps original de la question, bien visible."
    );
    assert_eq!(item["has_voted"], false);
    assert_eq!(item["solved"], false);

    let (status, body) = Call::get(format!("/v1/forum/questions/{}", q.slug))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(
        data["body_md"],
        "Le corps original de la question, bien visible."
    );
    assert_eq!(data["theme"]["slug"], slug.as_str());
    assert_eq!(data["viewer"]["can_participate"], false);
    assert_eq!(data["viewer"]["is_author"], false);
    assert_eq!(data["answers"], json!([]));

    for path in [
        "/v1/forum/themes/nope".to_owned(),
        "/v1/forum/questions?theme=nope".to_owned(),
        "/v1/forum/questions/nope".to_owned(),
    ] {
        let (status, _) = Call::get(&path).send(&env.app).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, body) = Call::get(list(&slug, "&sort=best")).send(&env.app).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn views_count_once_per_reader() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let path = format!("/v1/forum/questions/{}", q.slug);

    let ip = fresh_ip();
    let (_, body) = Call::get(&path).ip(&ip).send(&env.app).await;
    assert_eq!(body["data"]["views"], 1);
    // Reloading does not count again.
    let (_, body) = Call::get(&path).ip(&ip).send(&env.app).await;
    assert_eq!(body["data"]["views"], 1);
    // Another reader does.
    let (_, body) = Call::get(&path).member(&ada).send(&env.app).await;
    assert_eq!(body["data"]["views"], 2);
    let (_, body) = Call::get(&path).member(&ada).send(&env.app).await;
    assert_eq!(body["data"]["views"], 2);
    assert_eq!(body["data"]["viewer"]["is_author"], true);
    assert_eq!(body["data"]["viewer"]["can_participate"], true);
}

#[tokio::test]
async fn answers_come_as_a_tree_with_votes_and_the_accepted_answer() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let (theme_id, slug) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let first = answer(&env, q.id, &bob, None).await;
    let reply = answer(&env, q.id, &ada, Some(first)).await;
    let second = answer(&env, q.id, &bob, None).await;

    let (status, body) = Call::post(format!("/v1/forum/answers/{reply}/vote"))
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = Call::post(format!(
        "/v1/forum/questions/{}/answers/{second}/accept",
        q.slug
    ))
    .member(&ada)
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = Call::get(format!("/v1/forum/questions/{}", q.slug))
        .member(&ada)
        .send(&env.app)
        .await;
    let data = &body["data"];
    assert_eq!(data["answer_count"], 3);
    assert_eq!(data["accepted_answer_id"], second);
    let answers = data["answers"].as_array().unwrap();
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0]["id"], first);
    assert_eq!(answers[0]["children"][0]["id"], reply);
    assert_eq!(answers[0]["children"][0]["votes"], 1);
    assert_eq!(answers[0]["children"][0]["has_voted"], true);
    assert_eq!(answers[1]["id"], second);
    assert_eq!(answers[1]["has_voted"], false);

    let (_, body) = Call::get(list(&slug, "")).send(&env.app).await;
    assert_eq!(body["data"][0]["answer_count"], 3);
    assert_eq!(body["data"][0]["solved"], true);
}

#[tokio::test]
async fn lists_sort_pin_and_paginate() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let (theme_id, slug) = theme(&env).await;
    let old = question(&env, theme_id, &ada).await;
    let liked = question(&env, theme_id, &ada).await;
    let answered = question(&env, theme_id, &ada).await;
    let newest = question(&env, theme_id, &ada).await;
    // From the oldest to the newest.
    for (i, q) in [&old, &liked, &answered, &newest].iter().enumerate() {
        sqlx::query(
            "UPDATE site.questions SET created_at = now() - make_interval(days => $2) WHERE id = $1",
        )
        .bind(q.id)
        .bind(10 - i as i32)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    answer(&env, answered.id, &bob, None).await;
    for voter in [&ada, &bob] {
        Call::post(format!("/v1/forum/questions/{}/vote", liked.id))
            .member(voter)
            .send(&env.app)
            .await;
    }
    let moderator = env.member().verified().role("moderateur").create().await;
    let (status, body) = Call::post(format!("/v1/moderation/forum/questions/{}/pin", old.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["pinned"], true);

    let slugs = |body: &Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| q["slug"].as_str().unwrap().to_owned())
            .collect()
    };
    let (_, body) = Call::get(list(&slug, "")).send(&env.app).await;
    assert_eq!(
        slugs(&body),
        [&old, &newest, &answered, &liked].map(|q| q.slug.clone())
    );
    assert_eq!(body["data"][0]["is_pinned"], true);
    let (_, body) = Call::get(list(&slug, "&sort=popular")).send(&env.app).await;
    assert_eq!(
        slugs(&body),
        [&old, &liked, &newest, &answered].map(|q| q.slug.clone())
    );
    assert_eq!(body["data"][1]["votes"], 2);
    let (_, body) = Call::get(list(&slug, "&sort=unanswered"))
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 3);
    assert!(!slugs(&body).contains(&answered.slug));

    let (_, body) = Call::get(list(&slug, "&limit=3&page=2"))
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 4);
    assert_eq!(slugs(&body), std::slice::from_ref(&liked.slug));
    let (_, body) = Call::get(list(&slug, "&limit=3&page=9"))
        .send(&env.app)
        .await;
    assert_eq!(
        (body["total"].clone(), body["data"].clone()),
        (json!(4), json!([]))
    );
}

// --- Asking and answering -------------------------------------------------------

#[tokio::test]
async fn asking_needs_a_verified_member() {
    let Some(env) = Env::new().await else { return };
    let (_, slug) = theme(&env).await;
    let question = json!({
        "theme": slug,
        "title": "Comment resoudre cette equation ?",
        "body": "Voici mon probleme detaille en quelques mots.",
    });
    let (status, _) = Call::post("/v1/forum/questions")
        .json(question.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let unverified = env.member().create().await;
    let (status, _) = Call::post("/v1/forum/questions")
        .member(&unverified)
        .json(question.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let ada = env.member().verified().create().await;
    let (status, body) = Call::post("/v1/forum/questions")
        .member(&ada)
        .json(json!({
            "theme": slug,
            "title": "  Comment factoriser un trinôme ?  ",
            "body": "Je cherche la methode generale de factorisation.",
        }))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let new_slug = body["data"]["slug"].as_str().unwrap().to_owned();
    let (base, suffix) = new_slug.rsplit_once('-').unwrap();
    assert_eq!(base, "comment-factoriser-un-trinome");
    assert_eq!(suffix.len(), 6);
    let (title, user_id): (String, i64) =
        sqlx::query_as("SELECT titre, user_id FROM site.questions WHERE id = $1")
            .bind(body["data"]["id"].as_i64().unwrap())
            .fetch_one(&env.owner)
            .await
            .unwrap();
    assert_eq!(
        (title.as_str(), user_id),
        ("Comment factoriser un trinôme ?", ada.id)
    );
}

#[tokio::test]
async fn questions_follow_laravels_rules() {
    let Some(env) = Env::new().await else { return };
    let (_, slug) = theme(&env).await;
    let ada = env.member().verified().create().await;
    let ask = |body: Value| Call::post("/v1/forum/questions").member(&ada).json(body);

    let (status, body) = ask(json!({
        "theme": slug, "title": "Court", "body": "Un corps de question suffisamment long."
    }))
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(codes(&body), [("title".to_owned(), "too_short".to_owned())]);

    let (_, body) = ask(json!({})).send(&env.app).await;
    assert_eq!(
        codes(&body),
        [
            ("theme".to_owned(), "required".to_owned()),
            ("title".to_owned(), "required".to_owned()),
            ("body".to_owned(), "required".to_owned()),
        ]
    );
    let (_, body) = ask(json!({
        "theme": "nope", "title": "x".repeat(151), "body": "x".repeat(10_001)
    }))
    .send(&env.app)
    .await;
    assert_eq!(
        codes(&body),
        [
            ("theme".to_owned(), "invalid".to_owned()),
            ("title".to_owned(), "too_long".to_owned()),
            ("body".to_owned(), "too_long".to_owned()),
        ]
    );
    let (status, _) = ask(json!({"theme": slug, "admin": true}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn members_answer_and_reply_within_the_question() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q1 = question(&env, theme_id, &ada).await;
    let q2 = question(&env, theme_id, &ada).await;
    let reply = |slug: &str, body: Value| {
        Call::post(format!("/v1/forum/questions/{slug}/answers"))
            .member(&bob)
            .json(body)
    };

    let (status, body) = reply(&q1.slug, json!({"body": "Voici ma reponse a ta question."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let parent = body["data"]["id"].as_i64().unwrap();

    let (status, body) = reply(
        &q1.slug,
        json!({"body": "Reponse a la reponse.", "parent_id": parent}),
    )
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.answers WHERE parent_id = $1",
            parent
        )
        .await,
        1
    );

    // A reply to another question's answer is refused.
    let (status, body) = reply(
        &q2.slug,
        json!({"body": "Tentative de greffe.", "parent_id": parent}),
    )
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        codes(&body),
        [("parent_id".to_owned(), "invalid".to_owned())]
    );
    let (_, body) = reply(&q2.slug, json!({"body": "x"})).send(&env.app).await;
    assert_eq!(codes(&body), [("body".to_owned(), "too_short".to_owned())]);
    let (status, _) = reply("nope", json!({"body": "Une reponse."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_the_author_accepts_an_answer() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let bob = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let other = question(&env, theme_id, &ada).await;
    let good = answer(&env, q.id, &bob, None).await;
    let elsewhere = answer(&env, other.id, &bob, None).await;
    let accept = |id: i64| format!("/v1/forum/questions/{}/answers/{id}/accept", q.slug);

    let (status, body) = Call::post(accept(good)).member(&ada).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["accepted_answer_id"], good);
    // Again: the mark is taken back.
    let (_, body) = Call::post(accept(good)).member(&ada).send(&env.app).await;
    assert_eq!(body["data"]["accepted_answer_id"], Value::Null);

    let (status, _) = Call::post(accept(good)).member(&bob).send(&env.app).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::post(accept(elsewhere))
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = Call::post(accept(0)).member(&ada).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_vote_toggles_and_counts_once() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    // By id or by slug.
    let by_id = format!("/v1/forum/questions/{}/vote", q.id);
    let by_slug = format!("/v1/forum/questions/{}/vote", q.slug);

    let (status, body) = Call::post(&by_id).member(&ada).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], json!({"voted": true, "votes": 1}));
    let (_, body) = Call::post(&by_slug).member(&ada).send(&env.app).await;
    assert_eq!(body["data"], json!({"voted": false, "votes": 0}));
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.forum_votes WHERE votable_id = $1 AND votable_type = 'question'",
            q.id
        )
        .await,
        0
    );

    let (status, _) = Call::post(format!("/v1/forum/answers/{}/vote", i64::MAX))
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = Call::post("/v1/forum/questions/nope/vote")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = Call::post(&by_id).send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// --- Reports ----------------------------------------------------------------------

#[tokio::test]
async fn members_report_forum_messages() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let a = answer(&env, q.id, &ada, None).await;

    let (status, body) = Call::post("/v1/forum/reports")
        .member(&ada)
        .json(json!({"target_type": "question", "target_id": q.id, "type": "spam"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (kind, target, message, state): (String, i64, String, String) = sqlx::query_as(
        "SELECT signalable_type, signalable_id, message, statut FROM site.signalements WHERE id = $1",
    )
    .bind(body["data"]["id"].as_i64().unwrap())
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), target, message.as_str(), state.as_str()),
        ("question", q.id, "", "nouveau")
    );

    let (status, _) = Call::post("/v1/forum/reports")
        .member(&ada)
        .json(
            json!({"target_type": "answer", "target_id": a, "type": "hors_sujet",
                     "message": "Rien a voir."}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = Call::post("/v1/forum/reports")
        .json(json!({"target_type": "question", "target_id": q.id, "type": "spam"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = Call::post("/v1/forum/reports")
        .member(&ada)
        .json(
            json!({"target_type": "question", "target_id": q.id, "type": "faute",
                     "message": "x".repeat(2001)}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("type".to_owned(), "invalid".to_owned()),
            ("message".to_owned(), "too_long".to_owned()),
        ]
    );

    // A deleted question cannot be reported.
    sqlx::query("UPDATE site.questions SET deleted_at = now() WHERE id = $1")
        .bind(q.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = Call::post("/v1/forum/reports")
        .member(&ada)
        .json(json!({"target_type": "question", "target_id": q.id, "type": "abus"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reports_are_limited_to_twenty_a_minute() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    for _ in 0..20 {
        let (status, _) = Call::post("/v1/forum/reports")
            .member(&ada)
            .json(json!({}))
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (status, headers, _) = Call::post("/v1/forum/reports")
        .member(&ada)
        .json(json!({}))
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(headers.contains_key("retry-after"));
}

// --- Moderation -------------------------------------------------------------------

#[tokio::test]
async fn only_moderators_moderate() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let (status, _) = Call::post(format!("/v1/moderation/forum/questions/{}/hide", q.id))
        .member(&ada)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::delete(format!("/v1/moderation/forum/questions/{}", q.id))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.questions WHERE id = $1 AND masque_le IS NULL AND deleted_at IS NULL",
            q.id
        )
        .await,
        1
    );
}

#[tokio::test]
async fn a_hidden_body_is_kept_from_non_moderators() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let (theme_id, slug) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let a = answer(&env, q.id, &ada, None).await;

    let (status, body) = Call::post(format!("/v1/moderation/forum/questions/{}/hide", q.id))
        .member(&moderator)
        .json(json!({"reason": "Spam evident"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["hidden"], true);
    let by: i64 = sqlx::query_scalar("SELECT masque_par FROM site.questions WHERE id = $1")
        .bind(q.id)
        .fetch_one(&env.owner)
        .await
        .unwrap();
    assert_eq!(by, moderator.id);
    let (_, body) = Call::post(format!("/v1/moderation/forum/answers/{a}/hide"))
        .member(&moderator)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["hidden"], true);

    let path = format!("/v1/forum/questions/{}", q.slug);
    let (_, body) = Call::get(&path).send(&env.app).await;
    let data = &body["data"];
    assert_eq!(data["is_hidden"], true);
    assert_eq!(data["hidden_reason"], "Spam evident");
    assert_eq!(data["body_md"], Value::Null);
    assert_eq!(data["title"], "Une question de test bien formulee");
    assert_eq!(data["answers"][0]["body_md"], Value::Null);
    let (_, body) = Call::get(list(&slug, "")).member(&ada).send(&env.app).await;
    assert_eq!(body["data"][0]["excerpt"], Value::Null);

    // A moderator reads the real body, to judge.
    let (_, body) = Call::get(&path).member(&moderator).send(&env.app).await;
    assert_eq!(
        body["data"]["body_md"],
        "Le corps original de la question, bien visible."
    );
    assert_eq!(
        body["data"]["answers"][0]["body_md"],
        "Une reponse de test."
    );
    assert_eq!(body["data"]["viewer"]["is_moderator"], true);

    // Hiding again shows it again.
    let (_, body) = Call::post(format!("/v1/moderation/forum/questions/{}/hide", q.id))
        .member(&moderator)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["hidden"], false);
    let (_, body) = Call::get(&path).send(&env.app).await;
    assert_eq!(body["data"]["hidden_reason"], Value::Null);
    assert!(body["data"]["body_md"].is_string());

    let (status, body) = Call::post(format!("/v1/moderation/forum/questions/{}/hide", q.id))
        .member(&moderator)
        .json(json!({"reason": "x".repeat(301)}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("reason".to_owned(), "too_long".to_owned())]);
}

#[tokio::test]
async fn deleting_then_restoring_messages() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let a = answer(&env, q.id, &ada, None).await;
    let path = format!("/v1/forum/questions/{}", q.slug);

    let (status, _) = Call::delete(format!("/v1/moderation/forum/answers/{a}"))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = Call::get(&path).send(&env.app).await;
    assert_eq!(body["data"]["answer_count"], 0);
    let (status, _) = Call::post(format!("/v1/moderation/forum/answers/{a}/restore"))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = Call::get(&path).send(&env.app).await;
    assert_eq!(body["data"]["answer_count"], 1);

    let (status, _) = Call::delete(format!("/v1/moderation/forum/questions/{}", q.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // A soft delete: the page answers 404, the row stays.
    let (status, _) = Call::get(&path).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.questions WHERE id = $1 AND deleted_at IS NOT NULL",
            q.id
        )
        .await,
        1
    );
    let (status, _) = Call::delete(format!("/v1/moderation/forum/questions/{}", q.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = Call::post(format!("/v1/moderation/forum/questions/{}/restore", q.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = Call::get(&path).send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = Call::post("/v1/moderation/forum/questions/0/restore")
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_locked_thread_takes_no_more_answers() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;

    let (status, body) = Call::post(format!("/v1/moderation/forum/questions/{}/lock", q.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["locked"], true);

    let (status, body) = Call::post(format!("/v1/forum/questions/{}/answers", q.slug))
        .member(&ada)
        .json(json!({"body": "Ma réponse tardive."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["reason"], "locked");
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.answers WHERE question_id = $1",
            q.id
        )
        .await,
        0
    );
    let (_, body) = Call::get(format!("/v1/forum/questions/{}", q.slug))
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["is_locked"], true);
    assert_eq!(body["data"]["viewer"]["can_participate"], false);

    let (_, body) = Call::post(format!("/v1/moderation/forum/questions/{}/lock", q.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["locked"], false);
}

#[tokio::test]
async fn a_muted_member_cannot_post_until_unmuted() {
    let Some(env) = Env::new().await else { return };
    let moderator = env.member().verified().role("moderateur").create().await;
    let offender = env.member().verified().create().await;
    let (theme_id, slug) = theme(&env).await;
    let q = question(&env, theme_id, &moderator).await;

    let (status, body) = Call::put(format!("/v1/moderation/members/{}/mute", offender.id))
        .member(&moderator)
        .json(json!({"days": 7, "reason": "Comportement abusif repete."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["user_id"], offender.id.to_string());
    let (by, days): (i64, f64) = sqlx::query_as(
        "SELECT mute_par, extract(epoch FROM mute_jusqu_a - now() AT TIME ZONE 'UTC')::float8 / 86400
           FROM site.users WHERE id = $1",
    )
    .bind(offender.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(by, moderator.id);
    assert!((6.99..=7.0).contains(&days), "{days}");

    let ask = json!({
        "theme": slug,
        "title": "Une question pendant la suspension",
        "body": "Un corps assez long pour passer la validation.",
    });
    let (status, _) = Call::post("/v1/forum/questions")
        .member(&offender)
        .json(ask.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::post(format!("/v1/forum/questions/{}/answers", q.slug))
        .member(&offender)
        .json(json!({"body": "Une reponse."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, body) = Call::get(format!("/v1/forum/questions/{}", q.slug))
        .member(&offender)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["viewer"]["can_participate"], false);

    let (status, _) = Call::delete(format!("/v1/moderation/members/{}/mute", offender.id))
        .member(&moderator)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = Call::post("/v1/forum/questions")
        .member(&offender)
        .json(ask)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn mutes_follow_permission_levels() {
    let Some(env) = Env::new().await else { return };
    let member = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let other_moderator = env.member().verified().role("moderateur").create().await;
    let admin = env.member().verified().role("admin").create().await;
    let mute = |by: &TestMember, whom: i64, body: Value| {
        Call::put(format!("/v1/moderation/members/{whom}/mute"))
            .member(by)
            .json(body)
    };
    let valid = json!({"days": 1, "reason": "Calme-toi."});

    for (by, whom) in [
        (&moderator, moderator.id),
        (&moderator, other_moderator.id),
        (&moderator, admin.id),
    ] {
        let (status, _) = mute(by, whom, valid.clone()).send(&env.app).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{} → {whom}", by.pseudo);
    }
    let (status, _) = mute(&member, moderator.id, valid.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = mute(&admin, moderator.id, valid.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = mute(&moderator, i64::MAX, valid).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = mute(&moderator, member.id, json!({"days": 0, "reason": ""}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("days".to_owned(), "out_of_range".to_owned()),
            ("reason".to_owned(), "required".to_owned()),
        ]
    );
    let (_, body) = mute(
        &moderator,
        member.id,
        json!({"days": 366, "reason": "x".repeat(301)}),
    )
    .send(&env.app)
    .await;
    assert_eq!(
        codes(&body),
        [
            ("days".to_owned(), "out_of_range".to_owned()),
            ("reason".to_owned(), "too_long".to_owned()),
        ]
    );
}

// --- Translation ------------------------------------------------------------------

/// DeepL's `/v2/translate`, in-process: answers `<target_lang> <n>: <text>`
/// for each text, `n` counting the requests, and records what it got.
struct FakeDeepl {
    url: String,
    requests: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
}

impl FakeDeepl {
    async fn start() -> Self {
        let requests: Arc<Mutex<Vec<(HeaderMap, Value)>>> = Arc::default();
        let seen = requests.clone();
        let router = Router::new().route(
            "/v2/translate",
            axum::routing::post(move |headers: HeaderMap, body: String| {
                let seen = seen.clone();
                async move {
                    let body: Value = serde_json::from_str(&body).unwrap();
                    let n = {
                        let mut seen = seen.lock().unwrap();
                        seen.push((headers, body.clone()));
                        seen.len()
                    };
                    let target = body["target_lang"].as_str().unwrap().to_owned();
                    let translations: Vec<Value> = body["text"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|t| {
                            json!({"detected_source_language": "EN",
                                   "text": format!("{target} {n}: {}", t.as_str().unwrap())})
                        })
                        .collect();
                    axum::Json(json!({"translations": translations}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { url, requests }
    }

    fn sent(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    async fn env(&self) -> Option<Env> {
        Env::with(&[
            ("DEEPL_API_KEY", "cle-de-test"),
            ("DEEPL_API_URL", &self.url),
        ])
        .await
    }
}

#[tokio::test]
async fn a_question_is_translated_then_served_from_the_cache() {
    let deepl = FakeDeepl::start().await;
    let Some(env) = deepl.env().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let path = format!("/v1/forum/questions/{}/translations", q.slug);

    // Readers do not need an account.
    let (status, body) = Call::post(&path)
        .json(json!({"lang": "fr"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"],
        json!({
            "title": "FR 1: Une question de test bien formulee",
            "body_md": "FR 1: Le corps original de la question, bien visible.",
            "source_lang": "en",
            "target_lang": "fr",
            "cached": false,
        })
    );
    {
        let requests = deepl.requests.lock().unwrap();
        let (headers, sent) = &requests[0];
        assert_eq!(headers["authorization"], "DeepL-Auth-Key cle-de-test");
        assert_eq!(
            sent,
            &json!({
                "text": ["Une question de test bien formulee",
                         "Le corps original de la question, bien visible."],
                "target_lang": "FR",
                "preserve_formatting": true,
            })
        );
    }

    let (_, body) = Call::post(&path)
        .json(json!({"lang": "fr"}))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["cached"], true);
    assert_eq!(
        body["data"]["title"],
        "FR 1: Une question de test bien formulee"
    );
    assert_eq!(deepl.sent(), 1);
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.forum_translations
              WHERE traduisible_type = 'question' AND traduisible_id = $1 AND langue_cible = 'fr'",
            q.id
        )
        .await,
        1
    );

    // A changed post is translated again, in the same row.
    sqlx::query(
        "UPDATE site.questions SET corps = 'Le contenu original a été modifié.' WHERE id = $1",
    )
    .bind(q.id)
    .execute(&env.owner)
    .await
    .unwrap();
    let (_, body) = Call::post(&path)
        .json(json!({"lang": "fr"}))
        .send(&env.app)
        .await;
    assert_eq!(
        body["data"]["body_md"],
        "FR 2: Le contenu original a été modifié."
    );
    assert_eq!(body["data"]["cached"], false);
    assert_eq!(deepl.sent(), 2);
    assert_eq!(
        count(
            &env,
            "SELECT count(*) FROM site.forum_translations
              WHERE traduisible_type = 'question' AND traduisible_id = $1",
            q.id
        )
        .await,
        1
    );
}

#[tokio::test]
async fn an_answer_is_translated_into_english() {
    let deepl = FakeDeepl::start().await;
    let Some(env) = deepl.env().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let a = answer(&env, q.id, &ada, None).await;

    let (status, body) = Call::post(format!("/v1/forum/answers/{a}/translations"))
        .member(&ada)
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["title"], Value::Null);
    assert_eq!(body["data"]["body_md"], "EN-GB 1: Une reponse de test.");
    assert_eq!(body["data"]["target_lang"], "en");
    let requests = deepl.requests.lock().unwrap();
    assert_eq!(requests[0].1["target_lang"], "EN-GB");
    assert_eq!(requests[0].1["text"], json!(["Une reponse de test."]));
}

#[tokio::test]
async fn translations_refuse_other_languages_and_hidden_posts() {
    let deepl = FakeDeepl::start().await;
    let Some(env) = deepl.env().await else { return };
    let ada = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let path = format!("/v1/forum/questions/{}/translations", q.slug);

    let (status, body) = Call::post(&path)
        .json(json!({"lang": "de"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("lang".to_owned(), "invalid".to_owned())]);
    let (_, body) = Call::post(&path).json(json!({})).send(&env.app).await;
    assert_eq!(codes(&body), [("lang".to_owned(), "required".to_owned())]);
    let (status, _) = Call::post("/v1/forum/answers/0/translations")
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    sqlx::query("UPDATE site.questions SET masque_le = now() WHERE id = $1")
        .bind(q.id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = Call::post(&path)
        .member(&ada)
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::post(&path)
        .member(&moderator)
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deepl.sent(), 1);
}

#[tokio::test]
async fn translations_are_limited_to_ten_a_minute() {
    let deepl = FakeDeepl::start().await;
    let Some(env) = deepl.env().await else { return };
    // Counted before the post is looked up, as Laravel's `throttle`.
    let ip = fresh_ip();
    for _ in 0..9 {
        let (status, _) = Call::post("/v1/forum/answers/0/translations")
            .ip(&ip)
            .json(json!({"lang": "en"}))
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let (status, _) = Call::post(format!("/v1/forum/questions/{}/translations", q.slug))
        .ip(&ip)
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = Call::post(format!("/v1/forum/questions/{}/translations", q.slug))
        .ip(&ip)
        .json(json!({"lang": "ru"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn without_deepl_translations_answer_503() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let (status, body) = Call::post(format!("/v1/forum/questions/{}/translations", q.slug))
        .json(json!({"lang": "fr"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["reason"], "not_configured");
    assert_eq!(body["detail"], "The translation service is not configured.");
}

#[tokio::test]
async fn a_failing_deepl_answers_503() {
    // Nothing listens there.
    let Some(env) = Env::with(&[
        ("DEEPL_API_KEY", "cle-de-test"),
        ("DEEPL_API_URL", "http://127.0.0.1:1"),
    ])
    .await
    else {
        return;
    };
    let ada = env.member().verified().create().await;
    let (theme_id, _) = theme(&env).await;
    let q = question(&env, theme_id, &ada).await;
    let (status, body) = Call::post(format!("/v1/forum/questions/{}/translations", q.slug))
        .json(json!({"lang": "en"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["reason"], "upstream_failed");
    assert!(body["trace_id"].is_string() || body["trace_id"].is_null());
}
