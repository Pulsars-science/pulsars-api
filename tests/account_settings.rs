//! The member's settings (`src/routes/profile.rs`): profile, welcome steps,
//! language, avatar. Ported from Laravel's `CompteTest`, `OnboardingTest`,
//! `CerveauxTest` (the profile's choice) and `LocalisationTest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, Part, codes, png};
use serde_json::{Value, json};

async fn subject_roles(env: &Env, user_id: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT r.slug::text FROM site.role_user ru JOIN site.roles r ON r.id = ru.role_id
          WHERE ru.user_id = $1 ORDER BY r.slug",
    )
    .bind(user_id)
    .fetch_all(&env.owner)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_profile_is_shown_and_updated() {
    let Some(env) = Env::new().await else { return };
    let ada = env
        .member()
        .verified()
        .role("admin")
        .role("physique")
        .create()
        .await;

    let (status, body) = Call::get("/v1/me/profile")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["pseudo"], ada.pseudo.as_str());
    assert_eq!(data["email"], ada.email.as_str());
    assert_eq!(data["interests"], json!(["physique"]));
    assert_eq!(data["roles"][0]["slug"], "admin");
    assert_eq!(data["has_password"], false);
    assert_eq!(data["brains_public"], true);
    assert_eq!(data["linked_accounts"], json!([]));

    let pseudo = format!("n{}", common::unique());
    let (status, body) = Call::patch("/v1/me/profile")
        .member(&ada)
        .json(json!({
            "pseudo": pseudo, "bio": "  Maths et café.  ", "goals": ["curiosite", "rattraper", "curiosite"],
            "school_level": "lycee", "class": "Terminale",
            "interests": ["chimie", "super-admin", "nope"], "brains_public": false,
        }))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["pseudo"], pseudo.as_str());
    assert_eq!(data["bio"], "Maths et café.");
    assert_eq!(data["goals"], json!(["curiosite", "rattraper"]));
    assert_eq!(data["brains_public"], false);
    // Subjects follow the list; permissions and unknown slugs are untouched.
    assert_eq!(subject_roles(&env, ada.id).await, vec!["admin", "chimie"]);
    let name: String = sqlx::query_scalar("SELECT name::text FROM site.users WHERE id = $1")
        .bind(ada.id)
        .fetch_one(&env.owner)
        .await
        .unwrap();
    assert_eq!(name, pseudo);

    // The whole form, as Laravel's: what is left out is cleared, but the
    // brains' visibility stays.
    let (_, body) = Call::patch("/v1/me/profile")
        .member(&ada)
        .json(json!({"pseudo": pseudo}))
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["bio"], Value::Null);
    assert_eq!(body["data"]["interests"], json!([]));
    assert_eq!(body["data"]["brains_public"], false);
}

