//! Exercises as a member does them (`site.exercise_attempts`): the answer
//! key through `exercise_answers!()`, the attempts, and the catalogue of
//! Laravel's `ExerciseController::catalogue`, with each exercise's state for
//! the member.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Json;

/// What grading and the solution need, for an exercise of a published
/// course.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AnswerKeyRow {
    pub id: i64,
    pub course_id: i64,
    pub course_slug: String,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub correct: Option<Json<Value>>,
    pub accepted: Option<Json<Value>>,
    /// The accepted answers of every translation.
    pub tr_accepted: Vec<Json<Value>>,
    pub solution_md: Option<String>,
    pub tr_solution_md: Option<String>,
}

pub async fn answer_key(
    pool: &PgPool,
    id: i64,
    tr: Option<&str>,
) -> Result<Option<AnswerKeyRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT a.id, a.course_id, a.course_slug, a.type::text AS type, a.correct, a.accepted,
                coalesce((SELECT array_agg(t.accepted ORDER BY t.lang)
                            FROM ",
        exercise_answer_translations!(),
        " t WHERE t.exercise_id = a.id AND t.accepted IS NOT NULL), '{}') AS tr_accepted,
                a.solution_md,
                (SELECT t.solution_md FROM ",
        exercise_answer_translations!(),
        " t WHERE t.exercise_id = a.id AND t.lang = $2) AS tr_solution_md
           FROM ",
        exercise_answers!(),
        " a WHERE a.id = $1"
    ))
    .bind(id)
    .bind(tr)
    .fetch_optional(pool)
    .await
}

/// Records an attempt and enrolls the member on the exercise's course, in
/// one transaction. Returns whether the exercise was solved before.
pub async fn record(
    pool: &PgPool,
    user_id: i64,
    exercise_id: i64,
    course_id: i64,
    answer: &Value,
    correct: Option<bool>,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let solved_before: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.exercise_attempts
                         WHERE user_id = $1 AND exercise_id = $2 AND correct)",
    )
    .bind(user_id)
    .bind(exercise_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO site.exercise_attempts (user_id, exercise_id, reponse, correct, created_at)
         VALUES ($1, $2, $3, $4, now() AT TIME ZONE 'UTC')",
    )
    .bind(user_id)
    .bind(exercise_id)
    .bind(Json(answer))
    .bind(correct)
    .execute(&mut *tx)
    .await?;
    super::learning::enroll(&mut tx, user_id, course_id).await?;
    tx.commit().await?;
    Ok(solved_before)
}

/// The member's attempts at an exercise: how many, and the last one.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AttemptsRow {
    pub count: i64,
    pub solved: bool,
    pub last_answer: Option<Json<Value>>,
    pub last_correct: Option<bool>,
    pub last_at: Option<DateTime<Utc>>,
}

pub async fn attempts(
    pool: &PgPool,
    user_id: i64,
    exercise_id: i64,
) -> Result<AttemptsRow, sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*) AS count, coalesce(bool_or(a.correct), false) AS solved,
                (array_agg(a.reponse::jsonb ORDER BY a.id DESC))[1] AS last_answer,
                (array_agg(a.correct ORDER BY a.id DESC))[1] AS last_correct,
                (array_agg(a.created_at ORDER BY a.id DESC))[1] AT TIME ZONE 'UTC' AS last_at
           FROM site.exercise_attempts a
          WHERE a.user_id = $1 AND a.exercise_id = $2",
    )
    .bind(user_id)
    .bind(exercise_id)
    .fetch_one(pool)
    .await
}

/// Filters of the catalogue, validated.
#[derive(Debug, Default)]
pub struct CatalogFilters<'a> {
    /// Searched in the title and the statement, ignoring case.
    pub search: Option<&'a str>,
    /// The slug of the start of the course's theme (« Mathématiques »:
    /// `mathematiques`), as `/v1/exercises/facets` lists them.
    pub subject: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub difficulty: Option<&'a str>,
    /// `todo` (never solved), `new` (never tried), `solved`; members only.
    pub state: Option<&'a str>,
}

