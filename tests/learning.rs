//! Progress, exercises, XP and brains of the members (`src/routes/learning.rs`,
//! `practice.rs`, `dashboard.rs`), ported from Laravel's `ProgressionTest`,
//! `CerveauxTest`, `ExerciceTest`, `ExperienceExerciceTest`,
//! `ExercicesCatalogueTest`, `AccueilClassementTest` and the dashboard's.
//! On the demo content of `dev/db/30-demo-data.sql`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{Call, Env, codes};
use serde_json::{Value, json};

/// Lessons of `les-bases-du-calcul`, in reading order.
const BASICS: [i64; 4] = [1, 2, 3, 4];
/// The only lesson of `chimie-le-tableau-periodique`.
const CHEMISTRY_LESSON: i64 = 5;

async fn complete(env: &Env, token: &str, lesson: i64, completed: bool) -> (StatusCode, Value) {
    Call::put(format!("/v1/me/lessons/{lesson}/completion"))
        .session(token)
        .json(json!({ "completed": completed }))
        .send(&env.app)
        .await
}

async fn brains(env: &Env, user_id: i64) -> Vec<(String, i32)> {
    sqlx::query_as(
        "SELECT cle::text, montant FROM site.cerveaux_mouvements WHERE user_id = $1 ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&env.owner)
    .await
    .unwrap()
}

#[tokio::test]
async fn completing_a_lesson_updates_progress_and_pays_once() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;

    let (status, body) = complete(&env, &ada.token, BASICS[0], true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["completed"], true);
    assert_eq!(
        body["data"]["progress"],
        json!({"done": 1, "total": 4, "percent": 25})
    );
    assert_eq!(body["data"]["course_completed"], false);
    assert_eq!(body["rewards"], json!({"xp": 10, "brains": 5}));

    // Twice: one row, nothing more earned.
    let (_, body) = complete(&env, &ada.token, BASICS[0], true).await;
    assert_eq!(body["data"]["progress"]["done"], 1);
    assert_eq!(body["rewards"], json!({"xp": null, "brains": null}));

    // Taken back then done again: the XP comes back, the brains do not.
    let (_, body) = complete(&env, &ada.token, BASICS[0], false).await;
    assert_eq!(body["data"]["progress"]["done"], 0);
    let (_, body) = complete(&env, &ada.token, BASICS[0], true).await;
    assert_eq!(body["rewards"], json!({"xp": 10, "brains": null}));
    assert_eq!(brains(&env, ada.id).await, vec![("lecon:1".to_owned(), 5)]);

    // Enrolled, last lesson recorded.
    let last: Option<i64> = sqlx::query_scalar(
        "SELECT derniere_lesson_id FROM site.enrollments WHERE user_id = $1 AND course_id = 1",
    )
    .bind(ada.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert_eq!(last, Some(BASICS[0]));
}

#[tokio::test]
async fn a_course_is_completed_at_100_percent_until_a_lesson_is_taken_back() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (_, body) = complete(&env, &ada.token, CHEMISTRY_LESSON, true).await;
    assert_eq!(body["data"]["progress"]["percent"], 100);
    assert_eq!(body["data"]["course_completed"], true);
    // The course's 40 XP count in the member's level.
    let xp: i32 = sqlx::query_scalar("SELECT total FROM site.member_xp WHERE user_id = $1")
        .bind(ada.id)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(xp, 50);

    let (_, body) = complete(&env, &ada.token, CHEMISTRY_LESSON, false).await;
    assert_eq!(body["data"]["course_completed"], false);
}

