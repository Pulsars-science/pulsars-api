//! What every public content endpoint shares (`docs/contract/README.md`):
//! caching headers and strong ETags, `304 Not Modified`, `Content-Language`,
//! the 404 problem that public caches may keep for a minute, the `406` for
//! clients that refuse JSON, and the per-field translation fallback.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::cursor;
use super::params::{Filters, Lang};
use crate::error::ApiError;

/// `200` on public content.
pub const CACHE_PUBLIC: &str = "public, max-age=300, stale-while-revalidate=60";
/// `404` on public content.
pub const CACHE_NOT_FOUND: &str = "public, max-age=60";

/// An error of a public content endpoint: a 404 may be cached briefly, every
/// other error is `no-store` (set by [`ApiError`]).
#[derive(Debug)]
pub struct PublicError(pub ApiError);

impl From<ApiError> for PublicError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl From<sqlx::Error> for PublicError {
    fn from(error: sqlx::Error) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for PublicError {
    fn into_response(self) -> Response {
        let mut response = self.0.into_response();
        if response.status() == StatusCode::NOT_FOUND {
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(CACHE_NOT_FOUND),
            );
        }
        response
    }
}

pub type PublicResult = Result<Response, PublicError>;

/// `404 not-found` for a public resource.
pub fn not_found(detail: String, path: &str) -> PublicError {
    PublicError(ApiError::NotFound {
        detail,
        instance: Some(path.to_owned()),
    })
}

/// `{"data": ...}`.
#[derive(Debug, Serialize)]
pub struct Data<T> {
    pub data: T,
}

/// `{"data": [...], "next_cursor": ...}`.
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
}

/// One page of a keyset-paginated list.
pub struct Pagination {
    pub limit: u32,
    /// Fingerprint of the endpoint, language and filters.
    pub fingerprint: String,
}

impl Pagination {
    pub fn new(endpoint: &str, lang: Lang, filters: &Filters, limit: u32) -> Self {
        Self {
            limit,
            fingerprint: cursor::fingerprint(&filters.canonical(endpoint, lang)),
        }
    }

    /// Rows to fetch: one more than the page, to know whether another
    /// page follows.
    pub fn fetch(&self) -> i64 {
        i64::from(self.limit) + 1
    }

    /// Cuts `rows` to the page and builds `next_cursor` from the last row
    /// kept, with `key`.
    pub fn page<R, T>(
        &self,
        mut rows: Vec<R>,
        key: impl Fn(&R) -> Vec<serde_json::Value>,
        item: impl FnMut(R) -> T,
    ) -> Page<T> {
        let limit = self.limit as usize;
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next_cursor = match rows.last() {
            Some(last) if more => Some(cursor::encode(key(last), &self.fingerprint)),
            _ => None,
        };
        Page {
            data: rows.into_iter().map(item).collect(),
            next_cursor,
        }
    }
}

/// The translated value when there is one, the original otherwise, and
/// whether the translation was used. Empty translations are already null in
/// the views; an empty string is treated the same here.
pub fn translated(original: String, translation: Option<String>) -> (String, bool) {
    match translation {
        Some(text) if !text.is_empty() => (text, true),
        _ => (original, false),
    }
}

/// The `lang` of an object whose main field was (or was not) translated.
pub fn served(translated: bool) -> &'static str {
    if translated { "en" } else { "fr" }
}

/// A `200` JSON response with the public caching headers, or `304` when the
/// client's `If-None-Match` already names this body.
pub fn json<T: Serialize>(request: &HeaderMap, lang: Lang, body: &T) -> PublicResult {
    let bytes = serde_json::to_vec(body)
        .map_err(|e| ApiError::Internal(format!("cannot serialize a response: {e}")))?;
    let etag = etag(&bytes);
    let not_modified = request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .any(|value| matches_etag(value, &etag));

    let mut response = if not_modified {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&etag) {
        headers.insert(header::ETAG, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_PUBLIC),
    );
    headers.insert(
        header::CONTENT_LANGUAGE,
        HeaderValue::from_static(lang.code()),
    );
    Ok(response)
}

