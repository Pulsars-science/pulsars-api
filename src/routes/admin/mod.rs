//! `/v1/admin/*` (`openapi.yaml`, tag `admin`): the administration of the
//! site, as Laravel's `admin/*` routes had it. Every route takes an
//! [`AdminMember`](crate::auth::member::AdminMember) (Laravel's `auth`,
//! `verified`, `onboarding` and `admin` middlewares); giving or taking a
//! permission role also needs a super admin (`roles`).
//!
//! The site calls these from its server, like `/v1/auth/*`: no CORS, every
//! answer `Cache-Control: no-store`. Lists are numbered pages (`page`, a
//! fixed `per_page`, `total`) with the per-status `counts` the admin pages
//! show in their tabs.

/// `format!` for a query built from this module's constants only (never
/// from input), which sqlx asks to assert.
macro_rules! sql {
    ($($arg:tt)*) => {
        sqlx::AssertSqlSafe(format!($($arg)*))
    };
}

pub mod courses;
pub mod dashboard;
pub mod exams;
pub mod news;
pub mod queues;
pub mod requests;
pub mod roles;

use axum::body::Body;
use axum::extract::multipart::{Multipart, MultipartError};
use axum::extract::{FromRequest, Request};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Serialize;

use super::auth::{Data, no_store};
use super::params::Params;
use crate::error::{ApiError, ParameterError};

/// One page of an admin list.
#[derive(Debug, Serialize)]
pub struct Listing<T, C: Serialize> {
    pub data: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    /// Rows matching the filters, over every page.
    pub total: i64,
    /// Rows per status, whatever the filters (the tabs of the page).
    pub counters: C,
}

/// `page`, from 1.
pub(crate) fn page(params: &mut Params) -> u32 {
    params.page(false).unwrap_or(1)
}

pub(crate) fn offset(page: u32, per_page: u32) -> i64 {
    i64::from(page.saturating_sub(1)) * i64::from(per_page)
}

/// `200 {"data": …}`, never stored.
pub(crate) fn ok<T: Serialize>(data: T) -> Response {
    no_store(StatusCode::OK, &Data { data })
}

/// A [`Listing`], never stored.
pub(crate) fn listing<T: Serialize, C: Serialize>(listing: Listing<T, C>) -> Response {
    no_store(StatusCode::OK, &listing)
}

/// `201 {"data": …}`, never stored.
pub(crate) fn created<T: Serialize>(data: T) -> Response {
    no_store(StatusCode::CREATED, &Data { data })
}

pub(crate) fn not_found(what: &str, id: i64, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: format!("No {what} has the id {id}."),
        instance: Some(path.to_owned()),
    }
}

/// The id of the row of `table` whose slug is `slug` (Laravel binds news,
/// courses and exams by slug), or a 404.
pub(crate) async fn resolve(
    db: &sqlx::PgPool,
    table: &'static str,
    what: &str,
    slug: &str,
    path: &str,
) -> Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(sql!("SELECT id FROM {table} WHERE slug = $1"))
        .bind(slug)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::NotFound {
            detail: format!("No {what} has the slug '{slug}'."),
            instance: Some(path.to_owned()),
        })
}

/// A 422 for an action the row's state does not allow, on the body as a
/// whole (`parameter` empty), with its `code`.
pub(crate) fn refused(code: &'static str, detail: &str) -> ApiError {
    ApiError::InvalidFields(vec![ParameterError::field("", code, detail)])
}

/// A 409 for an action the row's state does not allow (Laravel's
/// `back()->with('erreur', …)`).
pub(crate) fn conflict(reason: &'static str, detail: &str) -> ApiError {
    ApiError::Conflict {
        reason,
        detail: detail.to_owned(),
    }
}

/// Field checks of a JSON body, with Laravel's `TrimStrings` and
/// `ConvertEmptyStringsToNull`: values are trimmed, and an empty one is
/// absent.
#[derive(Debug, Default)]
pub(crate) struct Fields {
    errors: Vec<ParameterError>,
}

impl Fields {
    /// A required string of `min` to `max` characters (`required|string|min|max`).
    pub fn text(&mut self, name: &str, value: Option<&str>, min: usize, max: usize) -> String {
        match self.optional(name, value, max) {
            None => {
                self.push(name, "required", "is required");
                String::new()
            }
            Some(text) if text.chars().count() < min => {
                self.push(
                    name,
                    "too_short",
                    format!("must be at least {min} characters"),
                );
                text
            }
            Some(text) => text,
        }
    }

