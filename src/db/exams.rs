//! Published exam papers, from `contenu` through `published_exams!()`,
//! `published_exam_parts!()` and `exam_part_translations!()`
//! (`super::sources`, `docs/contract/database.md` §4). Statements only: the
//! sources select no `corrige_md` and no correction file.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::types::Json;

macro_rules! exam_select {
    () => {
        concat!(
            "SELECT x.id, x.slug, x.title, x.type::text AS type,
                x.competition::text AS competition, x.subject::text AS subject,
                x.country::text AS country, x.level::text AS level, x.tracks,
                x.session::text AS session,
                x.duration_minutes::int AS duration_minutes,
                x.total_points::int AS total_points,
                x.part_count::int AS part_count, x.has_pdf,
                x.official_url, x.source_url::text AS source_url,
                x.published_at, x.description,
                count(*) OVER () AS total
           FROM ",
            published_exams!(),
            " x"
        )
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExamRow {
    pub id: i64,
    pub slug: String,
    pub title: String,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub competition: Option<String>,
    pub subject: String,
    pub country: String,
    pub level: String,
    /// JSON array of CPGE tracks.
    pub tracks: Json<serde_json::Value>,
    pub session: Option<String>,
    pub duration_minutes: Option<i32>,
    pub total_points: Option<i32>,
    pub part_count: i32,
    pub has_pdf: bool,
    /// Already filtered by the `SujetOfficiel` allow-list in the view.
    pub official_url: Option<String>,
    pub source_url: Option<String>,
    pub published_at: DateTime<Utc>,
    pub description: Option<String>,
    /// Exams matching the filters of a list.
    pub total: i64,
}

/// Filters of `GET /v1/exams`. `search` is a `LIKE` pattern.
#[derive(Debug, Default)]
pub struct ExamFilters<'a> {
    pub kind: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub level: Option<&'a str>,
    pub session: Option<&'a str>,
    pub country: Option<&'a str>,
    pub competition: Option<&'a str>,
    pub search: Option<&'a str>,
}

/// One page of exams, newest first, after `after` or after `offset` rows.
pub async fn list(
    pool: &PgPool,
    filters: &ExamFilters<'_>,
    after: Option<(DateTime<Utc>, i64)>,
    offset: i64,
    limit: i64,
) -> Result<Vec<ExamRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        exam_select!(),
        "
          WHERE ($1::text IS NULL OR x.type = $1)
            AND ($2::text IS NULL OR x.subject = $2)
            AND ($3::text IS NULL OR x.level = $3)
            AND ($4::text IS NULL OR x.session = $4)
            AND ($5::text IS NULL OR x.country = $5)
            AND ($6::text IS NULL OR x.competition = $6)
            AND ($7::text IS NULL OR x.title ILIKE $7 OR x.description ILIKE $7)
            AND ($8::timestamptz IS NULL OR (x.published_at, x.id) < ($8::timestamptz, $9::bigint))
          ORDER BY x.published_at DESC, x.id DESC
          LIMIT $10 OFFSET $11"
    ))
    .bind(filters.kind)
    .bind(filters.subject)
    .bind(filters.level)
    .bind(filters.session)
    .bind(filters.country)
    .bind(filters.competition)
    .bind(filters.search)
    .bind(after.map(|(at, _)| at))
    .bind(after.map(|(_, id)| id))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// The published exam `slug`.
pub async fn find(pool: &PgPool, slug: &str) -> Result<Option<ExamRow>, sqlx::Error> {
    sqlx::query_as(concat!(exam_select!(), " WHERE x.slug = $1"))
        .bind(slug)
        .fetch_optional(pool)
        .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PartRow {
    pub id: i64,
    pub position: i32,
    pub title: String,
    pub statement_md: String,
    /// `numeric(5,2)`, read as a float: at most 999.99, two decimals.
    pub points: Option<f64>,
    pub tr_title: Option<String>,
    pub tr_statement_md: Option<String>,
}

/// Parts of exam `exam_id`, in order.
pub async fn parts(
    pool: &PgPool,
    tr: Option<&str>,
    exam_id: i64,
) -> Result<Vec<PartRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT p.id, p.position::int AS position, p.title, p.statement_md,
                p.points::float8 AS points,
                t.title AS tr_title, t.statement_md AS tr_statement_md
           FROM ",
        published_exam_parts!(),
        " p
           LEFT JOIN ",
        exam_part_translations!(),
        " t ON t.exam_part_id = p.id AND t.lang = $1
          WHERE p.exam_id = $2
          ORDER BY p.position, p.id"
    ))
    .bind(tr)
    .bind(exam_id)
    .fetch_all(pool)
    .await
}

/// Levels and countries that published exams have, for list filters.
pub async fn facets(pool: &PgPool) -> Result<(Vec<String>, Vec<String>), sqlx::Error> {
    let (levels, countries): (Vec<String>, Vec<String>) = sqlx::query_as(concat!(
        "SELECT ARRAY(SELECT DISTINCT x.level::text FROM ",
        published_exams!(),
        " x ORDER BY 1),
                ARRAY(SELECT DISTINCT x.country::text FROM ",
        published_exams!(),
        " x ORDER BY 1)"
    ))
    .fetch_one(pool)
    .await?;
    Ok((levels, countries))
}