#[tokio::test]
async fn the_course_progress_resumes_at_the_first_lesson_not_done() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let path = "/v1/me/courses/les-bases-du-calcul/progress";

    let (status, body) = Call::get(path).member(&ada).send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["enrolled"], false);
    assert_eq!(
        body["data"]["resume_lesson"]["slug"],
        "priorites-operatoires"
    );

    complete(&env, &ada.token, BASICS[0], true).await;
    complete(&env, &ada.token, BASICS[2], true).await;
    let (_, body) = Call::get(format!("{path}?lang=en"))
        .member(&ada)
        .send(&env.app)
        .await;
    let data = &body["data"];
    assert_eq!(data["enrolled"], true);
    assert_eq!(data["completed_lesson_ids"], json!([1, 3]));
    assert_eq!(
        (data["done"].clone(), data["percent"].clone()),
        (json!(2), json!(50))
    );
    assert_eq!(
        data["resume_lesson"],
        json!({"slug": "les-fractions", "title": "Fractions", "lang": "en"})
    );

    let (_, body) = Call::get("/v1/me/progress")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(
        body["data"],
        json!({"les-bases-du-calcul": {"done": 2, "total": 4, "percent": 50}})
    );

    let (status, _) = Call::get("/v1/me/courses/demo-draft-course/progress")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_verified_members_mark_lessons() {
    let Some(env) = Env::new().await else { return };
    let (status, _) = Call::put("/v1/me/lessons/1/completion")
        .json(json!({"completed": true}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let fresh = env.member().create().await;
    let (status, _) = complete(&env, &fresh.token, 1, true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let ada = env.member().verified().create().await;
    // A lesson of a draft course does not exist for members.
    let (status, _) = complete(&env, &ada.token, 6, true).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// --- Exercises ---------------------------------------------------------------

async fn answer(env: &Env, token: &str, id: i64, body: Value) -> (StatusCode, Value) {
    Call::post(format!("/v1/exercises/{id}/attempts"))
        .session(token)
        .json(body)
        .send(&env.app)
        .await
}

#[tokio::test]
async fn the_solution_stays_hidden_until_an_attempt() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (status, body) = Call::get("/v1/me/exercises/1")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["attempt_count"], 0);
    assert_eq!(body["data"]["solution_md"], Value::Null);
    assert_eq!(body["data"]["last_attempt"], Value::Null);

    // A wrong answer still reveals the solution, never the right options.
    let (status, body) = answer(&env, &ada.token, 1, json!({"options": ["a"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["correct"], false);
    assert_eq!(body["rewards"]["xp"], Value::Null);
    assert!(
        body["data"]["solution_md"]
            .as_str()
            .unwrap()
            .contains("SOLUTION-MARKER-1")
    );
    assert!(!body.to_string().contains("correctes"));

    let (_, body) = Call::get("/v1/me/exercises/1?lang=en")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(body["data"]["attempt_count"], 1);
    assert_eq!(
        body["data"]["last_attempt"]["answer"],
        json!({"options": ["a"]})
    );
    assert_eq!(body["data"]["last_attempt"]["correct"], false);
    assert!(
        body["data"]["solution_md"]
            .as_str()
            .unwrap()
            .contains("SOLUTION-MARKER-EN-1")
    );
    assert_eq!(body["data"]["lang"], "en");

    // The public statement never has it.
    let (_, body) = Call::get("/v1/exercises/1").send(&env.app).await;
    assert!(!body.to_string().contains("SOLUTION"));
}

#[tokio::test]
async fn answers_are_graded_and_pay_xp_on_the_first_success_only() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;

    // QCM: exactly the right options, in any order, duplicates ignored.
    let (_, body) = answer(&env, &ada.token, 3, json!({"options": ["a", "c"]})).await;
    assert_eq!(body["data"]["correct"], false);
    let (_, body) = answer(
        &env,
        &ada.token,
        3,
        json!({"options": ["e", "c", "a", "a"]}),
    )
    .await;
    assert_eq!(body["data"]["correct"], true);
    assert_eq!(body["rewards"], json!({"xp": 8, "brains": null}));
    let (_, body) = answer(&env, &ada.token, 3, json!({"options": ["a", "c", "e"]})).await;
    assert_eq!(body["data"]["correct"], true);
    assert_eq!(body["rewards"]["xp"], Value::Null);
    assert_eq!(body["data"]["attempt_count"], 3);

    // Short answer: tolerant comparison.
    let (_, body) = answer(&env, &ada.token, 2, json!({"text": " 0,75 "})).await;
    assert_eq!(body["data"]["correct"], true);
    let (_, body) = answer(&env, &ada.token, 4, json!({"text": "52 Euros !"})).await;
    assert_eq!(body["data"]["correct"], true);
    let (_, body) = answer(&env, &ada.token, 4, json!({"text": "53"})).await;
    assert_eq!(body["data"]["correct"], false);

    // Free answer: never graded.
    let (_, body) = answer(&env, &ada.token, 5, json!({"text": "Non, 0,99."})).await;
    assert_eq!(body["data"]["correct"], Value::Null);
    assert_eq!(body["rewards"]["xp"], Value::Null);

    // Every attempt is kept, as Laravel stored it; the course is joined.
    let stored: Vec<Value> = sqlx::query_scalar(
        "SELECT reponse::jsonb FROM site.exercise_attempts WHERE user_id = $1 AND exercise_id = 2",
    )
    .bind(ada.id)
    .fetch_all(&env.owner)
    .await
    .unwrap();
    assert_eq!(stored, vec![json!({"texte": "0,75"})]);
    let enrolled: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.enrollments WHERE user_id = $1 AND course_id = 1)",
    )
    .bind(ada.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    assert!(enrolled);
    // 3 exercises solved: 24 XP.
    let xp: i32 = sqlx::query_scalar("SELECT total FROM site.member_xp WHERE user_id = $1")
        .bind(ada.id)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(xp, 24);
}

#[tokio::test]
async fn answers_are_validated_and_drafts_unknown() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let (status, body) = answer(&env, &ada.token, 1, json!({"options": []})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        codes(&body),
        vec![("options".to_owned(), "required".to_owned())]
    );
    let (_, body) = answer(&env, &ada.token, 1, json!({"options": ["x".repeat(51)]})).await;
    assert_eq!(codes(&body)[0].1, "too_long");
    let (_, body) = answer(&env, &ada.token, 2, json!({"text": "   "})).await;
    assert_eq!(
        codes(&body),
        vec![("text".to_owned(), "required".to_owned())]
    );
    let (status, _) = answer(&env, &ada.token, 6, json!({"options": ["a"]})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = Call::post("/v1/exercises/1/attempts")
        .json(json!({"options": ["b"]}))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_catalogue_filters_and_shows_the_members_state() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    answer(&env, &ada.token, 1, json!({"options": ["b"]})).await;
    answer(&env, &ada.token, 3, json!({"options": ["a"]})).await;

    let (status, body) = Call::get("/v1/exercises/catalog").send(&env.app).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<i64> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_i64().unwrap())
        .collect();
    // Published courses only, by course, chapter and position.
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    assert_eq!(body["total"], 5);
    assert_eq!(body["data"][0]["solved"], Value::Null);
    assert_eq!(body["data"][0]["xp"], 8);
    assert_eq!(body["data"][0]["course"]["slug"], "les-bases-du-calcul");
    assert!(!body.to_string().contains("SOLUTION"));

    let (_, body) = Call::get("/v1/exercises/catalog")
        .member(&ada)
        .send(&env.app)
        .await;
    let solved: Vec<Value> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["solved"].clone())
        .collect();
    assert_eq!(
        solved,
        vec![
            json!(true),
            Value::Null,
            json!(false),
            Value::Null,
            Value::Null
        ]
    );

    let list = |q: &str| {
        let q = q.to_owned();
        let token = ada.token.clone();
        let app = env.app.clone();
        async move {
            let (_, body) = Call::get(format!("/v1/exercises/catalog?{q}"))
                .session(&token)
                .send(&app)
                .await;
            body["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_i64().unwrap())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(list("state=solved").await, vec![1]);
    assert_eq!(list("state=todo").await, vec![2, 3, 4, 5]);
    assert_eq!(list("state=new").await, vec![2, 4, 5]);
    assert_eq!(list("q=POURCENTAGE").await, vec![4, 5]);
    assert_eq!(list("type=qcm").await, vec![1, 3]);
    assert_eq!(list("subject=mathematiques").await, vec![1, 2, 3, 4, 5]);
    assert_eq!(list("subject=chimie").await, Vec::<i64>::new());
    // A visitor's state filter is ignored.
    let (_, body) = Call::get("/v1/exercises/catalog?state=solved")
        .send(&env.app)
        .await;
    assert_eq!(body["total"], 5);

    // Random: within the filters, or 404.
    let (status, body) = Call::get("/v1/exercises/random?state=solved")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["id"], 1);
    assert_eq!(body["data"]["course"]["slug"], "les-bases-du-calcul");
    let (status, _) = Call::get("/v1/exercises/random?subject=chimie")
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, body) = Call::get("/v1/exercises/facets").send(&env.app).await;
    assert_eq!(
        body["data"]["subjects"].as_array().map(Vec::len),
        Some(1),
        "{body}"
    );
    assert_eq!(
        body["data"]["subjects"],
        json!([{"slug": "mathematiques", "name": "Mathématiques", "exercise_count": 5}])
    );
}

// --- Dashboard, brains, leaderboards -------------------------------------------

#[tokio::test]
async fn the_dashboard_shows_courses_stats_and_brains() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    complete(&env, &ada.token, BASICS[1], true).await;
    complete(&env, &ada.token, CHEMISTRY_LESSON, true).await;
    answer(&env, &ada.token, 1, json!({"options": ["b"]})).await;
    answer(&env, &ada.token, 3, json!({"options": ["a"]})).await;

    let (status, body) = Call::get("/v1/me/dashboard?lang=en")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["in_progress"][0]["slug"], "les-bases-du-calcul");
    assert_eq!(
        data["in_progress"][0]["progress"],
        json!({"done": 1, "total": 4, "percent": 25})
    );
    assert_eq!(
        data["in_progress"][0]["resume_lesson"],
        json!({"slug": "les-fractions", "title": "Les fractions"})
    );
    assert_eq!(data["completed"][0]["slug"], "chimie-le-tableau-periodique");
    assert_eq!(
        data["stats"],
        json!({"courses_started": 2, "courses_completed": 1,
               "exercises_attempted": 2, "exercises_solved": 1})
    );
    assert_eq!(data["brains"]["balance"], 10);
    let movement = &data["brains"]["movements"][1];
    assert_eq!(movement["reason"], "lecon");
    assert_eq!(movement["amount"], 5);
    assert_eq!(movement["lesson_title"], "Fractions");
    assert!(data["suggestions"].as_array().unwrap().is_empty());

    let (_, body) = Call::get("/v1/me/brains?limit=1")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    let cursor = body["next_cursor"].as_str().unwrap().to_owned();
    let (_, next) = Call::get(format!("/v1/me/brains?limit=1&cursor={cursor}"))
        .member(&ada)
        .send(&env.app)
        .await;
    assert_ne!(next["data"][0]["id"], body["data"][0]["id"]);
    assert_eq!(next["next_cursor"], Value::Null);

    let fresh = env.member().create().await;
    let (status, _) = Call::get("/v1/me/dashboard")
        .member(&fresh)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_xp_leaderboard_lists_everyone_and_the_home_page_the_best() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().role("moderateur").create().await;
    let (status, body) = Call::get("/v1/leaderboard/xp?limit=100")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let me = &body["me"];
    assert_eq!(me["pseudo"], ada.pseudo.as_str());
    assert_eq!(
        (me["xp"].clone(), me["level"].clone(), me["me"].clone()),
        (json!(0), json!(1), json!(true))
    );
    assert_eq!(me["roles"][0]["slug"], "moderateur");
    let xp: Vec<i64> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["xp"].as_i64().unwrap())
        .collect();
    assert!(xp.windows(2).all(|w| w[0] >= w[1]), "{xp:?}");
    assert_eq!(body["data"][0]["rank"], 1);
    assert!(!body.to_string().contains("example.invalid"), "no e-mail");

    let (status, _) = Call::get("/v1/leaderboard/xp").send(&env.app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = Call::get("/v1/leaderboard/top").send(&env.app).await;
    assert_eq!(status, StatusCode::OK);
    let top = body["data"].as_array().unwrap();
    assert!(top.len() <= 5 && !top.is_empty());
    assert!(top.iter().all(|l| l["xp"].as_i64().unwrap() > 0));
    assert_eq!(top[0]["rank"], 1);
    assert!(top.iter().all(|l| l.as_object().unwrap().len() == 5));

    let (_, body) = Call::get("/v1/rewards").send(&env.app).await;
    assert_eq!(body["data"]["xp"]["exercise"], 8);
    assert_eq!(body["data"]["brains"]["game_daily_cap"], 20);
}

