//! The languages section, from the `langues` schema the API owns
//! (`migrations/`, `docs/languages.md`): languages, their levels and
//! words, their grammar topics and sentences.
//!
//! Interface texts are JSON objects keyed by language: every query takes
//! `tr`, the language to read translations in (`None` for French), and
//! returns the French text plus the translation (`tr_*`, null when missing), for
//! [`crate::routes::public::translated`]. Only published languages exist
//! for the API.

use sqlx::PgPool;
use sqlx::types::Json;

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
    pub topic_count: i64,
}

macro_rules! language_select {
    () => {
        "SELECT g.code, g.name->>'fr' AS name, g.name->>$1 AS tr_name,
                g.native_name, g.script, g.reading_system, g.countries,
                (SELECT count(*) FROM langues.levels l WHERE l.language_code = g.code) AS level_count,
                (SELECT count(*) FROM langues.words w JOIN langues.levels l ON l.id = w.level_id
                  WHERE l.language_code = g.code) AS word_count,
                (SELECT count(*) FROM langues.topics t WHERE t.language_code = g.code) AS topic_count
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

/// A section of a topic's lesson: a rule and its examples
/// (`langues.topics.lesson`, one list per interface language).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LessonSection {
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub examples: Vec<LessonExample>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LessonExample {
    /// In the language taught.
    pub text: String,
    /// What to notice, in the interface language.
    pub note: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TopicRow {
    pub id: i64,
    pub slug: String,
    pub position: i32,
    pub title: String,
    pub tr_title: Option<String>,
    pub description: String,
    pub tr_description: Option<String>,
    pub choices: Vec<String>,
    pub lesson: Json<Vec<LessonSection>>,
    pub tr_lesson: Option<Json<Vec<LessonSection>>>,
    pub exercise_count: i64,
}

/// Grammar topics of language `code`, in order, with their lessons. The
/// language is not checked: call [`find`] first.
pub async fn topics(
    pool: &PgPool,
    tr: Option<&str>,
    code: &str,
) -> Result<Vec<TopicRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.id, t.slug, t.position::int AS position,
                t.title->>'fr' AS title, t.title->>$1 AS tr_title,
                t.description->>'fr' AS description, t.description->>$1 AS tr_description,
                t.choices, t.lesson->'fr' AS lesson,
                CASE WHEN jsonb_typeof(t.lesson->$1) = 'array' THEN t.lesson->$1 END AS tr_lesson,
                (SELECT count(*) FROM langues.topic_exercises e WHERE e.topic_id = t.id) AS exercise_count
           FROM langues.topics t
          WHERE t.language_code = $2
          ORDER BY t.position, t.slug",
    )
    .bind(tr)
    .bind(code)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExerciseRow {
    pub id: i64,
    pub sentence: String,
    pub answer: String,
    pub phrase: String,
    pub explanation: String,
    pub tr_explanation: Option<String>,
}

/// Sentences of topic `topic_id`, in order.
pub async fn exercises(
    pool: &PgPool,
    tr: Option<&str>,
    topic_id: i64,
) -> Result<Vec<ExerciseRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.id, e.sentence, e.answer, e.phrase,
                e.explanation->>'fr' AS explanation, e.explanation->>$1 AS tr_explanation
           FROM langues.topic_exercises e
          WHERE e.topic_id = $2
          ORDER BY e.position, e.id",
    )
    .bind(tr)
    .bind(topic_id)
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
