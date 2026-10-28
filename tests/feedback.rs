//! Reports and captcha, suggestions, proposed resources, the donations goal
//! and the exam page's corrections and counters (`src/routes/feedback.rs`,
//! `corrections.rs`, `files.rs`). Ported from Laravel's `SignalementTest`,
//! `SuggestionTest`, `DemandeRessourceTest`, `ObjectifDonsTest`,
//! `ExamenTest` and `TelechargementsTest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, Part, codes, fresh_ip, png, unique};
use serde_json::{Value, json};

const BAC: &str = "baccalaureat-general-2026-mathematiques-specialite-jour-1";

async fn reports(env: &Env, message: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM site.signalements WHERE message = $1")
        .bind(message)
        .fetch_one(&env.owner)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_member_reports_a_mistake_once() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let message = format!("Une faute dans la leçon {}", unique());
    let body = json!({"target_type": "lesson", "target_id": 2, "type": "faute",
                      "message": message, "location": "deuxième paragraphe"});
    let (status, _) = Call::post("/v1/reports")
        .member(&ada)
        .json(body.clone())
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // A double click: one report.
    let (status, _) = Call::post("/v1/reports")
        .member(&ada)
        .json(body)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(reports(&env, &message).await, 1);
    let (who, location): (Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT user_id, emplacement::text FROM site.signalements WHERE message = $1",
    )
    .bind(&message)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(
        (who, location.as_deref()),
        (Some(ada.id), Some("deuxième paragraphe"))
    );

    let (status, body) = Call::post("/v1/reports")
        .member(&ada)
        .json(json!({"target_type": "user", "target_id": 1, "type": "faute", "message": "court"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![
            ("target_type".to_owned(), "invalid".to_owned()),
            ("message".to_owned(), "too_short".to_owned())
        ]
    );
    let (_, body) = Call::post("/v1/reports")
        .member(&ada)
        .json(
            json!({"target_type": "course", "target_id": 99999, "type": "faute",
                     "message": "Ce cours n'existe pas du tout."}),
        )
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        vec![("message".to_owned(), "target_missing".to_owned())]
    );
}

#[tokio::test]
async fn a_visitor_reports_with_a_captcha_used_once() {
    let Some(env) = Env::new().await else { return };
    let ip = fresh_ip();
    let message = format!("Une erreur de calcul {}", unique());
    let report = |token: Value, answer: Value| {
        json!({"target_type": "exercise", "target_id": 1, "type": "faute", "message": message,
               "captcha_token": token, "captcha_answer": answer})
    };
    let (status, body) = Call::post("/v1/reports")
        .ip(&ip)
        .json(
            json!({"target_type": "exercise", "target_id": 1, "type": "faute", "message": message}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body)[0],
        ("captcha_token".to_owned(), "required".to_owned())
    );

    let (status, captcha) = Call::get("/v1/captcha").ip(&ip).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{captcha}");
    let (a, b) = (
        captcha["data"]["a"].as_u64().unwrap(),
        captcha["data"]["b"].as_u64().unwrap(),
    );
    assert!((1..=9).contains(&a) && (1..=9).contains(&b));
    assert_eq!(
        captcha["data"]["question"],
        format!("Combien font {a} + {b} ?")
    );
    let token = captcha["data"]["token"].clone();
    assert!(
        !captcha.to_string().contains(&format!("\"{}\"", a + b)),
        "the answer is not given"
    );

    let (status, body) = Call::post("/v1/reports")
        .ip(&ip)
        .json(report(token.clone(), json!((a + b + 1).to_string())))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![("captcha_answer".to_owned(), "incorrect".to_owned())]
    );

    // A wrong answer leaves the token usable; the right one spends it.
    let (status, _) = Call::post("/v1/reports")
        .ip(&ip)
        .json(report(token.clone(), json!(format!(" {} ", a + b))))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(reports(&env, &message).await, 1);
    let (status, body) = Call::post("/v1/reports")
        .ip(&ip)
        .json(report(token, json!((a + b).to_string())))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![("captcha_answer".to_owned(), "used".to_owned())]
    );
    let author: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM site.signalements WHERE message = $1")
            .bind(&message)
            .fetch_one(&env.owner)
            .await
            .unwrap();
    assert_eq!(author, None);
}