/// `$1` lang, `$2` member, `$3` search, `$4` subject, `$5` type, `$6`
/// difficulty, `$7` state. A macro so that the queries stay literals.
macro_rules! catalog_from {
    () => {
        concat!(
            " FROM ",
            published_exercises!(),
            " e
           JOIN ",
            published_courses!(),
            " c ON c.id = e.course_id
           LEFT JOIN ",
            exercise_translations!(),
            " et ON et.exercise_id = e.id AND et.lang = $1
           LEFT JOIN ",
            course_translations!(),
            " ct ON ct.course_id = c.id AND ct.lang = $1
          WHERE ($3::text IS NULL OR e.title ILIKE '%' || $3 || '%' ESCAPE '\\'
                                  OR e.question_md ILIKE '%' || $3 || '%' ESCAPE '\\')
            AND ($4::text IS NULL OR c.subject_slug = $4)
            AND ($5::text IS NULL OR e.type = $5)
            AND ($6::text IS NULL OR c.difficulty = $6)
            AND ($7::text IS NULL OR $2::bigint IS NULL OR CASE $7
                WHEN 'todo' THEN NOT EXISTS (SELECT 1 FROM site.exercise_attempts a
                    WHERE a.user_id = $2 AND a.exercise_id = e.id AND a.correct)
                WHEN 'new' THEN NOT EXISTS (SELECT 1 FROM site.exercise_attempts a
                    WHERE a.user_id = $2 AND a.exercise_id = e.id)
                ELSE EXISTS (SELECT 1 FROM site.exercise_attempts a
                    WHERE a.user_id = $2 AND a.exercise_id = e.id AND a.correct)
                END)"
        )
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CardRow {
    pub id: i64,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub title: String,
    pub tr_title: Option<String>,
    pub course_slug: String,
    pub course_title: String,
    pub tr_course_title: Option<String>,
    pub theme: String,
    pub difficulty: String,
    /// `None`: never tried; else whether one attempt was right.
    pub solved: Option<bool>,
}

/// `%`, `_` and `\` match themselves in a `LIKE` pattern.
pub fn like_literal(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// One page of the catalogue, by course, chapter and position, and the
/// number of matching exercises.
pub async fn catalog(
    pool: &PgPool,
    tr: Option<&str>,
    user_id: Option<i64>,
    filters: &CatalogFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<CardRow>, i64), sqlx::Error> {
    let search = filters.search.map(like_literal);
    let rows: Vec<CardRow> = sqlx::query_as(concat!(
        "SELECT e.id, e.type::text AS type, e.title, et.title AS tr_title,
                c.slug AS course_slug, c.title AS course_title, ct.title AS tr_course_title,
                c.theme, c.difficulty::text AS difficulty,
                (SELECT bool_or(coalesce(a.correct, false)) FROM site.exercise_attempts a
                  WHERE a.user_id = $2 AND a.exercise_id = e.id) AS solved",
        catalog_from!(),
        " ORDER BY e.course_id, e.chapter_id, e.position, e.id LIMIT $8 OFFSET $9"
    ))
    .bind(tr)
    .bind(user_id)
    .bind(search.as_deref())
    .bind(filters.subject)
    .bind(filters.kind)
    .bind(filters.difficulty)
    .bind(filters.state)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar(concat!("SELECT count(*)", catalog_from!()))
        .bind(tr)
        .bind(user_id)
        .bind(search.as_deref())
        .bind(filters.subject)
        .bind(filters.kind)
        .bind(filters.difficulty)
        .bind(filters.state)
        .fetch_one(pool)
        .await?;
    Ok((rows, total))
}

/// One exercise drawn at random among those of the filters.
pub async fn random(
    pool: &PgPool,
    user_id: Option<i64>,
    filters: &CatalogFilters<'_>,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    let search = filters.search.map(like_literal);
    sqlx::query_as(concat!(
        "SELECT e.id, c.slug",
        catalog_from!(),
        " ORDER BY random() LIMIT 1"
    ))
    .bind(None::<&str>)
    .bind(user_id)
    .bind(search.as_deref())
    .bind(filters.subject)
    .bind(filters.kind)
    .bind(filters.difficulty)
    .bind(filters.state)
    .fetch_optional(pool)
    .await
}

/// The subjects of the catalogue: the start of the published courses'
/// themes, with their exercise count, those with exercises only, by name.
pub async fn subjects(pool: &PgPool) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.subject_slug, min(c.subject_name), sum(c.exercise_count)::bigint
           FROM ",
        published_courses!(),
        " c GROUP BY 1 HAVING sum(c.exercise_count) > 0 ORDER BY min(c.subject_name) COLLATE \"C\", 1"
    ))
    .fetch_all(pool)
    .await
}
