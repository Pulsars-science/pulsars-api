//! `/v1/admin/*` end to end, with the cases of Laravel's `PanelAdminTest`,
//! `ActualiteTest`, `ExamenTest`, `DemandeRessourceTest`,
//! `GestionRolesTest`, `SignalementTest`, `SuggestionTest`,
//! `ObjectifDonsTest`, `PastilleAttenteTest` and
//! `TableauDeBordStatistiquesTest` that concern the administration.
//!
//! Rows are made per test with unique titles, so the tests run in parallel
//! and again on the same database; lists are read through a filter or a
//! search that finds only the test's own rows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use common::{Call, Env, TestMember, codes, fresh_ip, unique};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const BOUNDARY: &str = "pulsars-test-boundary";

/// A `multipart/form-data` body: text fields, then files as
/// (name, file name, bytes).
fn multipart(texts: &[(&str, &str)], files: &[(&str, &str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in texts {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    for (name, file_name, bytes) in files {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\nContent-Type: application/pdf\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// A request with a raw body, as `member`.
async fn send_raw(
    env: &Env,
    method: &str,
    path: &str,
    member: &TestMember,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::USER_AGENT, "integration-tests")
        .header("x-forwarded-for", fresh_ip())
        .header("pulsars-session", &member.token)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap();
    let response = env.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, bytes.to_vec())
}

async fn upload(env: &Env, admin: &TestMember, body: Vec<u8>) -> (StatusCode, Value) {
    let (status, _, bytes) = send_raw(
        env,
        "POST",
        "/v1/admin/exams",
        admin,
        &format!("multipart/form-data; boundary={BOUNDARY}"),
        body,
    )
    .await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn pdf_bytes(size: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    bytes.resize(size, b'x');
    bytes
}

async fn admin(env: &Env) -> TestMember {
    env.member().verified().role("admin").create().await
}

// --- Access ---------------------------------------------------------------

#[tokio::test]
async fn the_administration_is_for_admins_only() {
    let Some(env) = Env::new().await else { return };
    let member = env.member().verified().create().await;
    let moderator = env.member().verified().role("moderateur").create().await;
    let unverified = env.member().role("admin").create().await;
    let admin = admin(&env).await;
    let paths = [
        "/v1/admin/stats",
        "/v1/admin/pending",
        "/v1/admin/containers",
        "/v1/admin/people?q=ab",
        "/v1/admin/activity",
        "/v1/admin/news",
        "/v1/admin/courses",
        "/v1/admin/exams",
        "/v1/admin/resource-requests",
        "/v1/admin/members",
        "/v1/admin/reports",
        "/v1/admin/suggestions",
        "/v1/admin/game/stats",
        "/v1/admin/game/leaderboard",
        "/v1/admin/languages/stats",
    ];
    for path in paths {
        let (status, _) = Call::get(path).send(&env.app).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        for who in [&member, &moderator, &unverified] {
            let (status, _) = Call::get(path).member(who).send(&env.app).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
        let (status, headers, body) = Call::get(path).member(&admin).send_full(&env.app).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store", "{path}");
    }

    // A member cannot act either.
    let (status, _) = Call::post("/v1/admin/courses/les-bases-du-calcul/publish")
        .member(&member)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = Call::put("/v1/admin/settings/donation-goal")
        .member(&member)
        .json(json!({"goal": 100, "raised": 10}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// --- Dashboard ------------------------------------------------------------

#[tokio::test]
async fn the_dashboard_has_its_period_and_its_series() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;

    let (status, body) = Call::get("/v1/admin/stats")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["period"]["days"], 30);
    assert_eq!(data["period"]["choices"], json!([7, 30, 90]));
    assert_eq!(data["period"]["dates"].as_array().unwrap().len(), 30);
    // Series are filled with zeros day by day, even without data.
    for name in [
        "signups",
        "lessons",
        "attempts",
        "questions",
        "brains",
        "games",
    ] {
        assert_eq!(
            data["series"][name]["values"].as_array().unwrap().len(),
            30,
            "{name}"
        );
    }
    assert_eq!(data["series"]["visitors"], Value::Null);
    assert_eq!(
        data["active_members"]["values"].as_array().unwrap().len(),
        30
    );
    // The admin was just created: counted among today's sign-ups.
    assert!(data["accounts"]["signups"]["24h"].as_i64().unwrap() >= 1);
    assert!(data["series"]["signups"]["values"][29].as_i64().unwrap() >= 1);
    assert_eq!(data["audience"]["configured"], false);
    assert_eq!(data["audience"]["available"], false);
    assert_eq!(data["funnel"][0]["step"], "signed_up");
    for key in ["duel", "room", "ghost"] {
        assert!(data["games"]["by_mode"][key]["values"].is_array(), "{key}");
    }
    assert!(data["to_process"]["resource_requests"].is_i64());
    assert!(data["accounts"]["active_7d"].is_i64());
    assert!(data["audience"]["connected_24h"].is_i64());
    assert!(data["pending"]["total"].is_i64());
    assert!(data["donation_goal"]["goal"].is_number());

    let (_, body) = Call::get("/v1/admin/stats?days=7")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["period"]["days"], 7);
    assert_eq!(
        body["data"]["series"]["signups"]["values"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    // An unknown period falls back to thirty days.
    let (status, body) = Call::get("/v1/admin/stats?days=12")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["period"]["days"], 30);
}

#[tokio::test]
async fn brains_and_matches_are_counted_by_reason_and_by_mode() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let player = env.member().verified().create().await;
    let reason = format!("t{}", &unique()[..8]);
    sqlx::query(
        "INSERT INTO site.cerveaux_mouvements (user_id, montant, raison, cle, created_at)
         VALUES ($1, 5, $2, $3, now() AT TIME ZONE 'UTC'), ($1, 3, $2, $4, now() AT TIME ZONE 'UTC'),
                ($1, -4, $2, $5, now() AT TIME ZONE 'UTC')",
    )
    .bind(player.id)
    .bind(&reason)
    .bind(unique())
    .bind(unique())
    .bind(unique())
    .execute(&env.owner)
    .await
    .unwrap();
    let (_, body) = Call::get("/v1/admin/stats?days=7")
        .member(&admin)
        .send(&env.app)
        .await;
    let ours = body["data"]["brains"]["by_reason"][&reason].clone();
    // Only what was earned: the spending does not count.
    assert_eq!(ours["total"], 8);
    assert_eq!(ours["values"][6], 8);
    assert_eq!(ours["previous"], 0);
    sqlx::query("DELETE FROM site.cerveaux_mouvements WHERE user_id = $1")
        .bind(player.id)
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_donation_goal_is_typed_in_by_hand() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;

    let (status, body) = Call::put("/v1/admin/settings/donation-goal")
        .member(&admin)
        .json(json!({"goal": 2800, "raised": 1234.5}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], json!({"goal": 2800.0, "raised": 1234.5}));
    let stored: Value =
        sqlx::query_scalar("SELECT valeur::jsonb FROM site.reglages WHERE cle = 'objectif_dons'")
            .fetch_one(&env.db)
            .await
            .unwrap();
    assert_eq!(stored["montant"].as_f64(), Some(2800.0));
    assert_eq!(stored["collecte"].as_f64(), Some(1234.5));

    let (status, body) = Call::put("/v1/admin/settings/donation-goal")
        .member(&admin)
        .json(json!({"goal": -1, "raised": 2_000_000}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("goal".to_owned(), "too_small".to_owned()),
            ("raised".to_owned(), "too_large".to_owned())
        ]
    );
    let (_, body) = Call::put("/v1/admin/settings/donation-goal")
        .member(&admin)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(codes(&body).len(), 2);

    // 0 takes the goal off the site, and is valid.
    let (status, _) = Call::put("/v1/admin/settings/donation-goal")
        .member(&admin)
        .json(json!({"goal": 0, "raised": 0}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn game_and_languages_statistics_take_a_session() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let (status, body) = Call::get("/v1/admin/game/stats?days=7")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["days"], 7);
    assert_eq!(body["data"]["available"], true);
    assert!(body["data"]["stats"]["live"].is_object(), "{body}");

    let (status, body) = Call::get("/v1/admin/languages/stats")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"].is_object());
}

// --- News -----------------------------------------------------------------

#[tokio::test]
async fn news_are_written_as_drafts_then_published() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let title = format!("Annonce {}", unique());

    let (status, body) = Call::post("/v1/admin/news")
        .member(&admin)
        .json(json!({
            "title": format!("  {title}  "),
            "lead": "",
            "content_md": "Un texte assez long pour passer la validation.",
        }))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let news = &body["data"];
    assert!(news["id"].is_i64());
    let slug = news["slug"].as_str().unwrap().to_owned();
    assert_eq!(news["title"], title.as_str());
    assert_eq!(news["lead"], Value::Null);
    assert_eq!(news["published"], false);
    assert_eq!(news["author"], admin.pseudo.as_str());
    assert_eq!(slug, format!("annonce-{}", title[8..].to_lowercase()));

    // The same title gets another address.
    let (_, twin) = Call::post("/v1/admin/news")
        .member(&admin)
        .json(json!({"title": title, "content_md": "Un autre texte assez long, lui aussi."}))
        .send(&env.app)
        .await;
    assert_eq!(twin["data"]["slug"], format!("{slug}-2"));

    let public = format!("/v1/news/{slug}");
    let (status, _) = Call::get(&public).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = Call::post(format!("/v1/admin/news/{slug}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["published"], true);
    let (status, _) = Call::get(&public).send(&env.app).await;
    assert_eq!(status, StatusCode::OK);

    // Editing keeps the address.
    let (status, body) = Call::put(format!("/v1/admin/news/{slug}"))
        .member(&admin)
        .json(json!({
            "title": "Un titre tout neuf",
            "lead": "Le chapeau.",
            "content_md": "Le texte, réécrit de bout en bout.",
        }))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["slug"], slug.as_str());
    assert_eq!(body["data"]["lead"], "Le chapeau.");

    let (_, body) = Call::post(format!("/v1/admin/news/{slug}/unpublish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["published"], false);
    assert_eq!(body["data"]["published_at"], Value::Null);

    let (_, body) = Call::get(format!("/v1/admin/news?q={}", &title[8..]))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["per_page"], 25);
    assert!(body["counters"]["all"].as_i64().unwrap() >= 2);

    let (status, _) = Call::delete(format!("/v1/admin/news/{slug}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = Call::delete(format!("/v1/admin/news/{slug}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM contenu.actualites WHERE slug = $1")
        .bind(format!("{slug}-2"))
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn news_fields_follow_laravel_rules() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let (status, body) = Call::post("/v1/admin/news")
        .member(&admin)
        .json(json!({"title": "abc", "lead": "x".repeat(501), "content_md": "court"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("title".to_owned(), "too_short".to_owned()),
            ("lead".to_owned(), "too_long".to_owned()),
            ("content_md".to_owned(), "too_short".to_owned()),
        ]
    );
    let (status, _) = Call::post("/v1/admin/news")
        .member(&admin)
        .json(json!({"titre": "Un titre français"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // The default body limit still holds here.
    let (status, _) = Call::post("/v1/admin/news")
        .member(&admin)
        .json(json!({"title": "Trop long", "content_md": "x".repeat(100_000)}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

// --- Courses --------------------------------------------------------------

async fn create_course(env: &Env, admin: &TestMember, title: &str) -> Value {
    let (status, body) = Call::post("/v1/admin/courses")
        .member(admin)
        .json(json!({
            "title": title,
            "description": "Une description d'au moins vingt caractères.",
            "theme": "Mathématiques – Analyse",
            "difficulty": "debutant",
            "duration_minutes": 45,
        }))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["data"].clone()
}

#[tokio::test]
async fn a_course_is_created_as_a_draft_and_published_with_lessons() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let title = format!("Cours {}", unique());
    let course = create_course(&env, &admin, &title).await;
    let id = course["id"].as_i64().unwrap();
    let key = course["slug"].as_str().unwrap().to_owned();
    assert_eq!(course["status"], "brouillon");
    assert_eq!(course["publishable"], false);
    assert_eq!(course["lesson_count"], 0);

    // A course without lessons cannot be published.
    let (status, body) = Call::post(format!("/v1/admin/courses/{key}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [(String::new(), "no_lessons".to_owned())]);

    let chapter: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.chapters (course_id, titre, ordre) VALUES ($1, 'Chapitre', 1) RETURNING id",
    )
    .bind(id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO contenu.lessons (chapter_id, titre, slug, contenu_md, ordre)
         VALUES ($1, 'Leçon', 'lecon', 'Du texte.', 1)",
    )
    .bind(chapter)
    .execute(&env.owner)
    .await
    .unwrap();

    let (status, body) = Call::post(format!("/v1/admin/courses/{key}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "publie");
    let first = body["data"]["published_at"].clone();
    assert!(first.is_string());

    let (_, body) = Call::post(format!("/v1/admin/courses/{key}/unpublish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["status"], "brouillon");
    let (_, body) = Call::post(format!("/v1/admin/courses/{key}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    // The first publication date is kept.
    assert_eq!(body["data"]["published_at"], first);

    let (_, body) = Call::post(format!("/v1/admin/courses/{key}/archive"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["status"], "archive");
    let (_, body) = Call::get(format!(
        "/v1/admin/courses?status=archive&q={}",
        &title[6..]
    ))
    .member(&admin)
    .send(&env.app)
    .await;
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["data"][0]["id"], id);
    assert!(
        body["themes"]
            .as_array()
            .unwrap()
            .contains(&json!("Mathématiques – Analyse"))
    );
    let (_, body) = Call::get(format!("/v1/admin/courses?status=publie&q={}", &title[6..]))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 0);

    let (status, _) = Call::delete(format!("/v1/admin/courses/{key}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_followed_course_cannot_be_deleted() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let learner = env.member().verified().create().await;
    let course = create_course(&env, &admin, &format!("Suivi {}", unique())).await;
    let id = course["id"].as_i64().unwrap();
    let key = course["slug"].as_str().unwrap().to_owned();
    sqlx::query("INSERT INTO site.enrollments (user_id, course_id) VALUES ($1, $2)")
        .bind(learner.id)
        .bind(id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, body) = Call::delete(format!("/v1/admin/courses/{key}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["reason"], "has_enrollments");
    sqlx::query("DELETE FROM contenu.courses WHERE id = $1")
        .bind(id)
        .execute(&env.owner)
        .await
        .unwrap();

    let (status, body) = Call::post("/v1/admin/courses")
        .member(&admin)
        .json(json!({"title": "Cour", "difficulty": "expert", "duration_minutes": 2}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("title".to_owned(), "too_short".to_owned()),
            ("description".to_owned(), "required".to_owned()),
            ("theme".to_owned(), "required".to_owned()),
            ("difficulty".to_owned(), "invalid".to_owned()),
            ("duration_minutes".to_owned(), "too_small".to_owned()),
        ]
    );
}

// --- Exams ----------------------------------------------------------------

#[tokio::test]
async fn an_exam_goes_from_its_pdf_to_a_published_paper() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let pdf = pdf_bytes(200 * 1024);

    let (status, body) = upload(
        &env,
        &admin,
        multipart(
            &[
                ("subject", "mathematiques"),
                ("country", "FR"),
                ("ai_prompt", "Conserve le barème officiel."),
            ],
            &[("pdf", "bac-maths-2025.pdf", &pdf)],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let exam = &body["data"];
    let id = exam["id"].as_i64().unwrap();
    let key = exam["slug"].as_str().unwrap().to_owned();
    assert_eq!(exam["status"], "attente_import");
    assert_eq!(exam["title"], "Bac Maths 2025");
    assert_eq!(exam["country"], "FR");
    assert_eq!(exam["original_filename"], "bac-maths-2025.pdf");
    assert_eq!(exam["subject_label"], "Mathématiques");
    assert_eq!(exam["ai_prompt"], "Conserve le barème officiel.");
    let prompt = exam["import_prompt"].as_str().unwrap();
    assert!(prompt.contains("Je joins à ce message un sujet d’examen"));
    assert!(prompt.contains("Conserve le barème officiel."));
    assert!(prompt.contains("\"parties\""));
    assert!(exam["slug"].as_str().unwrap().starts_with("analyse-"));

    // Stored on the `local` disk, as Laravel did.
    let (path, size): (String, i64) = sqlx::query_as(
        "SELECT f.chemin, f.taille FROM contenu.exams x
           JOIN contenu.fichiers f ON f.disque = 'local' AND f.chemin = x.fichier_pdf
          WHERE x.id = $1",
    )
    .bind(id)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert!(
        path.starts_with("examens/") && path.ends_with(".pdf") && path.len() == 52,
        "{path}"
    );
    assert_eq!(size, i64::try_from(pdf.len()).unwrap());
    let (status, headers, bytes) = send_raw(
        &env,
        "GET",
        &format!("/v1/admin/exams/{key}/pdf"),
        &admin,
        "application/json",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(bytes, pdf);

    // Not JSON: refused, nothing changes.
    let (status, body) = Call::post(format!("/v1/admin/exams/{key}/import"))
        .member(&admin)
        .json(json!({"ai_response": "Voici les réponses du sujet."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [("ai_response".to_owned(), "invalid_json".to_owned())]
    );

    // A part without statement cannot be published.
    let (status, body) = Call::post(format!("/v1/admin/exams/{key}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [(String::new(), "no_parts".to_owned())]);

    let title = format!("Bac 2025 — Mathématiques {}", unique());
    let answer = json!({
        "titre": title,
        "type": "bac",
        "niveau": "Terminale",
        "session": "2025",
        "duree_minutes": 240,
        "points_total": 20,
        "description": "Sujet officiel analysé depuis le PDF.",
        "parties": [{
            "titre": "Suites",
            "points": 2.5,
            "enonce_md": "Étudier la suite.",
            "corrige_md": "On procède par récurrence.",
        }],
    });
    let (status, body) = Call::post(format!("/v1/admin/exams/{key}/import"))
        .member(&admin)
        .json(json!({"ai_response": format!("```json\n{answer}\n```")}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let exam = &body["data"];
    assert_eq!(exam["status"], "brouillon");
    assert_eq!(exam["title"], title.as_str());
    assert_eq!(exam["part_count"], 1);
    assert!(exam["analyzed_at"].is_string());
    assert_eq!(exam["published_at"], Value::Null);
    let slug = exam["slug"].as_str().unwrap().to_owned();
    assert!(slug.starts_with("bac-2025-mathematiques-"), "{slug}");
    let key = slug.clone();
    let points: f64 = sqlx::query_scalar(
        "SELECT points::float8 FROM contenu.exam_parties WHERE exam_id = $1 AND titre = 'Suites'",
    )
    .bind(id)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert!((points - 2.5).abs() < f64::EPSILON);

    // A draft is not public; once published, its PDF is served.
    let (status, _) = Call::get(format!("/v1/exams/{slug}")).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = Call::post(format!("/v1/admin/exams/{key}/publish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "publie");
    let (status, _) = Call::get(format!("/v1/exams/{slug}")).send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, bytes) = send_raw(
        &env,
        "GET",
        &format!("/v1/exams/{slug}/pdf"),
        &admin,
        "application/json",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, pdf);

    let (_, body) = Call::post(format!("/v1/admin/exams/{key}/unpublish"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["status"], "brouillon");

    let (_, body) = Call::get("/v1/admin/exams")
        .member(&admin)
        .send(&env.app)
        .await;
    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == id)
    );
    assert_eq!(body["subjects"][0]["value"], "mathematiques");
    assert!(body["countries"].as_array().unwrap().contains(&json!("GB")));

    // Deleting takes the PDF with it.
    let (status, _) = Call::delete(format!("/v1/admin/exams/{key}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let left: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM contenu.fichiers WHERE disque = 'local' AND chemin = $1)",
    )
    .bind(&path)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert!(!left);
}

#[tokio::test]
async fn an_upload_needs_a_pdf_a_subject_and_a_known_country() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    // This admin's exams: other tests make theirs in parallel.
    let mine = "SELECT count(*) FROM contenu.exams WHERE auteur_id = $1";
    let before: i64 = sqlx::query_scalar(mine)
        .bind(admin.id)
        .fetch_one(&env.db)
        .await
        .unwrap();

    let (status, body) = upload(
        &env,
        &admin,
        multipart(
            &[("subject", "mathematiques"), ("country", "XX")],
            &[("pdf", "sujet.pdf", &pdf_bytes(1000))],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("country".to_owned(), "invalid".to_owned())]);

    let (_, body) = upload(
        &env,
        &admin,
        multipart(
            &[("subject", "astrologie")],
            &[("pdf", "sujet.pdf", b"PK\x03\x04 not a pdf")],
        ),
    )
    .await;
    assert_eq!(
        codes(&body),
        [
            ("pdf".to_owned(), "not_pdf".to_owned()),
            ("subject".to_owned(), "invalid".to_owned()),
            ("country".to_owned(), "required".to_owned()),
        ]
    );

    let (_, body) = upload(&env, &admin, multipart(&[("country", "FR")], &[])).await;
    assert_eq!(codes(&body)[0], ("pdf".to_owned(), "required".to_owned()));

    // `max:20480`: one byte too many.
    let (status, body) = upload(
        &env,
        &admin,
        multipart(
            &[("subject", "physique"), ("country", "BE")],
            &[("pdf", "gros.pdf", &pdf_bytes(20 * 1024 * 1024 + 1))],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("pdf".to_owned(), "too_large".to_owned())]);

    // Over the route's limit: refused while reading.
    let (status, _) = upload(
        &env,
        &admin,
        multipart(&[], &[("pdf", "enorme.pdf", &pdf_bytes(22 * 1024 * 1024))]),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let (status, _, _) = send_raw(
        &env,
        "POST",
        "/v1/admin/exams",
        &admin,
        "application/json",
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let after: i64 = sqlx::query_scalar(mine)
        .bind(admin.id)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(after, before, "a refused upload created an exam");
}

// --- Resource requests ----------------------------------------------------

/// A request planted as `POST /ressources/proposer` leaves it.
async fn plant_request(env: &Env, member: &TestMember, title: &str) -> (i64, Vec<u8>) {
    let pdf = pdf_bytes(4096);
    let path = format!("demandes-ressources/{}.pdf", unique());
    sqlx::query(
        "INSERT INTO contenu.fichiers (disque, chemin, contenu, type_mime, taille, created_at)
         VALUES ('local', $1, $2, 'application/pdf', $3, now())",
    )
    .bind(&path)
    .bind(&pdf)
    .bind(i64::try_from(pdf.len()).unwrap())
    .execute(&env.owner)
    .await
    .unwrap();
    let id = sqlx::query_scalar(
        "INSERT INTO site.demandes_ressources (user_id, titre, auteur_nom, licence, categorie,
                                               fichier, nom_fichier_original, taille, created_at, updated_at)
         VALUES ($1, $2, 'Grace Hopper', 'cc-by-sa-4.0', 'Méthode', $3, 'cours.pdf', $4, now(), now())
         RETURNING id",
    )
    .bind(member.id)
    .bind(title)
    .bind(&path)
    .bind(i64::try_from(pdf.len()).unwrap())
    .fetch_one(&env.owner)
    .await
    .unwrap();
    (id, pdf)
}

#[tokio::test]
async fn an_admin_accepts_a_resource_request_and_publishes_it() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let member = env.member().verified().create().await;
    let title = format!("Annales corrigées {}", unique());
    let (id, pdf) = plant_request(&env, &member, &title).await;

    let (_, body) = Call::get("/v1/admin/resource-requests?status=en_attente")
        .member(&admin)
        .send(&env.app)
        .await;
    let ours = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("{body}"));
    assert_eq!(ours["title"], title.as_str());
    assert_eq!(ours["author_name"], "Grace Hopper");
    assert_eq!(ours["requested_by"], member.pseudo.as_str());
    assert_eq!(ours["license"]["name"], "CC BY-SA 4.0");
    assert_eq!(ours["status"], "en_attente");
    assert!(body["counters"]["en_attente"].as_i64().unwrap() >= 1);

    let (status, headers, bytes) = send_raw(
        &env,
        "GET",
        &format!("/v1/admin/resource-requests/{id}/file"),
        &admin,
        "application/json",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("inline;")
    );
    assert_eq!(bytes, pdf);

    // A member cannot accept.
    let (status, _) = Call::post(format!("/v1/admin/resource-requests/{id}/accept"))
        .member(&member)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = Call::post(format!("/v1/admin/resource-requests/{id}/accept"))
        .member(&admin)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "acceptee");
    assert_eq!(body["data"]["handled_by"], admin.pseudo.as_str());
    let slug = body["data"]["resource_slug"].as_str().unwrap().to_owned();

    // Published, with its credit and licence, and its own copy of the file.
    let (titre, auteur, licence, statut, fichier, ajoute_par): (
        String,
        String,
        String,
        String,
        String,
        i64,
    ) = sqlx::query_as(
        "SELECT titre, auteur_nom, licence, statut, fichier, ajoute_par
               FROM contenu.ressources WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(
        (
            titre.as_str(),
            auteur.as_str(),
            licence.as_str(),
            statut.as_str()
        ),
        (title.as_str(), "Grace Hopper", "cc-by-sa-4.0", "publie")
    );
    assert!(fichier.starts_with("ressources/"));
    assert_eq!(ajoute_par, member.id);
    let (status, _, bytes) = send_raw(
        &env,
        "GET",
        &format!("/v1/resources/{slug}/file"),
        &admin,
        "application/json",
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, pdf);

    let (status, body) = Call::post(format!("/v1/admin/resource-requests/{id}/accept"))
        .member(&admin)
        .json(json!({}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["reason"], "already_handled");
    let (status, body) = Call::post(format!("/v1/admin/resource-requests/{id}/reject"))
        .member(&admin)
        .json(json!({"reason": "Trop tard."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["reason"], "already_accepted");

    sqlx::query("DELETE FROM contenu.ressources WHERE slug = $1")
        .bind(&slug)
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_refusal_needs_a_reason() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let member = env.member().verified().create().await;
    let (id, _) = plant_request(&env, &member, &format!("Hors sujet {}", unique())).await;

    let (status, body) = Call::post(format!("/v1/admin/resource-requests/{id}/reject"))
        .member(&admin)
        .json(json!({"reason": "  "}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("reason".to_owned(), "required".to_owned())]);

    let (status, body) = Call::post(format!("/v1/admin/resource-requests/{id}/reject"))
        .member(&admin)
        .json(json!({"reason": "Hors sujet pour la plateforme."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "refusee");
    assert_eq!(body["data"]["reason"], "Hors sujet pour la plateforme.");
    assert_eq!(body["data"]["resource_slug"], Value::Null);

    let (status, _) = Call::post("/v1/admin/resource-requests/999999999/reject")
        .member(&admin)
        .json(json!({"reason": "Absent."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// --- Roles ----------------------------------------------------------------

async fn roles_of(env: &Env, id: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT r.slug::text FROM site.role_user ru JOIN site.roles r ON r.id = ru.role_id
          WHERE ru.user_id = $1 ORDER BY 1",
    )
    .bind(id)
    .fetch_all(&env.db)
    .await
    .unwrap()
}

#[tokio::test]
async fn an_admin_manages_subjects_but_not_permissions() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let target = env.member().verified().create().await;
    let path = |slug: &str| format!("/v1/admin/members/{}/roles/{slug}", target.id);

    let (status, body) = Call::put(path("physique"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["roles"], json!(["physique"]));
    // Giving it again changes nothing.
    let (status, _) = Call::put(path("physique"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(roles_of(&env, target.id).await, ["physique"]);

    let (status, _) = Call::delete(path("physique"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(roles_of(&env, target.id).await.is_empty());

    // An admin makes no other admin.
    let (status, _) = Call::put(path("admin")).member(&admin).send(&env.app).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(roles_of(&env, target.id).await.is_empty());

    let (status, _) = Call::put(path("role-bidon"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = Call::put("/v1/admin/members/999999999/roles/physique")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, body) = Call::get(format!("/v1/admin/members?q={}", target.pseudo))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["data"][0]["email"], target.email.as_str());
    assert_eq!(body["can_manage_permissions"], false);
    assert_eq!(body["catalog"]["permission"][0]["slug"], "super-admin");
    assert!(body["catalog"]["matiere"].is_array());
}

#[tokio::test]
async fn a_super_admin_manages_permissions_but_keeps_their_own() {
    let Some(env) = Env::new().await else { return };
    let boss = env.member().verified().role("super-admin").create().await;
    let target = env.member().verified().create().await;

    let (status, body) = Call::put(format!("/v1/admin/members/{}/roles/moderateur", target.id))
        .member(&boss)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(roles_of(&env, target.id).await, ["moderateur"]);

    let (status, body) = Call::delete(format!("/v1/admin/members/{}/roles/super-admin", boss.id))
        .member(&boss)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [(String::new(), "own_super_admin".to_owned())]
    );
    assert_eq!(roles_of(&env, boss.id).await, ["super-admin"]);

    let (_, body) = Call::get("/v1/admin/members")
        .member(&boss)
        .send(&env.app)
        .await;
    assert_eq!(body["can_manage_permissions"], true);
}

// --- Reports and suggestions ----------------------------------------------

#[tokio::test]
async fn an_admin_files_a_report() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let reader = env.member().verified().create().await;
    // Course 1 of the demo data, « Les bases du calcul ».
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.signalements (signalable_type, signalable_id, user_id, type, message,
                                        emplacement, created_at, updated_at)
         VALUES ('course', 1, $1, 'faute', 'Une faute dans la leçon 2.', 'Leçon 2', now(), now())
         RETURNING id",
    )
    .bind(reader.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    let ghost: i64 = sqlx::query_scalar(
        "INSERT INTO site.signalements (signalable_type, signalable_id, type, message, created_at)
         VALUES ('lesson', 999999999, 'amelioration', 'Une idée.', now()) RETURNING id",
    )
    .fetch_one(&env.owner)
    .await
    .unwrap();

    let (_, body) = Call::get("/v1/admin/reports?status=nouveau&type=faute")
        .member(&admin)
        .send(&env.app)
        .await;
    let ours = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("{body}"));
    assert_eq!(ours["author"], reader.pseudo.as_str());
    assert_eq!(ours["location"], "Leçon 2");
    assert_eq!(
        ours["target"],
        json!({
            "type": "course", "id": 1, "exists": true, "label": "Les bases du calcul",
            "web_url": "https://pulsars.science/cours/les-bases-du-calcul",
            "course_slug": "les-bases-du-calcul",
        })
    );
    let (_, body) = Call::get("/v1/admin/reports?type=amelioration")
        .member(&admin)
        .send(&env.app)
        .await;
    let deleted = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == ghost)
        .cloned()
        .unwrap();
    assert_eq!(deleted["target"]["exists"], false);
    assert_eq!(deleted["target"]["web_url"], Value::Null);

    let (_, before) = Call::get("/v1/admin/pending")
        .member(&admin)
        .send(&env.app)
        .await;
    let (status, body) = Call::patch(format!("/v1/admin/reports/{id}"))
        .member(&admin)
        .json(json!({"status": "corrige", "reply": "Corrigé, merci !"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "corrige");
    assert_eq!(body["data"]["reply"], "Corrigé, merci !");
    assert_eq!(body["data"]["handled_by"], admin.pseudo.as_str());
    let (_, after) = Call::get("/v1/admin/pending")
        .member(&admin)
        .send(&env.app)
        .await;
    // A closed report leaves the to-do list (other tests may add theirs).
    assert!(after["data"]["reports"].as_i64() <= before["data"]["reports"].as_i64());

    // Reopening clears the handling, and keeps the reply.
    let (_, body) = Call::patch(format!("/v1/admin/reports/{id}"))
        .member(&admin)
        .json(json!({"status": "nouveau"}))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["handled_by"], Value::Null);
    assert_eq!(body["data"]["handled_at"], Value::Null);
    assert_eq!(body["data"]["reply"], "Corrigé, merci !");

    let (status, body) = Call::patch(format!("/v1/admin/reports/{id}"))
        .member(&admin)
        .json(json!({"status": "supprime"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body), [("status".to_owned(), "invalid".to_owned())]);
    let (status, _) = Call::patch(format!("/v1/admin/reports/{id}"))
        .member(&reader)
        .json(json!({"status": "refuse"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    sqlx::query("DELETE FROM site.signalements WHERE id IN ($1, $2)")
        .bind(id)
        .bind(ghost)
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_admin_files_a_suggestion() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let member = env.member().verified().create().await;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.suggestions (user_id, titre, message, image, created_at, updated_at)
         VALUES ($1, 'Mode sombre', 'Un thème sombre, svp.', 'suggestions/capture 1.png', now(), now())
         RETURNING id",
    )
    .bind(member.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();

    let (_, body) = Call::get("/v1/admin/suggestions?status=nouvelle")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(body["per_page"], 20);
    let ours = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("{body}"));
    assert_eq!(ours["author"], member.pseudo.as_str());
    assert_eq!(
        ours["image_url"],
        "https://pulsars.science/storage/suggestions/capture%201.png"
    );

    let (status, body) = Call::patch(format!("/v1/admin/suggestions/{id}"))
        .member(&admin)
        .json(json!({"status": "retenue", "reply": "Bonne idée."}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "retenue");
    assert!(body["data"]["handled_at"].is_string());

    let (status, _) = Call::patch("/v1/admin/suggestions/999999999")
        .member(&admin)
        .json(json!({"status": "ecartee"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM site.suggestions WHERE id = $1")
        .bind(id)
        .execute(&env.owner)
        .await
        .unwrap();
}

// --- Umami ------------------------------------------------------------------

/// A fake Umami: a token for the right password, numbers for that token.
async fn fake_umami() -> String {
    use axum::Json as AxumJson;
    use axum::extract::Path;
    use axum::routing::{get, post};

    async fn login(AxumJson(body): AxumJson<Value>) -> Result<AxumJson<Value>, StatusCode> {
        if body["username"] == "admin" && body["password"] == "umami-secret" {
            Ok(AxumJson(json!({"token": "fake-token"})))
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
    async fn website(
        headers: HeaderMap,
        Path(rest): Path<String>,
    ) -> Result<AxumJson<Value>, StatusCode> {
        if headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            != Some("Bearer fake-token")
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let today = chrono::Utc::now().format("%Y-%m-%d 00:00:00").to_string();
        Ok(AxumJson(match rest.as_str() {
            "" => json!({"createdAt": "2026-08-01T00:00:00.000Z"}),
            "stats" => json!({"visitors": 12, "pageviews": 34}),
            "pageviews" => json!({"pageviews": [], "sessions": [{"x": today, "y": 5}]}),
            "metrics" => json!([{"x": "/cours", "y": 9}, {"x": "/", "y": 4}]),
            _ => return Err(StatusCode::NOT_FOUND),
        }))
    }
    async fn site(headers: HeaderMap) -> Result<AxumJson<Value>, StatusCode> {
        website(headers, Path(String::new())).await
    }

    let app = axum::Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/websites/site-1", get(site))
        .route("/api/websites/site-1/{rest}", get(website));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn the_audience_comes_from_umami_when_configured() {
    let url = fake_umami().await;
    let Some(env) = Env::with(&[
        ("UMAMI_URL", url.as_str()),
        ("UMAMI_WEBSITE_ID", "site-1"),
        ("UMAMI_USERNAME", "admin"),
        ("UMAMI_PASSWORD", "umami-secret"),
    ])
    .await
    else {
        return;
    };
    {
        use fred::prelude::KeysInterface as _;
        let _: i64 = env
            .state
            .redis
            .del(vec![
                "umami:token",
                "admin:umami:audience",
                "admin:umami:visitors:7",
                "admin:umami:pages:7",
            ])
            .await
            .unwrap();
    }
    let admin = admin(&env).await;
    let (status, body) = Call::get("/v1/admin/stats?days=7")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["audience"]["configured"], true);
    assert_eq!(data["audience"]["available"], true, "{}", data["audience"]);
    assert_eq!(
        data["audience"]["visitors"],
        json!({"24h": 12, "7d": 12, "28d": 12, "total": 12})
    );
    assert_eq!(data["audience"]["page_views"]["total"], 34);
    assert_eq!(data["audience"]["measured_since"], "2026-08-01T00:00:00Z");
    assert_eq!(data["series"]["visitors"]["values"][6], 5);
    assert_eq!(data["series"]["visitors"]["total"], 5);
    assert_eq!(
        data["popular_pages"],
        json!([{"path": "/cours", "views": 9}, {"path": "/", "views": 4}])
    );
}

// --- Dashboard: games, downloads, HTTP, catalog, views --------------------

/// Held by the tests that add downloads: the series counts every one.
static DOWNLOADS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `GET /v1/admin/stats?days=7` as `admin`, its `data`.
async fn stats7(env: &Env, admin: &TestMember) -> Value {
    let (status, body) = Call::get("/v1/admin/stats?days=7")
        .member(admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"].clone()
}

/// A published course of `theme`, with `exercises` exercises; its id and slug.
async fn published_course(env: &Env, author: i64, theme: &str, exercises: i32) -> (i64, String) {
    let slug = format!("cours-{}", unique());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.courses (titre, slug, description, theme, auteur_id, statut, published_at)
         VALUES ($1, $1, 'Description', $2, $3, 'publie', now() AT TIME ZONE 'UTC' - interval '1 hour')
         RETURNING id",
    )
    .bind(&slug)
    .bind(theme)
    .bind(author)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    let chapter: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.chapters (course_id, titre, ordre) VALUES ($1, 'Chapitre', 1) RETURNING id",
    )
    .bind(id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    for n in 0..exercises {
        sqlx::query(
            "INSERT INTO contenu.exercises (chapter_id, type, titre, question_md, donnees, ordre)
             VALUES ($1, 'libre', 'Exercice', 'Pourquoi ?', '{}', $2)",
        )
        .bind(chapter)
        .bind(n + 1)
        .execute(&env.owner)
        .await
        .unwrap();
    }
    (id, slug)
}

/// An exam of `subject`, published or a draft, with a statement PDF or not;
/// its id and slug.
async fn exam(env: &Env, author: i64, subject: &str, published: bool, pdf: bool) -> (i64, String) {
    let slug = format!("examen-{}", unique());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.exams (titre, slug, type, matiere, niveau, statut, fichier_pdf,
                                    auteur_id, published_at, created_at, updated_at)
         VALUES ($1, $1, 'bac', $2, 'terminale', $3, $4, $5,
                 now() AT TIME ZONE 'UTC' - interval '1 hour',
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(&slug)
    .bind(subject)
    .bind(if published { "publie" } else { "brouillon" })
    .bind(pdf.then(|| format!("examens/{slug}.pdf")))
    .bind(author)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    (id, slug)
}

/// Removes what `author` planted, with its downloads and views: the
/// content tests of `http.rs` read the demo content only.
async fn remove_content(env: &Env, author: i64) {
    for statement in [
        "DELETE FROM contenu.telechargements WHERE telechargeable_type = 'exam' AND telechargeable_id IN
            (SELECT id FROM contenu.exams WHERE auteur_id = $1)",
        "DELETE FROM site.content_views v WHERE
            (kind = 'exam' AND content_id IN (SELECT id FROM contenu.exams WHERE auteur_id = $1))
            OR (kind = 'course' AND content_id IN (SELECT id FROM contenu.courses WHERE auteur_id = $1))",
        "DELETE FROM contenu.exams WHERE auteur_id = $1",
        "DELETE FROM contenu.courses WHERE auteur_id = $1",
    ] {
        sqlx::query(statement)
            .bind(author)
            .execute(&env.owner)
            .await
            .unwrap();
    }
}

/// A download of exam `id`, `days_ago` days ago.
async fn download(env: &Env, kind: &str, id: i64, days_ago: i32) {
    sqlx::query(
        "INSERT INTO contenu.telechargements (telechargeable_type, telechargeable_id, visiteur_hash, created_at)
         VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC' - make_interval(days => $4))",
    )
    .bind(kind)
    .bind(id)
    .bind(unique())
    .bind(days_ago)
    .execute(&env.owner)
    .await
    .unwrap();
}

/// A duel (or `mode`) started `days_ago` days ago, ended with `end_reason`
/// (none: still running). No seat: guests' matches count too.
async fn game(env: &Env, mode: &str, end_reason: Option<&str>, days_ago: i64) {
    let started = chrono::Utc::now() - chrono::Duration::days(days_ago);
    sqlx::query(
        "INSERT INTO game.matches
                (id, mode, ranked, subject, rules, seed, created_at, started_at, ended_at, end_reason)
         VALUES ($1, $2, false, 'physics', '{\"duration\": 60, \"target\": null, \"penalty\": false}',
                 1, $3, $3, CASE WHEN $4::text IS NULL THEN NULL ELSE $3 + interval '1 minute' END, $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(mode)
    .bind(started)
    .bind(end_reason)
    .execute(&env.owner)
    .await
    .unwrap();
}

fn series_of(data: &Value, name: &str) -> (i64, i64, i64) {
    let series = &data["series"][name];
    (
        series["values"][6].as_i64().unwrap(),
        series["total"].as_i64().unwrap(),
        series["previous"].as_i64().unwrap(),
    )
}

#[tokio::test]
async fn duels_and_pdf_downloads_are_counted_by_day() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let _downloads = DOWNLOADS.lock().await;
    let before = stats7(&env, &admin).await;

    // Two duels played to the end today, guests' included; one void, one
    // still running and a room match do not count; one in the week before.
    game(&env, "duel", Some("completed"), 0).await;
    game(&env, "duel", Some("forfeit"), 0).await;
    game(&env, "duel", Some("void"), 0).await;
    game(&env, "duel", None, 0).await;
    game(&env, "room", Some("completed"), 0).await;
    game(&env, "duel", Some("timeout"), 8).await;
    // Three exam downloads today, one the week before; a resource's not.
    let (id, _) = exam(&env, admin.id, "Mathématiques", true, true).await;
    for _ in 0..3 {
        download(&env, "exam", id, 0).await;
    }
    download(&env, "exam", id, 8).await;
    download(&env, "ressource", -id, 0).await;

    let after = stats7(&env, &admin).await;
    for (name, today, previous) in [("duels", 2, 1), ("pdf_downloads", 3, 1)] {
        let (b_today, b_total, b_previous) = series_of(&before, name);
        let (a_today, a_total, a_previous) = series_of(&after, name);
        assert_eq!(a_today - b_today, today, "{name}");
        assert_eq!(a_total - b_total, today, "{name}");
        assert_eq!(a_previous - b_previous, previous, "{name}");
        assert_eq!(after["series"][name]["values"].as_array().unwrap().len(), 7);
    }
    remove_content(&env, admin.id).await;
}

#[tokio::test]
async fn the_catalog_counts_published_content_by_subject() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let _downloads = DOWNLOADS.lock().await;
    let u = unique();
    let zeta = format!("Zeta {u}");
    // Two published courses of the subject (three exercises), a draft not.
    published_course(&env, admin.id, &format!("{zeta} – Analyse"), 2).await;
    published_course(&env, admin.id, &format!("{zeta} – Algèbre"), 1).await;
    sqlx::query(
        "INSERT INTO contenu.courses (titre, slug, description, theme, auteur_id)
         VALUES ($1, $1, 'Brouillon', $2, $3)",
    )
    .bind(format!("brouillon-{u}"))
    .bind(format!("{zeta} – Brouillon"))
    .bind(admin.id)
    .execute(&env.owner)
    .await
    .unwrap();
    // Exams of the same subject, spelt differently: two PDF, one without,
    // a draft with one (not counted).
    let (with_pdf, _) = exam(&env, admin.id, &format!("zétà {u}"), true, true).await;
    exam(&env, admin.id, &format!("ZETA-{u}"), true, true).await;
    exam(&env, admin.id, &zeta, true, false).await;
    let (draft, _) = exam(&env, admin.id, &zeta, false, true).await;
    for _ in 0..3 {
        download(&env, "exam", with_pdf, 0).await;
    }
    // Before the period, or of a draft: not counted.
    download(&env, "exam", with_pdf, 8).await;
    download(&env, "exam", draft, 0).await;
    // A subject of exams only, named after them.
    exam(&env, admin.id, &format!("Omega {u}"), true, true).await;
    // A subject with nothing published: absent.
    exam(&env, admin.id, &format!("Nada {u}"), false, true).await;

    let data = stats7(&env, &admin).await;
    let catalog = data["catalog"].as_array().unwrap();
    let find = |slug: &str| catalog.iter().position(|row| row["subject_slug"] == slug);
    let zeta_slug = format!("zeta-{u}");
    let zeta_at = find(&zeta_slug).unwrap_or_else(|| panic!("{zeta_slug} in {catalog:?}"));
    // The courses' name, though the exams spell it otherwise.
    assert_eq!(
        catalog[zeta_at],
        json!({
            "subject_slug": zeta_slug,
            "subject_name": zeta,
            "courses": 2,
            "exercises": 3,
            "pdfs": 2,
            "downloads": 3,
        })
    );
    let omega_at = find(&format!("omega-{u}")).unwrap();
    assert_eq!(
        catalog[omega_at],
        json!({
            "subject_slug": format!("omega-{u}"),
            "subject_name": format!("Omega {u}"),
            "courses": 0,
            "exercises": 0,
            "pdfs": 1,
            "downloads": 0,
        })
    );
    assert!(find(&format!("nada-{u}")).is_none());
    // Most downloaded first.
    assert!(zeta_at < omega_at);
    let downloads: Vec<i64> = catalog
        .iter()
        .map(|row| row["downloads"].as_i64().unwrap())
        .collect();
    assert!(downloads.windows(2).all(|w| w[0] >= w[1]), "{downloads:?}");
    remove_content(&env, admin.id).await;
}

#[tokio::test]
async fn the_most_viewed_content_is_compared_with_the_period_before() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    // Leftovers of an interrupted run would outrank this one's.
    sqlx::query("DELETE FROM site.content_views WHERE views >= 1000000")
        .execute(&env.owner)
        .await
        .unwrap();
    let u = unique();
    let (first, first_slug) = published_course(&env, admin.id, &format!("Top{u} – Un"), 0).await;
    let (third, _) = published_course(&env, admin.id, &format!("Top{u} – Deux"), 0).await;
    let (second, second_slug) = exam(&env, admin.id, &format!("Top {u}"), true, false).await;
    let (draft, _) = exam(&env, admin.id, &format!("Top {u}"), false, false).await;
    let views = |kind: &'static str, id: i64, days_ago: i32, views: i32| {
        let owner = env.owner.clone();
        async move {
            sqlx::query(
                "INSERT INTO site.content_views (kind, content_id, day, views)
                 VALUES ($1, $2, (now() AT TIME ZONE 'UTC')::date - $3, $4)",
            )
            .bind(kind)
            .bind(id)
            .bind(days_ago)
            .bind(views)
            .execute(&owner)
            .await
            .unwrap();
        }
    };
    views("course", first, 0, 1_000_000).await;
    views("course", first, 3, 2).await;
    views("course", first, 8, 5).await;
    views("exam", second, 0, 1_000_001).await;
    views("course", third, 1, 1_000_000).await;
    // No longer published: left out, whatever its views.
    views("exam", draft, 0, 2_000_000).await;

    let data = stats7(&env, &admin).await;
    let top = data["top_content"].as_array().unwrap();
    assert!(top.len() <= 10);
    assert_eq!(
        top[0],
        json!({
            "kind": "course",
            "slug": first_slug,
            "title": first_slug,
            "subject_name": format!("Top{u}"),
            "views": 1_000_002,
            "previous": 5,
        })
    );
    assert_eq!(top[1]["kind"], "exam");
    assert_eq!(top[1]["slug"], second_slug);
    assert_eq!(top[1]["subject_name"], format!("Top {u}"));
    assert_eq!(top[1]["previous"], 0);
    assert_eq!(top[2]["views"], 1_000_000);
    let views: Vec<i64> = top.iter().map(|r| r["views"].as_i64().unwrap()).collect();
    assert!(views.windows(2).all(|w| w[0] >= w[1]), "{views:?}");
    remove_content(&env, admin.id).await;
}

#[tokio::test]
async fn a_course_view_is_counted_once_in_a_while() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let reader = env.member().verified().create().await;
    let (id, slug) = published_course(&env, admin.id, "Vues – Cours", 0).await;
    let counts = || async {
        let vues: i64 = sqlx::query_scalar("SELECT vues FROM contenu.courses WHERE id = $1")
            .bind(id)
            .fetch_one(&env.db)
            .await
            .unwrap();
        let today: Option<i32> = sqlx::query_scalar(
            "SELECT views FROM site.content_views
              WHERE kind = 'course' AND content_id = $1 AND day = (now() AT TIME ZONE 'UTC')::date",
        )
        .bind(id)
        .fetch_optional(&env.db)
        .await
        .unwrap();
        (vues, today.unwrap_or(0))
    };
    let path = format!("/v1/courses/{slug}/view");
    let ip = fresh_ip();

    let (status, headers, body) = Call::post(&path).ip(&ip).send_full(&env.app).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(counts().await, (1, 1));
    // The same visitor again: a reload.
    let (status, _) = Call::post(&path).ip(&ip).send(&env.app).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(counts().await, (1, 1));
    // A member is someone else, and is counted once too.
    for _ in 0..2 {
        let (status, _) = Call::post(&path).member(&reader).send(&env.app).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    assert_eq!(counts().await, (2, 2));
    // A robot is not counted.
    let (status, _) = Call::post(&path)
        .user_agent("Mozilla/5.0 (compatible; Googlebot/2.1)")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(counts().await, (2, 2));

    // Only published courses.
    let (status, body) = Call::post(format!("/v1/courses/nope-{}/view", unique()))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    sqlx::query("UPDATE contenu.courses SET statut = 'brouillon' WHERE id = $1")
        .bind(id)
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, _) = Call::post(&path).send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // An exam's view, counted by its corrections, is in the same table.
    let (exam_id, exam_slug) = exam(&env, admin.id, "Vues", true, false).await;
    let (status, body) = Call::get(format!("/v1/exams/{exam_slug}/corrections"))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["views"], 1);
    let views: i32 = sqlx::query_scalar(
        "SELECT views FROM site.content_views WHERE kind = 'exam' AND content_id = $1",
    )
    .bind(exam_id)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(views, 1);
    remove_content(&env, admin.id).await;
}

#[tokio::test]
async fn http_traffic_is_kept_by_day_with_its_p95() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    // The only test that writes this table: it starts from nothing.
    sqlx::query("DELETE FROM site.http_daily")
        .execute(&env.owner)
        .await
        .unwrap();
    let data = stats7(&env, &admin).await;
    assert_eq!(data["http"]["available"], false);
    assert_eq!(data["http"]["since"], Value::Null);
    assert_eq!(data["http"]["p95_ms"]["total"], Value::Null);
    assert_eq!(
        data["http"]["p95_ms"]["values"],
        json!([null, null, null, null, null, null, null])
    );

    let daily = &env.state.http_daily;
    // Requests answered since the env started: the dashboard above.
    let today = chrono::Utc::now().date_naive();
    let counted = |daily: &pulsars_api::http_daily::HttpDaily| {
        daily.pending().get(&today).map_or(0, |d| d.requests)
    };
    assert_eq!(counted(daily), 1);
    let (status, _) = Call::get("/v1/admin/pending")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = Call::get("/nowhere").send(&env.app).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Probes are not traffic.
    Call::get("/healthz").send(&env.app).await;
    assert_eq!(counted(daily), 3);

    // A database that does not answer: the counts wait for the next flush.
    let down = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(1))
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:1/none")
        .unwrap();
    assert!(daily.flush(&down).await.is_err());
    assert_eq!(counted(daily), 3);
    daily.flush(&env.db).await.unwrap();
    assert!(daily.pending().is_empty());
    // Nothing new: nothing to write.
    daily.flush(&env.db).await.unwrap();

    // Another instance flushes today too, and wrote earlier days.
    let other = pulsars_api::http_daily::HttpDaily::default();
    other.record(
        StatusCode::INTERNAL_SERVER_ERROR,
        std::time::Duration::from_millis(2),
    );
    other.flush(&env.db).await.unwrap();
    let mut slow = vec![0i64; 16];
    slow[6] = 100; // 100 requests between 100 and 150 ms
    let mut fast = vec![0i64; 16];
    fast[0] = 20;
    sqlx::query(
        "INSERT INTO site.http_daily (day, requests, errors_5xx, buckets)
         VALUES ((now() AT TIME ZONE 'UTC')::date - 1, 100, 3, $1),
                ((now() AT TIME ZONE 'UTC')::date - 10, 20, 0, $2)",
    )
    .bind(&slow)
    .bind(&fast)
    .execute(&env.owner)
    .await
    .unwrap();

    let data = stats7(&env, &admin).await;
    let http = &data["http"];
    assert_eq!(http["available"], true);
    let ten_days_ago = (today - chrono::Days::new(10))
        .format("%Y-%m-%d")
        .to_string();
    assert_eq!(http["since"], ten_days_ago);
    // Today: the three requests of this env and the other's one.
    assert_eq!(http["requests"]["values"][6], 4);
    assert_eq!(http["requests"]["values"][5], 100);
    assert_eq!(http["requests"]["total"], 104);
    assert_eq!(http["requests"]["previous"], 20);
    assert_eq!(http["errors_5xx"]["values"][6], 1);
    assert_eq!(http["errors_5xx"]["values"][5], 3);
    assert_eq!(http["errors_5xx"]["total"], 4);
    let p95 = &http["p95_ms"];
    // 95 of 100 between 100 and 150 ms: 147.5, rounded.
    assert_eq!(p95["values"][5], 148);
    assert!(p95["values"][6].is_i64());
    assert_eq!(p95["values"][0], Value::Null);
    // Over the week, from the summed buckets.
    let total = p95["total"].as_i64().unwrap();
    assert!((100..=150).contains(&total), "{total}");
    assert_eq!(p95["previous"], 5);
    let buckets: Vec<i64> =
        sqlx::query_scalar("SELECT buckets FROM site.http_daily WHERE day = $1")
            .bind(today)
            .fetch_one(&env.db)
            .await
            .unwrap();
    assert_eq!(buckets.len(), 16);
    assert_eq!(buckets.iter().sum::<i64>(), 4);
}

// --- Containers -------------------------------------------------------------

#[tokio::test]
async fn the_containers_come_from_the_podman_snapshot() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let (status, body) = Call::get("/v1/admin/containers")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"],
        json!({"available": false, "reason": "not_configured", "taken_at": null,
               "stale": false, "containers": []})
    );

    let dir = std::env::temp_dir().join(format!("pulsars-podman-{}", unique()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("releve.json");
    let Some(env) = Env::with(&[("PODMAN_SNAPSHOT_PATH", path.to_str().unwrap())]).await else {
        return;
    };
    let admin = self::admin(&env).await;
    let get = || async {
        let (status, body) = Call::get("/v1/admin/containers")
            .member(&admin)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["data"].clone()
    };
    assert_eq!(get().await["reason"], "missing");

    std::fs::write(&path, b"{\"taken_at\": ").unwrap();
    assert_eq!(get().await["reason"], "unreadable");

    // A fresh podman 5 snapshot.
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut snapshot: Value =
        serde_json::from_str(include_str!("fixtures/podman/podman5.json")).unwrap();
    snapshot["taken_at"] = Value::from(now.clone());
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let data = get().await;
    assert_eq!(data["available"], true);
    assert_eq!(data["reason"], Value::Null);
    assert_eq!(data["taken_at"], now);
    assert_eq!(data["stale"], false);
    let names: Vec<&str> = data["containers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "pulsars-migrate",
            "pulsars-web",
            "pulsars-api",
            "pulsars-db",
            "umami"
        ]
    );
    assert_eq!(
        data["containers"][1],
        json!({
            "name": "pulsars-web",
            "image": "ghcr.io/pulsars-science/pulsars-web:0.1.0",
            "state": "running",
            "health": "unhealthy",
            "started_at": "2026-10-29T09:15:50Z",
            "restarts": 1,
            "cpu_percent": 12.5,
            "memory_bytes": 157286400,
            "memory_limit_bytes": 536870912,
        })
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn the_game_leaderboard_takes_an_admin_session() {
    let Some(env) = Env::new().await else { return };
    let admin = admin(&env).await;
    let (status, headers, body) = Call::get("/v1/admin/game/leaderboard?subject=maths&limit=5")
        .member(&admin)
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(body["subject"], "maths");
    let leaders = body["data"].as_array().unwrap();
    assert!(leaders.len() <= 5);
    // The same body as the public route.
    let (_, public) = Call::get("/v1/game/leaderboard?subject=maths&limit=5")
        .send(&env.app)
        .await;
    assert_eq!(body, public);
    let (status, body) = Call::get("/v1/admin/game/leaderboard?subject=biology")
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