#[tokio::test]
async fn a_member_sends_a_suggestion_with_or_without_an_image() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let message = format!("Un mode sombre plus contrasté {}", unique());
    let (status, body) = Call::post("/v1/suggestions")
        .member(&ada)
        .multipart(vec![Part::text("message", &message)])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = Call::post("/v1/suggestions")
        .member(&ada)
        .multipart(vec![
            Part::text("title", "Idée"),
            Part::text("message", &message),
            Part::file("image", "capture.png", &png(800, 600)),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let image: Option<String> =
        sqlx::query_scalar("SELECT image::text FROM site.suggestions WHERE id = $1")
            .bind(body["data"]["id"].as_i64().unwrap())
            .fetch_one(&env.owner)
            .await
            .unwrap();
    let image = image.unwrap();
    assert!(
        image.starts_with("suggestions/") && image.ends_with(".png"),
        "{image}"
    );
    let (status, _) = Call::get(format!("/v1/storage/{image}"))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = Call::post("/v1/suggestions")
        .member(&ada)
        .multipart(vec![
            Part::text("message", "court"),
            Part::file("image", "x.pdf", b"%PDF-1.4"),
        ])
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        vec![
            ("message".to_owned(), "too_short".to_owned()),
            ("image".to_owned(), "invalid_image".to_owned())
        ]
    );
    let (status, _) = Call::post("/v1/suggestions")
        .multipart(vec![Part::text("message", &message)])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_member_proposes_a_pdf_resource() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let title = format!("Fiches de révision {}", unique());
    let (status, body) = Call::post("/v1/resource-requests")
        .member(&ada)
        .multipart(vec![
            Part::text("title", &title),
            Part::text("author_name", "Ada L."),
            Part::text("license", "cc-by-4.0"),
            Part::text("license_custom", "ignored"),
            Part::text("lang", "fr"),
            Part::file("file", "fiches.pdf", b"%PDF-1.7 fiches"),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["status"], "en_attente");
    let (path, original, size, custom): (String, String, i64, Option<String>) = sqlx::query_as(
        "SELECT fichier::text, nom_fichier_original::text, taille, licence_personnalisee::text
           FROM site.demandes_ressources WHERE titre = $1",
    )
    .bind(&title)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert!(path.starts_with("demandes-ressources/"));
    assert_eq!((original.as_str(), size, custom), ("fiches.pdf", 15, None));
    let stored: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM contenu.fichiers WHERE disque = 'local' AND chemin = $1)",
    )
    .bind(&path)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert!(stored);

    let (status, body) = Call::post("/v1/resource-requests")
        .member(&ada)
        .multipart(vec![
            Part::text("title", &title),
            Part::text("author_name", "Ada"),
            Part::text("license", "autre"),
            Part::file("file", "page.html", b"<html>"),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![
            ("license_custom".to_owned(), "required".to_owned()),
            ("file".to_owned(), "not_pdf".to_owned())
        ]
    );
    let (_, body) = Call::post("/v1/resource-requests")
        .member(&ada)
        .multipart(vec![Part::text("title", &title)])
        .send(&env.app)
        .await;
    let fields: Vec<String> = codes(&body).into_iter().map(|(f, _)| f).collect();
    assert_eq!(fields, vec!["author_name", "license", "file"]);

    let (_, body) = Call::get("/v1/licenses").send(&env.app).await;
    let licenses = body["data"].as_array().unwrap();
    assert_eq!(licenses.last().unwrap()["key"], "autre");
    assert_eq!(licenses.last().unwrap()["free"], true);
}

#[tokio::test]
async fn the_donations_goal_is_null_until_set() {
    let Some(env) = Env::new().await else { return };
    // The only test that writes this setting.
    sqlx::query("DELETE FROM site.reglages WHERE cle = 'objectif_dons'")
        .execute(&env.owner)
        .await
        .unwrap();
    let (status, body) = Call::get("/v1/settings/donation-goal").send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"data": null}));
    for (value, expected) in [
        (
            json!({"montant": 500, "collecte": 120.5}),
            json!({"percent": 24, "raised": 120.5, "goal": 500.0, "currency": "EUR"}),
        ),
        (
            json!({"montant": 100, "collecte": 250}),
            json!({"percent": 100, "raised": 250.0, "goal": 100.0, "currency": "EUR"}),
        ),
        (json!({"montant": 0, "collecte": 10}), Value::Null),
        (
            json!({"montant": 100, "collecte": -5}),
            json!({"percent": 0, "raised": 0.0, "goal": 100.0, "currency": "EUR"}),
        ),
    ] {
        sqlx::query(
            "INSERT INTO site.reglages (cle, valeur) VALUES ('objectif_dons', $1)
             ON CONFLICT (cle) DO UPDATE SET valeur = EXCLUDED.valeur",
        )
        .bind(sqlx::types::Json(&value))
        .execute(&env.owner)
        .await
        .unwrap();
        let (_, body) = Call::get("/v1/settings/donation-goal").send(&env.app).await;
        assert_eq!(body["data"], expected, "{value}");
    }
    sqlx::query("DELETE FROM site.reglages WHERE cle = 'objectif_dons'")
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_exam_page_shows_corrections_and_counts_views() {
    let Some(env) = Env::new().await else { return };
    let ip = fresh_ip();
    let path = format!("/v1/exams/{BAC}/corrections");
    let (status, body) = Call::get(format!("{path}?lang=en"))
        .ip(&ip)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["parts"][0]["part_id"], 46);
    assert_eq!(data["parts"][0]["correction_md"], "CORRIGE-MARKER-EN-46");
    assert_eq!(data["parts"][0]["lang"], "en");
    assert_eq!(data["parts"][1]["correction_md"], "CORRIGE-MARKER-47");
    assert_eq!(data["has_correction_pdf"], true);
    let views = data["views"].as_i64().unwrap();
    // A reload right away is the same view; another reader is not.
    let (_, again) = Call::get(&path).ip(&ip).send(&env.app).await;
    assert_eq!(again["data"]["views"], views);
    let ada = env.member().create().await;
    let (_, other) = Call::get(&path).member(&ada).send(&env.app).await;
    assert!(other["data"]["views"].as_i64().unwrap() > views);

    // Drafts keep their corrections, and statements never carry them.
    let (status, _) = Call::get(
        "/v1/exams/diplome-national-du-brevet-2026-mathematiques-serie-generale/corrections",
    )
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, statement) = Call::get(format!("/v1/exams/{BAC}")).send(&env.app).await;
    assert!(!statement.to_string().contains("CORRIGE"));
}

