//! Private ESGI content, imports and role isolation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, Part};
use serde_json::{Value, json};

async fn set_esgi_email(env: &Env, member: &common::TestMember) {
    sqlx::query("UPDATE site.users SET email = $2 WHERE id = $1")
        .bind(member.id)
        .bind(format!("{}@myskolae.fr", member.pseudo))
        .execute(&env.owner)
        .await
        .unwrap();
}

fn document() -> Value {
    json!({
        "title": "Révision Web API",
        "description": "REST et HTTP",
        "quiz": [{
            "id": "q1",
            "question": "Que signifie Stateless ?",
            "options": {"a": "Sans base", "b": "Chaque requête est complète"},
            "answer": "b",
            "explanation": "Le serveur ne garde pas de contexte de session."
        }],
        "flashcards": [{
            "id": "q1",
            "front": "Stateless",
            "back": "Chaque requête porte son contexte."
        }]
    })
}

#[tokio::test]
async fn esgi_reading_needs_the_exact_email_domain() {
    let Some(env) = Env::new().await else { return };
    let outsider = env.member().verified().create().await;
    let student = env.member().verified().create().await;
    set_esgi_email(&env, &student).await;

    let (status, _) = Call::get("/v1/esgi/subjects")
        .member(&outsider)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = Call::get("/v1/esgi/subjects")
        .member(&student)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 9);
    assert_eq!(body["data"][0]["slug"], "web-api");
}

#[tokio::test]
async fn admins_and_the_esgi_role_read_without_a_school_address() {
    let Some(env) = Env::new().await else { return };
    let admin = env.member().verified().role("admin").create().await;
    let guest = env.member().verified().role("esgi").create().await;

    for member in [&admin, &guest] {
        let (status, body) = Call::get("/v1/esgi/subjects")
            .member(member)
            .send(&env.app)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

#[tokio::test]
async fn an_admin_gives_the_esgi_role() {
    let Some(env) = Env::new().await else { return };
    let admin = env.member().verified().role("admin").create().await;
    let guest = env.member().verified().create().await;

    let (status, body) = Call::put(format!("/v1/admin/members/{}/roles/esgi", guest.id))
        .member(&admin)
        .send(&env.app)
        .await;
    assert!(status.is_success(), "{status} {body}");

    let (status, body) = Call::get("/v1/esgi/subjects")
        .member(&guest)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn only_admin_esgi_imports_a_valid_json_and_pdf() {
    let Some(env) = Env::new().await else { return };
    let student = env.member().verified().create().await;
    set_esgi_email(&env, &student).await;
    let admin = env.member().verified().role("admin_esgi").create().await;
    let json = serde_json::to_vec(&document()).unwrap();
    let pdf = b"%PDF-1.7\nESGI";

    let (status, _) = Call::post("/v1/esgi/admin/courses")
        .member(&student)
        .multipart(vec![
            Part::text("subject", "web-api"),
            Part::file("json", "cours.json", &json),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = Call::post("/v1/esgi/admin/courses")
        .member(&admin)
        .multipart(vec![
            Part::text("subject", "web-api"),
            Part::file("json", "cours.json", &json),
            Part::file("pdf", "cours.pdf", pdf),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let slug = body["data"]["slug"].as_str().unwrap();
    assert_eq!(body["data"]["quiz"][0]["answer"], "b");
    assert!(body["data"]["pdf_url"].is_string());

    let (status, body) = Call::get(format!("/v1/esgi/courses/{slug}"))
        .member(&student)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["flashcards"][0]["front"], "Stateless");

    let (status, headers, _) = Call::get(format!("/v1/esgi/courses/{slug}/pdf"))
        .member(&student)
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "application/pdf");

    let (status, _) = Call::delete(format!("/v1/esgi/admin/courses/{slug}"))
        .member(&admin)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn import_rejects_an_answer_outside_the_options() {
    let Some(env) = Env::new().await else { return };
    let admin = env.member().verified().role("admin_esgi").create().await;
    let mut invalid = document();
    invalid["quiz"][0]["answer"] = json!("z");
    let json = serde_json::to_vec(&invalid).unwrap();

    let (status, body) = Call::post("/v1/esgi/admin/courses")
        .member(&admin)
        .multipart(vec![
            Part::text("subject", "web-api"),
            Part::file("json", "cours.json", &json),
        ])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["errors"][0]["parameter"], "quiz.0.answer");
}
