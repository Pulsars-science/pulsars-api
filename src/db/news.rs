//! Published news of the team (`contenu.actualites`), through
//! `published_news!()` and `news_translations!()` (`super::sources`).

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Columns of a news item, `$extra` after them, with its translation (`t`).
macro_rules! news_select {
    ($extra:literal) => {
        concat!(
            "SELECT a.id, a.slug, a.title, a.summary::text AS summary,
                    a.author_pseudo::text AS author_pseudo,
                    a.published_at, a.updated_at,
                    t.title AS tr_title, t.summary AS tr_summary",
            $extra,
            "
               FROM ",
            published_news!(),
            " a
               LEFT JOIN ",
            news_translations!(),
            " t ON t.news_id = a.id AND t.lang = $1"
        )
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NewsRow {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub author_pseudo: Option<String>,
    pub published_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tr_title: Option<String>,
    pub tr_summary: Option<String>,
}

/// One page of published news, newest first, after `after`. `search` is a
/// `LIKE` pattern.
pub async fn list(
    pool: &PgPool,
    tr: Option<&str>,
    search: Option<&str>,
    after: Option<(DateTime<Utc>, i64)>,
    limit: i64,
) -> Result<Vec<NewsRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        news_select!(""),
        "
          WHERE ($2::text IS NULL OR a.title ILIKE $2 OR a.summary ILIKE $2
                 OR t.title ILIKE $2 OR t.summary ILIKE $2)
            AND ($3::timestamptz IS NULL OR (a.published_at, a.id) < ($3::timestamptz, $4::bigint))
          ORDER BY a.published_at DESC, a.id DESC
          LIMIT $5"
    ))
    .bind(tr)
    .bind(search)
    .bind(after.map(|(at, _)| at))
    .bind(after.map(|(_, id)| id))
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NewsDetailRow {
    #[sqlx(flatten)]
    pub news: NewsRow,
    pub content_md: String,
    pub tr_content_md: Option<String>,
}

/// The published news item `slug`, with its content.
pub async fn find(
    pool: &PgPool,
    tr: Option<&str>,
    slug: &str,
) -> Result<Option<NewsDetailRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        news_select!(", a.content_md, t.content_md AS tr_content_md"),
        " WHERE a.slug = $2"
    ))
    .bind(tr)
    .bind(slug)
    .fetch_optional(pool)
    .await
}
