//! `pulsars-api content check|fix|translate` (`src/content.rs`), with the
//! cases of Laravel's `VerifierContenuTest`, `CorrigerContenuTest` and
//! `TraductionContenuTest`; DeepL is a fake server.
//!
//! `check` reads the whole database: the tests look for the problems of
//! their own rows only. `translate` is run on the test's own rows only
//! (`translate_element`), never over the shared demo content.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use axum::Json as AxumJson;
use axum::http::{HeaderMap, StatusCode, header};
use common::{Env, unique};
use pulsars_api::config::{DeeplConfig, Secret};
use pulsars_api::content;
use pulsars_api::deepl::Deepl;
use serde_json::{Value, json};

/// A course with one chapter, two lessons with the same slug, and a QCM
/// whose right answer is not among its options.
async fn plant_course(env: &Env) -> (i64, String, i64, i64) {
    let slug = format!("verif-{}", unique());
    let author = env.member().verified().create().await;
    let course: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.courses (titre, slug, description, theme, auteur_id)
         VALUES ('Vérif', $1, 'Description', 'Alchimie – Plomb', $2) RETURNING id",
    )
    .bind(&slug)
    .bind(author.id)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    let chapter: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.chapters (course_id, titre, ordre) VALUES ($1, 'Chapitre', 1) RETURNING id",
    )
    .bind(course)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    let lesson: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.lessons (chapter_id, titre, slug, contenu_md, ordre)
         VALUES ($1, 'Leçon', 'double', 'Le calcul $x^2$ vaut 4 et 4 fois rien.', 1) RETURNING id",
    )
    .bind(chapter)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO contenu.lessons (chapter_id, titre, slug, contenu_md, ordre)
         VALUES ($1, 'Leçon bis', 'double-bis', '   ', 2)",
    )
    .bind(chapter)
    .execute(&env.owner)
    .await
    .unwrap();
    let exercise: i64 = sqlx::query_scalar(
        r#"INSERT INTO contenu.exercises (chapter_id, type, titre, question_md, donnees, solution_md, ordre)
           VALUES ($1, 'qcm', 'QCM', 'Combien ?', '{"options": [{"id": "a", "texte": "Un"}, {"id": "b", "texte": "Deux"}], "correctes": ["z"]}',
                   NULL, 1) RETURNING id"#,
    )
    .bind(chapter)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    (course, slug, lesson, exercise)
}

async fn remove_course(env: &Env, course: i64) {
    sqlx::query(
        "DELETE FROM contenu.contenu_translations t
          WHERE (t.traduisible_type = 'course' AND t.traduisible_id = $1)
             OR (t.traduisible_type = 'lesson' AND t.traduisible_id IN (
                    SELECT l.id FROM contenu.lessons l
                      JOIN contenu.chapters c ON c.id = l.chapter_id WHERE c.course_id = $1))
             OR (t.traduisible_type = 'exercise' AND t.traduisible_id IN (
                    SELECT e.id FROM contenu.exercises e
                      JOIN contenu.chapters c ON c.id = e.chapter_id WHERE c.course_id = $1))",
    )
    .bind(course)
    .execute(&env.owner)
    .await
    .unwrap();
    sqlx::query("DELETE FROM contenu.courses WHERE id = $1")
        .bind(course)
        .execute(&env.owner)
        .await
        .unwrap();
}

#[tokio::test]
async fn check_reports_what_is_wrong_and_changes_nothing() {
    let Some(env) = Env::new().await else { return };
    let (course, slug, _, exercise) = plant_course(&env).await;
    let second: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.chapters (course_id, titre, ordre) VALUES ($1, 'Deux', 2) RETURNING id",
    )
    .bind(course)
    .fetch_one(&env.owner)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE contenu.lessons SET slug = 'double', chapter_id = $2
          WHERE slug = 'double-bis' AND chapter_id IN (SELECT id FROM contenu.chapters WHERE course_id = $1)",
    )
    .bind(course)
    .bind(second)
    .execute(&env.owner)
    .await
    .unwrap();

    let check = content::check(&env.db, "en").await.unwrap();
    let ours: Vec<&String> = check
        .problems
        .iter()
        .filter(|p| p.contains(&format!("'{slug}'")))
        .collect();
    let name = format!("Course '{slug}'");
    for expected in [
        format!("{name}: subject unknown to the catalogue ('Alchimie – Plomb')."),
        format!("{name}: no translation in en."),
        format!("{name}: duplicate lesson slug 'double'."),
        format!("{name}, chapter 1: no translation in en."),
        format!("{name}: empty lesson 'double'."),
        format!("{name}, exercise {exercise}: no solution."),
        format!("{name}, exercise {exercise}: right answers missing or not among the options."),
    ] {
        assert!(ours.contains(&&expected), "{expected} not in {ours:#?}");
    }
    assert!(check.courses >= 1);
    remove_course(&env, course).await;
}