    /// An optional string of at most `max` characters (`nullable|string|max`).
    pub fn optional(&mut self, name: &str, value: Option<&str>, max: usize) -> Option<String> {
        let text = value.map(str::trim).filter(|v| !v.is_empty())?;
        if text.chars().count() > max {
            self.push(
                name,
                "too_long",
                format!("must be at most {max} characters"),
            );
        }
        Some(text.to_owned())
    }

    /// One of `allowed` (`required|in:…`).
    pub fn one_of(&mut self, name: &str, value: Option<&str>, allowed: &[&str]) -> String {
        match value.map(str::trim).filter(|v| !v.is_empty()) {
            None => {
                self.push(name, "required", "is required");
                String::new()
            }
            Some(v) if allowed.contains(&v) => v.to_owned(),
            Some(v) => {
                self.push(
                    name,
                    "invalid",
                    format!("must be one of: {}", allowed.join(", ")),
                );
                v.to_owned()
            }
        }
    }

    /// A number between `min` and `max` (`numeric|min|max`), required.
    pub fn number(&mut self, name: &str, value: Option<f64>, min: f64, max: f64) -> f64 {
        match value {
            None => {
                self.push(name, "required", "is required");
                0.0
            }
            Some(n) if !n.is_finite() || n < min => {
                self.push(name, "too_small", format!("must be at least {min}"));
                n
            }
            Some(n) if n > max => {
                self.push(name, "too_large", format!("must be at most {max}"));
                n
            }
            Some(n) => n,
        }
    }

    pub fn push(&mut self, name: &str, code: &'static str, detail: impl Into<String>) {
        self.errors.push(ParameterError::field(name, code, detail));
    }

    pub fn finish(self) -> Result<(), ApiError> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(ApiError::InvalidFields(self.errors))
        }
    }
}

/// A slug for `title` that no row of `table` has yet, as Laravel's
/// `slugUnique()`: the slug of its first 60 characters, else `fallback`,
/// then `-2`, `-3`… `except` is a row that may keep its own slug.
pub(crate) async fn unique_slug(
    db: impl sqlx::PgExecutor<'_> + Copy,
    table: &'static str,
    title: &str,
    limit: Option<usize>,
    fallback: &str,
    except: Option<i64>,
) -> Result<String, sqlx::Error> {
    let cut: String = match limit {
        Some(n) => title
            .chars()
            .take(n)
            .collect::<String>()
            .trim_end()
            .to_owned(),
        None => title.to_owned(),
    };
    let base: String = sqlx::query_scalar("SELECT contenu.slugify($1)")
        .bind(&cut)
        .fetch_one(db)
        .await?;
    let base = if base.is_empty() {
        fallback.to_owned()
    } else {
        base
    };
    // `table` is one of the constants of the callers, never input.
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE slug = $1 AND ($2::bigint IS NULL OR id <> $2))"
    );
    let mut slug = base.clone();
    let mut i = 2;
    while sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.clone()))
        .bind(&slug)
        .bind(except)
        .fetch_one(db)
        .await?
    {
        slug = format!("{base}-{i}");
        i += 1;
    }
    Ok(slug)
}

/// Laravel's `local` disk, stored in `contenu.fichiers`.
pub(crate) const DISK: &str = "local";

/// A `multipart/form-data` body (the uploads). Any other `Content-Type` is a
/// 415, like a non-JSON body elsewhere.
pub struct Form(pub Multipart);

impl<S: Send + Sync> FromRequest<S> for Form {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        Multipart::from_request(request, state)
            .await
            .map(Self)
            .map_err(|_| ApiError::UnsupportedMediaType)
    }
}

/// A multipart body that cannot be read: 413 over the route's limit, 400
/// otherwise.
pub(crate) fn multipart_error(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::PayloadTooLarge
    } else {
        ApiError::InvalidBody(Vec::new())
    }
}

