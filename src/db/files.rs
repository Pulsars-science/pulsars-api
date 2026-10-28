//! Files stored in `contenu.fichiers` (bytea), through
//! `exam_statement_files!()` and `resource_files!()` (`super::sources`):
//! only those of published content, looked up by the content's slug.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// What is known of a file before reading it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileMeta {
    pub file_id: i64,
    pub size: i64,
    pub mime: String,
    pub updated_at: Option<DateTime<Utc>>,
}

/// The statement PDF of the published exam `slug`.
pub async fn exam_statement(pool: &PgPool, slug: &str) -> Result<Option<FileMeta>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT file_id, size, mime, updated_at FROM ",
        exam_statement_files!(),
        " f WHERE f.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// The hosted file of the published resource `slug`.
pub async fn resource(pool: &PgPool, slug: &str) -> Result<Option<FileMeta>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT file_id, size, mime, updated_at FROM ",
        resource_files!(),
        " f WHERE f.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// `len` bytes of file `file_id` from offset `start` (0-based). Only that
/// slice leaves the database.
pub async fn read(
    pool: &PgPool,
    file_id: i64,
    start: i64,
    len: i64,
) -> Result<Vec<u8>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT substring(contenu FROM $2::int + 1 FOR $3::int)
           FROM contenu.fichiers WHERE id = $1",
    )
    .bind(file_id)
    .bind(start)
    .bind(len)
    .fetch_one(pool)
    .await
}
