//! Exercises of published courses, from `api_read.exercises` and the
//! translation views (`docs/contract/database.md` §4). The views carry no
//! answer: no `solution_md`, no `donnees`, no `correctes`, no `acceptees`.

use sqlx::PgPool;
use sqlx::types::Json;

macro_rules! exercise_select {
    () => {
        "SELECT e.id, e.type::text AS type, e.title, e.question_md,
                e.position::int AS position,
                -- Translated options when the translation has them (same ids,
                -- same order), else the original ones.
                COALESCE(et.options, e.options) AS options, e.multiple,
                ch.position::int AS chapter_position, ch.title AS chapter_title,
                c.slug AS course_slug, c.title AS course_title,
                c.difficulty::text AS course_difficulty,
                et.title AS tr_title, et.question_md AS tr_question_md,
                cht.title AS tr_chapter_title, ct.title AS tr_course_title
           FROM api_read.exercises e
           JOIN api_read.chapters ch ON ch.id = e.chapter_id
           JOIN api_read.courses c ON c.id = e.course_id
           LEFT JOIN api_read.exercise_translations et ON et.exercise_id = e.id AND et.lang = $1
           LEFT JOIN api_read.chapter_translations cht ON cht.chapter_id = ch.id AND cht.lang = $1
           LEFT JOIN api_read.course_translations ct ON ct.course_id = c.id AND ct.lang = $1"
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExerciseRow {
    pub id: i64,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub title: String,
    pub question_md: String,
    pub position: i32,
    /// QCM only: `[{"id": "a", "text": "31"}, …]`.
    pub options: Option<Json<serde_json::Value>>,
    pub multiple: Option<bool>,
    pub chapter_position: i32,
    pub chapter_title: String,
    pub course_slug: String,
    pub course_title: String,
    pub course_difficulty: String,
    pub tr_title: Option<String>,
    pub tr_question_md: Option<String>,
    pub tr_chapter_title: Option<String>,
    pub tr_course_title: Option<String>,
}

/// Filters of `GET /v1/exercises`.
#[derive(Debug, Default)]
pub struct ExerciseFilters<'a> {
    pub course: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub topic: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub difficulty: Option<&'a str>,
}

/// One page of exercises, by id, after `after`.
pub async fn list(
    pool: &PgPool,
    tr: Option<&str>,
    filters: &ExerciseFilters<'_>,
    after: Option<i64>,
    limit: i64,
) -> Result<Vec<ExerciseRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        exercise_select!(),
        "
          WHERE ($2::text IS NULL OR c.slug = $2)
            AND ($3::text IS NULL OR c.subject_slug = $3)
            AND ($4::text IS NULL OR c.topic_slug = $4)
            AND ($5::text IS NULL OR e.type = $5)
            AND ($6::text IS NULL OR c.difficulty = $6)
            AND ($7::bigint IS NULL OR e.id > $7)
          ORDER BY e.id
          LIMIT $8"
    ))
    .bind(tr)
    .bind(filters.course)
    .bind(filters.subject)
    .bind(filters.topic)
    .bind(filters.kind)
    .bind(filters.difficulty)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Exercise `id`, if its course is published.
pub async fn find(
    pool: &PgPool,
    tr: Option<&str>,
    id: i64,
) -> Result<Option<ExerciseRow>, sqlx::Error> {
    sqlx::query_as(concat!(exercise_select!(), " WHERE e.id = $2"))
        .bind(tr)
        .bind(id)
        .fetch_optional(pool)
        .await
}
