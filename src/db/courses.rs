//! Courses, their chapters and lessons, from `contenu` through the
//! `published_*!()` and `*_translations!()` sources (`super::sources`), and
//! the subject names of `contenu.matieres` (`docs/contract/database.md` §4).
//!
//! Every query takes `tr`, the language to read translations in, or `None`
//! for the original French text. Small integer columns are cast to `int`
//! so a type change in Laravel's tables cannot break decoding.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::types::Json;

/// Columns of a course, with its translation (`t`) and the translated
/// subject name (`sn`).
macro_rules! course_select {
    () => {
        concat!(
            "SELECT c.id, c.slug, c.title, c.description, c.theme,
                c.subject_name, c.subject_slug, c.topic_name, c.topic_slug,
                c.difficulty::text AS difficulty, c.levels,
                c.duration_minutes::int AS duration_minutes, c.tags,
                c.author_pseudo::text AS author_pseudo,
                c.chapter_count::int AS chapter_count,
                c.lesson_count::int AS lesson_count,
                c.exercise_count::int AS exercise_count,
                c.published_at, c.updated_at,
                t.title AS tr_title, t.description AS tr_description,
                sn.name AS tr_subject_name,
                count(*) OVER () AS total,
                ARRAY(SELECT DISTINCT a.lang::text FROM ",
            course_translations!(),
            " a
                       WHERE a.course_id = c.id ORDER BY 1) AS translation_langs
           FROM ",
            published_courses!(),
            " c
           LEFT JOIN ",
            course_translations!(),
            " t ON t.course_id = c.id AND t.lang = $1
           LEFT JOIN ",
            subject_names!(),
            " sn ON sn.subject_slug = c.subject_slug AND sn.lang = $1"
        )
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CourseRow {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub theme: String,
    pub subject_name: String,
    pub subject_slug: String,
    pub topic_name: Option<String>,
    pub topic_slug: Option<String>,
    pub difficulty: String,
    pub levels: Vec<String>,
    pub duration_minutes: Option<i32>,
    /// A JSON array of free tags. Anything that is not a string is skipped.
    pub tags: Json<serde_json::Value>,
    pub author_pseudo: Option<String>,
    pub chapter_count: i32,
    pub lesson_count: i32,
    pub exercise_count: i32,
    pub published_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tr_title: Option<String>,
    pub tr_description: Option<String>,
    pub tr_subject_name: Option<String>,
    /// Languages with a course-level translation.
    pub translation_langs: Vec<String>,
    /// Courses matching the filters of a list (0 for a single course).
    #[sqlx(default)]
    pub total: i64,
}

/// Filters of `GET /v1/courses`. `search` is a `LIKE` pattern.
#[derive(Debug, Default)]
pub struct CourseFilters<'a> {
    pub subject: Option<&'a str>,
    pub topic: Option<&'a str>,
    pub difficulty: Option<&'a str>,
    pub level: Option<&'a str>,
    pub tag: Option<&'a str>,
    /// The site's `theme` (free text, e.g. `algebre`).
    pub theme: Option<&'a str>,
    /// Only courses with a translation in this language.
    pub translated_in: Option<&'a str>,
    pub search: Option<&'a str>,
}

/// Order of `GET /v1/courses`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Original title, then id.
    #[default]
    Title,
    /// Latest published first, then highest id (the site's library).
    Recent,
}

/// Where a page of courses starts.
#[derive(Debug, Clone)]
pub enum After {
    Start,
    /// After `(title, id)`, in [`Sort::Title`].
    Title(String, i64),
    /// After `(published_at, id)`, in [`Sort::Recent`].
    Recent(DateTime<Utc>, i64),
    /// After `n` rows (`page`).
    Offset(i64),
}