#[tokio::test]
async fn fix_replaces_a_unique_text_and_keeps_translations_up_to_date() {
    let Some(env) = Env::new().await else { return };
    let (course, _, lesson, exercise) = plant_course(&env).await;
    let original = [
        "Leçon".to_owned(),
        "Le calcul $x^2$ vaut 4 et 4 fois rien.".to_owned(),
    ];
    let hash = content::source_hash(&original.clone().map(Some), &[]);
    sqlx::query(
        r#"INSERT INTO contenu.contenu_translations (traduisible_type, traduisible_id, langue, champs, source_hash)
           VALUES ('lesson', $1, 'en', '{"titre": "Lesson", "contenu_md": "The calcul is 4."}', $2),
                  ('exercise', $3, 'ru', '{"titre": "Тест", "options": {"a": "Один", "b": "Дво"}}', 'stale')"#,
    )
    .bind(lesson)
    .bind(&hash)
    .bind(exercise)
    .execute(&env.owner)
    .await
    .unwrap();

    let file = json!({"corrections": [
        {"cle": format!("lesson:{lesson}"), "langue": "fr", "champ": "contenu_md",
         "ancien": "vaut 4", "nouveau": "vaut quatre"},
        {"cle": format!("lesson:{lesson}"), "langue": "en", "champ": "contenu_md",
         "ancien": "calcul", "nouveau": "result"},
        {"cle": format!("exercise:{exercise}"), "langue": "ru", "champ": "options.b",
         "ancien": "Дво", "nouveau": "Два"},
        {"cle": format!("exercise:{exercise}"), "langue": "fr", "champ": "options.a",
         "ancien": "Un", "nouveau": "Une"},
        {"cle": "lesson:abc", "langue": "fr", "champ": "titre", "ancien": "a", "nouveau": "b"},
        {"cle": "course:999999999", "langue": "fr", "champ": "titre", "ancien": "a", "nouveau": "b"},
    ]});

    // A dry run checks and writes nothing.
    let dry = content::fix(&env.db, &file, true).await.unwrap();
    assert_eq!(dry.applied, 4);
    assert_eq!(
        dry.refused,
        [
            "lesson:abc: malformed key",
            "course:999999999: element not found"
        ]
    );
    let unchanged: String =
        sqlx::query_scalar("SELECT contenu_md FROM contenu.lessons WHERE id = $1")
            .bind(lesson)
            .fetch_one(&env.db)
            .await
            .unwrap();
    assert_eq!(unchanged, original[1]);

    let fixed = content::fix(&env.db, &file, false).await.unwrap();
    assert_eq!(fixed.applied, 4);
    let (text, translated, tr_hash): (String, Value, String) = sqlx::query_as(
        "SELECT l.contenu_md, t.champs::jsonb, t.source_hash FROM contenu.lessons l
           JOIN contenu.contenu_translations t ON t.traduisible_type = 'lesson' AND t.traduisible_id = l.id
          WHERE l.id = $1",
    )
    .bind(lesson)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(text, "Le calcul $x^2$ vaut quatre et 4 fois rien.");
    assert_eq!(translated["contenu_md"], "The result is 4.");
    // It was up to date: it still is, under the new hash.
    assert_eq!(
        tr_hash,
        content::source_hash(&[Some("Leçon".to_owned()), Some(text)], &[])
    );
    let (data, ru): (Value, Value) = sqlx::query_as(
        "SELECT e.donnees::jsonb, t.champs::jsonb FROM contenu.exercises e
           JOIN contenu.contenu_translations t ON t.traduisible_type = 'exercise' AND t.traduisible_id = e.id
          WHERE e.id = $1",
    )
    .bind(exercise)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(data["options"][0]["texte"], "Une");
    assert_eq!(data["correctes"], json!(["z"]));
    assert_eq!(ru["options"]["b"], "Два");

    // "i" twice: refused, and the element is left whole.
    let again = content::fix(
        &env.db,
        &json!({"corrections": [
            {"cle": format!("lesson:{lesson}"), "langue": "fr", "champ": "titre",
             "ancien": "Leçon", "nouveau": "Cours"},
            {"cle": format!("lesson:{lesson}"), "langue": "fr", "champ": "contenu_md",
             "ancien": "i", "nouveau": "y"},
        ]}),
        false,
    )
    .await
    .unwrap();
    assert_eq!(again.applied, 0);
    assert_eq!(
        again.refused,
        [format!(
            "lesson:{lesson}: fr, contenu_md: text 'ancien' found 2 times (it must be unique)"
        )]
    );
    let title: String = sqlx::query_scalar("SELECT titre FROM contenu.lessons WHERE id = $1")
        .bind(lesson)
        .fetch_one(&env.db)
        .await
        .unwrap();
    assert_eq!(title, "Leçon");

    assert!(
        content::fix(&env.db, &json!({"x": 1}), false)
            .await
            .is_err()
    );
    remove_course(&env, course).await;
}