#[tokio::test]
async fn the_profile_is_validated() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let other = env.member().verified().create().await;
    let (status, body) = Call::patch("/v1/me/profile")
        .member(&ada)
        .json(
            json!({"pseudo": other.pseudo.to_uppercase(), "bio": "x".repeat(281),
                     "goals": ["voyager"], "school_level": "maternelle", "class": "x".repeat(61)}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        [
            ("pseudo", "taken"),
            ("bio", "too_long"),
            ("goals/0", "invalid"),
            ("school_level", "invalid"),
            ("class", "too_long")
        ]
        .map(|(f, c)| (f.to_owned(), c.to_owned()))
        .to_vec()
    );
    let (_, body) = Call::patch("/v1/me/profile")
        .member(&ada)
        .json(json!({"pseudo": "a b"}))
        .send(&env.app)
        .await;
    assert_eq!(
        codes(&body),
        vec![("pseudo".to_owned(), "invalid_format".to_owned())]
    );
    // Keeping one's own pseudo is fine.
    let (status, _) = Call::patch("/v1/me/profile")
        .member(&ada)
        .json(json!({"pseudo": ada.pseudo}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    let fresh = env.member().create().await;
    let (status, _) = Call::get("/v1/me/profile")
        .member(&fresh)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_welcome_steps_need_goals_and_level() {
    let Some(env) = Env::new().await else { return };
    // Before the address is verified, even.
    let ada = env.member().create().await;
    let (status, body) = Call::post("/v1/me/onboarding")
        .member(&ada)
        .json(json!({"goals": []}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![
            ("goals".to_owned(), "required".to_owned()),
            ("school_level".to_owned(), "required".to_owned())
        ]
    );
    let (status, _) = Call::post("/v1/me/onboarding")
        .member(&ada)
        .json(
            json!({"goals": ["reviser-examen"], "school_level": "college",
                     "interests": ["mathematiques", "admin"]}),
        )
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = Call::get("/v1/auth/session")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["user"]["onboarded"], true);
    // A subject is added, never a permission.
    assert_eq!(subject_roles(&env, ada.id).await, vec!["mathematiques"]);
}

#[tokio::test]
async fn the_account_keeps_its_language() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().create().await;
    let (status, body) = Call::put("/v1/me/locale")
        .member(&ada)
        .json(json!({"locale": "de"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body)[0].1, "invalid");
    let (status, _) = Call::put("/v1/me/locale")
        .member(&ada)
        .json(json!({"locale": "ru"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = Call::get("/v1/auth/session")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["user"]["locale"], "ru");
}

#[tokio::test]
async fn an_avatar_is_stored_in_the_database_and_served_publicly() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (status, body) = Call::put("/v1/me/avatar")
        .member(&ada)
        .multipart(vec![Part::file("avatar", "moi.png", &png(64, 64))])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let url = body["data"]["avatar_url"].as_str().unwrap().to_owned();
    let path = url.split("/storage/").nth(1).unwrap().to_owned();
    assert!(
        path.starts_with("avatars/") && path.ends_with(".png"),
        "{path}"
    );

    let (status, headers, _) = Call::get(format!("/v1/storage/{path}"))
        .send_full(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/png");
    assert!(
        headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; sandbox"
    );

    // Not an image, too large: refused.
    let (status, body) = Call::put("/v1/me/avatar")
        .member(&ada)
        .multipart(vec![Part::file(
            "avatar",
            "x.png",
            b"<svg onload=alert(1)>",
        )])
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(codes(&body)[0].1, "invalid_image");
    let (_, body) = Call::put("/v1/me/avatar")
        .member(&ada)
        .multipart(vec![Part::file("avatar", "x.png", &png(7000, 10))])
        .send(&env.app)
        .await;
    assert_eq!(codes(&body)[0].1, "too_large_dimensions");
    let (status, _) = Call::put("/v1/me/avatar")
        .member(&ada)
        .json(json!({"avatar": "x"}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // Removed: the file goes too.
    let (status, _) = Call::delete("/v1/me/avatar")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = Call::get(format!("/v1/storage/{path}"))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The private disk is never served.
    let (status, _) = Call::get("/v1/storage/examens/bac-2026-maths-jour-1.pdf")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_forms_list_the_subject_roles() {
    let Some(env) = Env::new().await else { return };
    let (status, body) = Call::get("/v1/roles?category=matiere").send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let roles = body["data"].as_array().unwrap();
    assert!(roles.iter().all(|r| r["category"] == "matiere"));
    assert!(roles.iter().any(|r| r["slug"] == "physique"));
    let (status, _) = Call::get("/v1/roles?category=nope").send(&env.app).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn me_answers_the_session_with_the_account_state() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (status, body) = Call::get("/v1/me").member(&ada).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["id"], ada.id.to_string());
    assert_eq!(data["email"], ada.email.as_str());
    assert_eq!(
        (
            data["email_verified"].clone(),
            data["onboarded"].clone(),
            data["locale"].clone()
        ),
        (json!(true), json!(true), json!("fr"))
    );
    assert_eq!(data["forum_muted_until"], Value::Null);
    assert_eq!(data["brains"], 0);
    let (status, _) = Call::get("/v1/me").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