#[tokio::test]
async fn downloads_count_people_and_not_robots() {
    let Some(env) = Env::new().await else { return };
    let counters = |env: &Env| {
        let pool = env.owner.clone();
        async move {
            let row: (i64, i64) = sqlx::query_as(
                "SELECT telechargements, (SELECT count(*) FROM contenu.telechargements
                                           WHERE telechargeable_type = 'exam' AND telechargeable_id = 2)
                   FROM contenu.exams WHERE id = 2",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            row
        }
    };
    let browser = "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/126";
    let ip = fresh_ip();
    let before = counters(&env).await;

    // A preview is not a download.
    let (status, headers, _) = Call::get(format!("/v1/exams/{BAC}/pdf"))
        .ip(&ip)
        .user_agent(browser)
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "public, max-age=3600");
    assert_eq!(counters(&env).await, before);

    // Twice the statement, once the correction: three downloads, one person.
    for file in ["pdf", "pdf", "correction-pdf"] {
        let (status, headers, _) =
            Call::get(format!("/v1/exams/{BAC}/{file}?disposition=attachment"))
                .ip(&ip)
                .user_agent(browser)
                .send_full(&env.app)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["cache-control"], "private, no-store");
    }
    let after = counters(&env).await;
    assert_eq!((after.0 - before.0, after.1 - before.1), (3, 1));
    let (_, headers, _) = Call::get(format!(
        "/v1/exams/{BAC}/correction-pdf?disposition=attachment"
    ))
    .ip(&ip)
    .user_agent(browser)
    .send_full(&env.app)
    .await;
    assert_eq!(
        headers["content-disposition"],
        format!("attachment; filename=\"{BAC}-corrige.pdf\"").as_str()
    );

    // A robot: nothing.
    let now = counters(&env).await;
    Call::get(format!("/v1/exams/{BAC}/pdf?disposition=attachment"))
        .ip(&fresh_ip())
        .user_agent("Googlebot/2.1 (+http://www.google.com/bot.html)")
        .send(&env.app)
        .await;
    assert_eq!(counters(&env).await, now);

    // No correction PDF for a draft.
    let (status, _) = Call::get(
        "/v1/exams/diplome-national-du-brevet-2026-mathematiques-serie-generale/correction-pdf",
    )
    .send(&env.app)
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forms_have_an_hourly_cadence_and_file_limits() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    for i in 0..10 {
        let (status, _) = Call::post("/v1/suggestions")
            .member(&ada)
            .multipart(vec![Part::text("message", &format!("Une idée numéro {i}"))])
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = Call::post("/v1/suggestions")
        .member(&ada)
        .multipart(vec![Part::text("message", "Une idée de trop")])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![("message".to_owned(), "too_many".to_owned())]
    );

    // Just over 20 MiB: the field's 422; far over: 413.
    let mut big = b"%PDF-1.4 ".to_vec();
    big.resize(20 * 1024 * 1024 + 10, b'x');
    let (status, body) = Call::post("/v1/resource-requests")
        .member(&ada)
        .multipart(vec![
            Part::text("title", "Gros"),
            Part::text("author_name", "Ada"),
            Part::text("license", "cc0-1.0"),
            Part::file("file", "gros.pdf", &big),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![("file".to_owned(), "too_large".to_owned())]
    );
    big.resize(22 * 1024 * 1024, b'x');
    let (status, body) = Call::post("/v1/resource-requests")
        .member(&ada)
        .multipart(vec![Part::file("file", "gros.pdf", &big)])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
}