/// `count` random letters and digits, as Laravel's `Str::random()` names
/// stored files.
pub(crate) fn random_name(count: usize) -> Result<String, ApiError> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = vec![0u8; count];
    getrandom::fill(&mut bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness for a file name: {e}")))?;
    // 248 = 4 × 62: no modulo bias.
    let mut name = String::with_capacity(count);
    let mut pool = bytes.into_iter();
    while name.len() < count {
        let byte = match pool.next() {
            Some(b) => b,
            None => {
                let mut one = [0u8; 1];
                getrandom::fill(&mut one)
                    .map_err(|e| ApiError::Internal(format!("no randomness: {e}")))?;
                one[0]
            }
        };
        if byte < 248 {
            name.push(char::from(ALPHABET[usize::from(byte % 62)]));
        }
    }
    Ok(name)
}

/// Stores `content` at `path` on the `local` disk.
pub(crate) async fn store_file(
    db: impl sqlx::PgExecutor<'_>,
    path: &str,
    content: &[u8],
    mime: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO contenu.fichiers (disque, chemin, contenu, type_mime, taille, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC'))",
    )
    .bind(DISK)
    .bind(path)
    .bind(content)
    .bind(mime)
    .bind(i64::try_from(content.len()).unwrap_or(i64::MAX))
    .execute(db)
    .await?;
    Ok(())
}

/// The whole PDF at `path` on the `local` disk, shown in the browser under
/// `file_name`, or `None` when it is not stored.
pub(crate) async fn pdf_response(
    db: &sqlx::PgPool,
    path: &str,
    file_name: &str,
) -> Result<Option<Response>, ApiError> {
    let content: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT contenu FROM contenu.fichiers WHERE disque = $1 AND chemin = $2",
    )
    .bind(DISK)
    .bind(path)
    .fetch_optional(db)
    .await?;
    let Some(content) = content else {
        return Ok(None);
    };
    let mut response = Response::new(Body::from(content));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    // Uploaded by someone: never interpreted as a page.
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    if let Ok(value) = HeaderValue::from_str(&inline_disposition(file_name)) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(Some(response))
}

/// `inline; filename="…"; filename*=UTF-8''…` (RFC 6266): an ASCII
/// fallback, then the real name percent-encoded.
fn inline_disposition(file_name: &str) -> String {
    let fallback: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "inline; filename=\"{fallback}\"; filename*=UTF-8''{}",
        crate::links::encode(file_name, false)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(fields: Fields) -> Vec<(String, &'static str)> {
        match fields.finish() {
            Ok(()) => Vec::new(),
            Err(ApiError::InvalidFields(errors)) => errors
                .into_iter()
                .map(|e| (e.parameter, e.code.unwrap_or_default()))
                .collect(),
            Err(other) => panic!("{other:?}"),
        }
    }

    #[test]
    fn stored_names_are_random_letters_and_digits() {
        let name = random_name(40).unwrap();
        assert_eq!(name.len(), 40);
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_ne!(name, random_name(40).unwrap());
    }

    #[test]
    fn dispositions_keep_the_real_name() {
        assert_eq!(
            inline_disposition("Cours d’été.pdf"),
            "inline; filename=\"Cours d__t_.pdf\"; filename*=UTF-8''Cours%20d%E2%80%99%C3%A9t%C3%A9.pdf"
        );
    }

    #[test]
    fn strings_are_trimmed_and_empty_ones_absent() {
        let mut f = Fields::default();
        assert_eq!(
            f.text("title", Some("  Hello world  "), 5, 150),
            "Hello world"
        );
        assert_eq!(f.optional("summary", Some("   "), 10), None);
        assert!(codes(f).is_empty());

        let mut f = Fields::default();
        f.text("title", Some("   "), 5, 150);
        f.text("body", Some("abc"), 5, 150);
        f.optional("summary", Some(&"x".repeat(11)), 10);
        f.one_of("difficulty", Some("expert"), &["debutant"]);
        f.number("amount", Some(-1.0), 0.0, 10.0);
        f.number("raised", None, 0.0, 10.0);
        assert_eq!(
            codes(f),
            [
                ("title".to_owned(), "required"),
                ("body".to_owned(), "too_short"),
                ("summary".to_owned(), "too_long"),
                ("difficulty".to_owned(), "invalid"),
                ("amount".to_owned(), "too_small"),
                ("raised".to_owned(), "required"),
            ]
        );
    }
}