#[tokio::test]
async fn match_brains_are_capped_at_20_a_utc_day() {
    let Some(env) = Env::new().await else { return };
    let ada = env.member().verified().create().await;
    let day = chrono::DateTime::parse_from_rfc3339("2026-03-10T23:59:59.800Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let mut conn = env.db.acquire().await.unwrap();
    let mut earned = Vec::new();
    for i in 0..5 {
        let got =
            pulsars_api::db::brains::credit_match(&mut conn, ada.id, &format!("m{i}"), true, day)
                .await
                .unwrap();
        earned.push(got);
    }
    assert_eq!(earned, vec![Some(5), Some(5), Some(5), Some(5), Some(0)]);
    // The next UTC day starts again; a replayed match pays nothing.
    let next = day + chrono::Duration::seconds(1);
    let again = pulsars_api::db::brains::credit_match(&mut conn, ada.id, "m5", false, next)
        .await
        .unwrap();
    assert_eq!(again, Some(2));
    let replay = pulsars_api::db::brains::credit_match(&mut conn, ada.id, "m5", false, next)
        .await
        .unwrap();
    assert_eq!(replay, Some(0));
    // An account that no longer exists is skipped.
    let gone = pulsars_api::db::brains::credit_match(&mut conn, 999_999_999_999, "m6", true, next)
        .await
        .unwrap();
    assert_eq!(gone, None);
}