/// Strong validator: the first 128 bits of the body's SHA-256, quoted.
fn etag(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

/// `If-None-Match` uses the weak comparison (RFC 9110 §13.1.2): `W/` is
/// ignored, `*` matches any current representation.
pub fn matches_etag(header: &HeaderValue, etag: &str) -> bool {
    let Ok(header) = header.to_str() else {
        return false;
    };
    header.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

/// Answers `406 not-acceptable` when `Accept` rules out JSON. An absent or
/// unparsable header accepts anything.
pub async fn require_json(request: Request, next: Next) -> Response {
    let acceptable = request
        .headers()
        .get_all(header::ACCEPT)
        .iter()
        .collect::<Vec<_>>();
    if acceptable.is_empty() || acceptable.iter().any(|v| accepts_json(v)) {
        next.run(request).await
    } else {
        ApiError::NotAcceptable.into_response()
    }
}

fn accepts_json(header: &HeaderValue) -> bool {
    let Ok(header) = header.to_str() else {
        return true;
    };
    header.split(',').any(|range| {
        let mut parts = range.split(';');
        let media = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let refused = parts.any(|p| {
            p.trim()
                .strip_prefix("q=")
                .and_then(|q| q.trim().parse::<f32>().ok())
                .is_some_and(|q| q <= 0.0)
        });
        !refused
            && matches!(
                media.as_str(),
                "*/*" | "application/*" | "application/json" | "application/problem+json"
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etags_are_strong_and_stable() {
        let a = etag(b"{\"data\":[]}");
        assert_eq!(a, etag(b"{\"data\":[]}"));
        assert_ne!(a, etag(b"{\"data\":[1]}"));
        assert!(a.starts_with('"') && a.ends_with('"'));
        assert_eq!(a.len(), 34);
    }

    #[test]
    fn if_none_match_uses_the_weak_comparison() {
        let tag = "\"abc\"";
        for header in ["\"abc\"", "W/\"abc\"", "\"x\", \"abc\"", "*"] {
            assert!(
                matches_etag(&HeaderValue::from_static(header), tag),
                "{header}"
            );
        }
        assert!(!matches_etag(&HeaderValue::from_static("\"abd\""), tag));
    }

    #[test]
    fn accept_must_allow_json() {
        for ok in [
            "*/*",
            "application/json",
            "text/html, application/json;q=0.5",
            "application/*",
            "Application/JSON",
            "application/problem+json",
        ] {
            assert!(accepts_json(&HeaderValue::from_static(ok)), "{ok}");
        }
        for refused in ["text/html", "application/xml", "application/json;q=0"] {
            assert!(
                !accepts_json(&HeaderValue::from_static(refused)),
                "{refused}"
            );
        }
    }

    #[test]
    fn translations_fall_back_per_field() {
        assert_eq!(
            translated("Les fractions".into(), Some("Fractions".into())),
            ("Fractions".to_owned(), true)
        );
        assert_eq!(
            translated("Les fractions".into(), Some(String::new())),
            ("Les fractions".to_owned(), false)
        );
        assert_eq!(
            translated("Les fractions".into(), None),
            ("Les fractions".to_owned(), false)
        );
    }

    #[test]
    fn pages_end_with_a_null_cursor() {
        let filters = super::super::params::Params::parse(None)
            .finish()
            .expect("valid");
        let pagination = Pagination::new("exercises", Lang::Fr, &filters, 2);
        assert_eq!(pagination.fetch(), 3);
        let key = |n: &i64| vec![serde_json::Value::from(*n)];
        let full = pagination.page(vec![1, 2, 3], key, |n| n);
        assert_eq!(full.data, vec![1, 2]);
        let next = full.next_cursor.expect("another page");
        assert_eq!(
            cursor::id_key(&cursor::decode(&next, &pagination.fingerprint).expect("valid"))
                .expect("shape"),
            2
        );
        let last = pagination.page(vec![3], key, |n| n);
        assert_eq!(last.next_cursor, None);
    }
}
