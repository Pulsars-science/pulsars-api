//! Published free resources, from `contenu` through `published_resources!()`,
//! `resource_translations!()` and `contenu.licences` (`super::sources`,
//! `docs/contract/database.md` §4).

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Resources without `created_at` (never the case with Eloquent) sort as
/// the oldest.
macro_rules! resource_select {
    () => {
        concat!(
            "SELECT r.id, r.slug, r.title, r.description, r.category::text AS category,
                r.doc_lang::text AS doc_lang, r.mode, r.attribution::text AS attribution,
                r.license_key::text AS license_key,
                r.license_custom::text AS license_custom,
                l.name AS license_name, l.url AS license_url,
                r.source_url::text AS source_url, r.size_bytes,
                coalesce(r.created_at, 'epoch'::timestamptz) AS created_at, r.updated_at,
                t.title AS tr_title, t.description AS tr_description,
                t.category AS tr_category
           FROM ",
            published_resources!(),
            " r
           LEFT JOIN ",
            licenses!(),
            " l ON l.key = r.license_key
           LEFT JOIN ",
            resource_translations!(),
            " t ON t.resource_id = r.id AND t.lang = $1"
        )
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResourceRow {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub doc_lang: Option<String>,
    /// `host` or `link`.
    pub mode: String,
    pub attribution: Option<String>,
    pub license_key: Option<String>,
    /// Free text, meaningful only for the key `autre`.
    pub license_custom: Option<String>,
    /// From `contenu.licences`; null for a key it does not know.
    pub license_name: Option<String>,
    pub license_url: Option<String>,
    pub source_url: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tr_title: Option<String>,
    pub tr_description: Option<String>,
    pub tr_category: Option<String>,
}

/// Filters of `GET /v1/resources`. `search` is a `LIKE` pattern.
#[derive(Debug, Default)]
pub struct ResourceFilters<'a> {
    /// Original (French) category.
    pub category: Option<&'a str>,
    pub doc_lang: Option<&'a str>,
    pub mode: Option<&'a str>,
    pub license: Option<&'a str>,
    pub search: Option<&'a str>,
}

/// One page of resources, newest first, after `after`.
pub async fn list(
    pool: &PgPool,
    tr: Option<&str>,
    filters: &ResourceFilters<'_>,
    after: Option<(DateTime<Utc>, i64)>,
    limit: i64,
) -> Result<Vec<ResourceRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        resource_select!(),
        "
          WHERE ($2::text IS NULL OR r.category = $2)
            AND ($3::text IS NULL OR r.doc_lang = $3)
            AND ($4::text IS NULL OR r.mode = $4)
            AND ($5::text IS NULL OR r.license_key = $5)
            AND ($6::text IS NULL OR r.title ILIKE $6 OR r.description ILIKE $6
                 OR t.title ILIKE $6 OR t.description ILIKE $6)
            AND ($7::timestamptz IS NULL
                 OR (coalesce(r.created_at, 'epoch'::timestamptz), r.id) < ($7::timestamptz, $8::bigint))
          ORDER BY coalesce(r.created_at, 'epoch'::timestamptz) DESC, r.id DESC
          LIMIT $9"
    ))
    .bind(tr)
    .bind(filters.category)
    .bind(filters.doc_lang)
    .bind(filters.mode)
    .bind(filters.license)
    .bind(filters.search)
    .bind(after.map(|(at, _)| at))
    .bind(after.map(|(_, id)| id))
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// The published resource `slug`.
pub async fn find(
    pool: &PgPool,
    tr: Option<&str>,
    slug: &str,
) -> Result<Option<ResourceRow>, sqlx::Error> {
    sqlx::query_as(concat!(resource_select!(), " WHERE r.slug = $2"))
        .bind(tr)
        .bind(slug)
        .fetch_optional(pool)
        .await
}