/// A fake DeepL: `EN:` before each text, for the right key.
async fn fake_deepl(fail: bool) -> String {
    async fn translate(
        headers: HeaderMap,
        AxumJson(body): AxumJson<Value>,
    ) -> Result<AxumJson<Value>, StatusCode> {
        if headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            != Some("DeepL-Auth-Key test-key")
        {
            return Err(StatusCode::FORBIDDEN);
        }
        assert_eq!(body["source_lang"], "FR");
        assert_eq!(body["preserve_formatting"], true);
        let prefix = format!("{}:", body["target_lang"].as_str().unwrap());
        let translations: Vec<Value> = body["text"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| json!({"detected_source_language": "FR", "text": format!("{prefix}{}", t.as_str().unwrap())}))
            .collect();
        Ok(AxumJson(json!({"translations": translations})))
    }
    async fn broken() -> StatusCode {
        StatusCode::BAD_REQUEST
    }
    let app = if fail {
        axum::Router::new().route("/v2/translate", axum::routing::post(broken))
    } else {
        axum::Router::new().route("/v2/translate", axum::routing::post(translate))
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn deepl(url: String) -> Deepl {
    let mut deepl = Deepl::new(DeeplConfig {
        key: Secret::new("test-key"),
        url,
        languages: [("fr", "FR"), ("en", "EN-GB"), ("ru", "RU")]
            .map(|(site, deepl)| (site.to_owned(), deepl.to_owned()))
            .to_vec(),
    })
    .unwrap();
    deepl.retry_delay = Duration::ZERO;
    deepl
}

#[tokio::test]
async fn translate_writes_up_to_date_translations_with_deepl() {
    let Some(env) = Env::new().await else { return };
    let (course, _, lesson, exercise) = plant_course(&env).await;
    let deepl = deepl(fake_deepl(false).await);

    assert!(
        content::translate_element(&env.db, &deepl, "lesson", lesson, "en", false)
            .await
            .unwrap()
    );
    let champs: Value = sqlx::query_scalar(
        "SELECT champs::jsonb FROM contenu.contenu_translations
          WHERE traduisible_type = 'lesson' AND traduisible_id = $1 AND langue = 'en'",
    )
    .bind(lesson)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(champs["titre"], "EN-GB:Leçon");
    // The formula is put aside, then back in place.
    assert_eq!(
        champs["contenu_md"],
        "EN-GB:Le calcul $x^2$ vaut 4 et 4 fois rien."
    );
    // Up to date: not translated again, unless forced.
    assert!(
        !content::translate_element(&env.db, &deepl, "lesson", lesson, "en", false)
            .await
            .unwrap()
    );
    assert!(
        content::translate_element(&env.db, &deepl, "lesson", lesson, "en", true)
            .await
            .unwrap()
    );

    // A QCM's options are translated; its empty solution stays empty.
    assert!(
        content::translate_element(&env.db, &deepl, "exercise", exercise, "ru", false)
            .await
            .unwrap()
    );
    let champs: Value = sqlx::query_scalar(
        "SELECT champs::jsonb FROM contenu.contenu_translations
          WHERE traduisible_type = 'exercise' AND traduisible_id = $1 AND langue = 'ru'",
    )
    .bind(exercise)
    .fetch_one(&env.db)
    .await
    .unwrap();
    assert_eq!(champs["options"], json!({"a": "RU:Un", "b": "RU:Deux"}));
    assert_eq!(champs["solution_md"], "");
    assert!(champs.get("correctes").is_none());

    // A service that refuses: an error, nothing written.
    let broken = deepl_broken().await;
    let error = content::translate_element(&env.db, &broken, "course", course, "en", false)
        .await
        .unwrap_err();
    assert!(error.contains("HTTP 400"), "{error}");

    let mut log = |_: String| {};
    assert!(
        content::translate(&env.db, &deepl, "fr", false, None, &mut log)
            .await
            .is_err()
    );
    assert!(
        content::translate(&env.db, &deepl, "de", false, None, &mut log)
            .await
            .is_err()
    );
    remove_course(&env, course).await;
}

async fn deepl_broken() -> Deepl {
    deepl(fake_deepl(true).await)
}