/// One page of published courses in `sort` order, from `after`. Each row
/// carries `total`, the number of courses matching the filters.
pub async fn list(
    pool: &PgPool,
    tr: Option<&str>,
    filters: &CourseFilters<'_>,
    sort: Sort,
    after: After,
    limit: i64,
) -> Result<Vec<CourseRow>, sqlx::Error> {
    let (title, recent, offset) = match after {
        After::Start => (None, None, 0),
        After::Title(ref title, id) => (Some((title.as_str(), id)), None, 0),
        After::Recent(at, id) => (None, Some((at, id)), 0),
        After::Offset(n) => (None, None, n),
    };
    sqlx::query_as(concat!(
        course_select!(),
        concat!(
            "
          WHERE ($2::text IS NULL OR c.subject_slug = $2)
            AND ($3::text IS NULL OR c.topic_slug = $3)
            AND ($4::text IS NULL OR c.difficulty = $4)
            AND ($5::text IS NULL OR $5 = ANY(c.levels))
            AND ($6::text IS NULL OR c.tags @> jsonb_build_array($6::text))
            AND ($7::text IS NULL OR EXISTS (
                    SELECT 1 FROM ",
            course_translations!(),
            " x
                     WHERE x.course_id = c.id AND x.lang = $7))
            AND ($8::text IS NULL OR c.title ILIKE $8 OR c.description ILIKE $8
                 OR t.title ILIKE $8 OR t.description ILIKE $8)
            AND ($9::text IS NULL OR (c.title, c.id) > ($9::text, $10::bigint))
            AND ($12::timestamptz IS NULL
                 OR (c.published_at, c.id) < ($12::timestamptz, $13::bigint))
            AND ($15::text IS NULL OR c.theme = $15)
          ORDER BY CASE WHEN $14 THEN c.published_at END DESC,
                   CASE WHEN $14 THEN c.id END DESC,
                   c.title, c.id
          LIMIT $11 OFFSET $16"
        )
    ))
    .bind(tr)
    .bind(filters.subject)
    .bind(filters.topic)
    .bind(filters.difficulty)
    .bind(filters.level)
    .bind(filters.tag)
    .bind(filters.translated_in)
    .bind(filters.search)
    .bind(title.map(|(title, _)| title))
    .bind(title.map(|(_, id)| id))
    .bind(limit)
    .bind(recent.map(|(at, _)| at))
    .bind(recent.map(|(_, id)| id))
    .bind(sort == Sort::Recent)
    .bind(filters.theme)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// The published course `slug`.
pub async fn find(
    pool: &PgPool,
    tr: Option<&str>,
    slug: &str,
) -> Result<Option<CourseRow>, sqlx::Error> {
    sqlx::query_as(concat!(course_select!(), " WHERE c.slug = $2"))
        .bind(tr)
        .bind(slug)
        .fetch_optional(pool)
        .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChapterRow {
    pub id: i64,
    pub position: i32,
    pub title: String,
    pub tr_title: Option<String>,
}

/// Chapters of course `course_id`, in order.
pub async fn chapters(
    pool: &PgPool,
    tr: Option<&str>,
    course_id: i64,
) -> Result<Vec<ChapterRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ch.id, ch.position::int AS position, ch.title, t.title AS tr_title
           FROM ",
        published_chapters!(),
        " ch
           LEFT JOIN ",
        chapter_translations!(),
        " t ON t.chapter_id = ch.id AND t.lang = $1
          WHERE ch.course_id = $2
          ORDER BY ch.position, ch.id"
    ))
    .bind(tr)
    .bind(course_id)
    .fetch_all(pool)
    .await
}

/// A lesson without its content.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LessonRef {
    pub id: i64,
    pub chapter_id: i64,
    pub slug: String,
    pub position: i32,
    pub title: String,
    pub tr_title: Option<String>,
}

/// Lessons of course `course_id`, in reading order across chapters.
pub async fn lessons(
    pool: &PgPool,
    tr: Option<&str>,
    course_id: i64,
) -> Result<Vec<LessonRef>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT l.id, l.chapter_id, l.slug, l.position::int AS position, l.title,
                t.title AS tr_title
           FROM ",
        published_lessons!(),
        " l
           LEFT JOIN ",
        lesson_translations!(),
        " t ON t.lesson_id = l.id AND t.lang = $1
          WHERE l.course_id = $2
          ORDER BY l.chapter_position, l.position, l.id"
    ))
    .bind(tr)
    .bind(course_id)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LessonContent {
    pub content_md: String,
    pub tr_content_md: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// The content of lesson `id`.
pub async fn lesson_content(
    pool: &PgPool,
    tr: Option<&str>,
    id: i64,
) -> Result<Option<LessonContent>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT l.content_md, t.content_md AS tr_content_md, l.updated_at
           FROM ",
        published_lessons!(),
        " l
           LEFT JOIN ",
        lesson_translations!(),
        " t ON t.lesson_id = l.id AND t.lang = $1
          WHERE l.id = $2"
    ))
    .bind(tr)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// An exercise of the course outline, without its question.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExerciseRef {
    pub id: i64,
    pub chapter_id: i64,
    pub position: i32,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub title: String,
    pub tr_title: Option<String>,
}

/// Exercises of course `course_id`, in order.
pub async fn exercises(
    pool: &PgPool,
    tr: Option<&str>,
    course_id: i64,
) -> Result<Vec<ExerciseRef>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT e.id, e.chapter_id, e.position::int AS position, e.type::text AS type,
                e.title, t.title AS tr_title
           FROM ",
        published_exercises!(),
        " e
           LEFT JOIN ",
        exercise_translations!(),
        " t ON t.exercise_id = e.id AND t.lang = $1
          WHERE e.course_id = $2
          ORDER BY e.chapter_position, e.position, e.id"
    ))
    .bind(tr)
    .bind(course_id)
    .fetch_all(pool)
    .await
}

/// A (subject, topic) pair of the catalogue and its number of courses.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CatalogRow {
    pub subject_slug: String,
    pub subject_name: String,
    pub tr_subject_name: Option<String>,
    pub topic_slug: Option<String>,
    pub topic_name: Option<String>,
    pub course_count: i32,
}

/// Published courses counted by subject and topic, sorted by (translated)
/// subject name, then topic name; courses without a topic come first in
/// their subject.
pub async fn catalog(pool: &PgPool, tr: Option<&str>) -> Result<Vec<CatalogRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.subject_slug, c.subject_name, sn.name AS tr_subject_name,
                c.topic_slug, c.topic_name, count(*)::int AS course_count
           FROM ",
        published_courses!(),
        " c
           LEFT JOIN ",
        subject_names!(),
        " sn ON sn.subject_slug = c.subject_slug AND sn.lang = $1
          GROUP BY c.subject_slug, c.subject_name, sn.name, c.topic_slug, c.topic_name
          ORDER BY coalesce(sn.name, c.subject_name), c.subject_slug,
                   c.topic_name NULLS FIRST, c.topic_slug"
    ))
    .bind(tr)
    .fetch_all(pool)
    .await
}
