//! The languages section, from the `langues` schema the API owns
//! (`migrations/20261020000000_langues_schema.sql`, `docs/languages.md`).
//!
//! Interface texts are JSON objects keyed by language: every query takes
//! `tr`, the language to read translations in (`None` for French), and
//! returns the French text plus the translation (`tr_*`, null when missing), for
//! [`crate::routes::public::translated`]. Only published languages exist
//! for the API.

use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LanguageRow {
    pub code: String,
    pub name: String,
    pub tr_name: Option<String>,
    pub native_name: String,
    pub script: String,
    pub reading_system: Option<String>,
    pub countries: Vec<String>,
    pub level_count: i64,
    pub word_count: i64,
}

macro_rules! language_select {
    () => {
        "SELECT g.code, g.name->>'fr' AS name, g.name->>$1 AS tr_name,
                g.native_name, g.script, g.reading_system, g.countries,
                (SELECT count(*) FROM langues.levels l WHERE l.language_code = g.code) AS level_count,
                (SELECT count(*) FROM langues.words w JOIN langues.levels l ON l.id = w.level_id
                  WHERE l.language_code = g.code) AS word_count
           FROM langues.languages g
          WHERE g.published"
    };
}

/// Published languages, in order.
pub async fn list(pool: &PgPool, tr: Option<&str>) -> Result<Vec<LanguageRow>, sqlx::Error> {
    sqlx::query_as(concat!(language_select!(), " ORDER BY g.position, g.code"))
        .bind(tr)
        .fetch_all(pool)
        .await
}

/// The published language `code`.
pub async fn find(
    pool: &PgPool,
    tr: Option<&str>,
    code: &str,
) -> Result<Option<LanguageRow>, sqlx::Error> {
    sqlx::query_as(concat!(language_select!(), " AND g.code = $2"))
        .bind(tr)
        .bind(code)
        .fetch_optional(pool)
        .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LevelRow {
    pub id: i64,
    pub slug: String,
    pub framework: String,
    pub position: i32,
    pub title: String,
    pub tr_title: Option<String>,
    pub description: String,
    pub tr_description: Option<String>,
    pub word_count: i64,
}

/// Levels of language `code`, in order. The language is not checked:
/// call [`find`] first.
pub async fn levels(
    pool: &PgPool,
    tr: Option<&str>,
    code: &str,
) -> Result<Vec<LevelRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT l.id, l.slug, l.framework, l.position::int AS position,
                l.title->>'fr' AS title, l.title->>$1 AS tr_title,
                l.description->>'fr' AS description, l.description->>$1 AS tr_description,
                (SELECT count(*) FROM langues.words w WHERE w.level_id = l.id) AS word_count
           FROM langues.levels l
          WHERE l.language_code = $2
          ORDER BY l.position",
    )
    .bind(tr)
    .bind(code)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WordRow {
    pub id: i64,
    pub text: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub tr_meaning: Option<String>,
}

/// Words of level `level_id`, in order.
pub async fn words(
    pool: &PgPool,
    tr: Option<&str>,
    level_id: i64,
) -> Result<Vec<WordRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT w.id, w.text, w.reading,
                w.meaning->>'fr' AS meaning, w.meaning->>$1 AS tr_meaning
           FROM langues.words w
          WHERE w.level_id = $2
          ORDER BY w.position, w.id",
    )
    .bind(tr)
    .bind(level_id)
    .fetch_all(pool)
    .await
}

/// A learner's event (`POST /v1/languages/events`), already validated.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    LessonCompleted {
        score: i32,
        correct: i16,
        questions: i16,
        duration_ms: i32,
        /// Ids of the words missed; ids not in the level are dropped.
        missed_words: Vec<i64>,
    },
    WritingCompleted {
        character: String,
        /// `trace` or `memory`.
        mode: &'static str,
        mistakes: i16,
    },
}

/// Records `event` for the level `slug` of the published language `code`.
/// `false` when there is no such level.
pub async fn record_event(
    pool: &PgPool,
    learner: uuid::Uuid,
    code: &str,
    slug: &str,
    event: &Event,
) -> Result<bool, sqlx::Error> {
    let (kind, score, correct, questions, duration_ms, missed, character, mode, mistakes) =
        match event {
            Event::LessonCompleted {
                score,
                correct,
                questions,
                duration_ms,
                missed_words,
            } => (
                "lesson.completed",
                Some(*score),
                Some(*correct),
                Some(*questions),
                Some(*duration_ms),
                Some(missed_words.as_slice()),
                None,
                None,
                None,
            ),
            Event::WritingCompleted {
                character,
                mode,
                mistakes,
            } => (
                "writing.completed",
                None,
                None,
                None,
                None,
                None,
                Some(character.as_str()),
                Some(*mode),
                Some(*mistakes),
            ),
        };
    let inserted = sqlx::query(
        "INSERT INTO langues.events
                (kind, learner, language_code, level_slug, score, correct, questions,
                 duration_ms, missed_words, character, mode, mistakes)
         SELECT $1, $2, l.language_code, l.slug, $5, $6, $7, $8,
                CASE WHEN $9::bigint[] IS NOT NULL THEN
                    ARRAY(SELECT w.id FROM langues.words w
                           WHERE w.level_id = l.id AND w.id = ANY($9::bigint[]) ORDER BY w.id)
                END,
                $10, $11, $12
           FROM langues.levels l
           JOIN langues.languages g ON g.code = l.language_code AND g.published
          WHERE l.language_code = $3 AND l.slug = $4",
    )
    .bind(kind)
    .bind(learner)
    .bind(code)
    .bind(slug)
    .bind(score)
    .bind(correct)
    .bind(questions)
    .bind(duration_ms)
    .bind(missed)
    .bind(character)
    .bind(mode)
    .bind(mistakes)
    .execute(pool)
    .await?;
    Ok(inserted.rows_affected() == 1)
}
