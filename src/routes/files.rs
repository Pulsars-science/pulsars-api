//! `GET /v1/exams/{slug}/pdf` and `/v1/resources/{slug}/file`
//! (`openapi.yaml`, tags `exams` and `resources`): the PDF of a published
//! exam statement and the file of a published resource, read from
//! `contenu.fichiers`.
//!
//! Never a correction, never a draft's file, never a storage path: the file
//! is found through the published content's slug only. The exam's
//! correction PDF has its own route, `GET /v1/exams/{slug}/correction-pdf`.
//!
//! An exam file asked with `disposition=attachment` is a download, counted
//! as Laravel's `/examens/{exam}/telecharger` and `/corrige` counted them
//! (`super::corrections::count_download`), and is never cached; `inline`
//! is the preview (`apercu-pdf`), not counted.

use axum::body::Body;
use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;

use super::auth::Caller;
use super::params::Params;
use super::public::{self, PublicResult};
use crate::auth::member::{MaybeMember, Member};
use crate::db::files::{self, FileMeta};
use crate::error::ApiError;
use crate::state::AppState;

/// `disposition`: shown in the browser (default) or saved.
const DISPOSITIONS: &[&str] = &["inline", "attachment"];

/// A stored file's name changes with its content: an hour in shared caches.
const CACHE_FILE: &str = "public, max-age=3600";

/// Uploaded by a member: never interpreted as a page (as Laravel's
/// `FichierPublicController`).
const FILE_CSP: &str = "default-src 'none'; sandbox";

pub async fn exam_pdf(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let disposition = disposition(query.as_deref())?;
    let meta = files::exam_statement(&state.db, &slug)
        .await?
        .ok_or_else(|| {
            public::not_found(
                format!("No published exam '{slug}' has a hosted PDF."),
                uri.path(),
            )
        })?;
    exam_file(
        &state,
        member,
        &caller,
        meta,
        &slug,
        &slug,
        disposition,
        &headers,
    )
    .await
}

/// `GET /v1/exams/{slug}/correction-pdf`: the correction written by
/// Pulsars, public like the corrections of the exam page.
pub async fn exam_correction_pdf(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let disposition = disposition(query.as_deref())?;
    let meta = files::exam_correction(&state.db, &slug)
        .await?
        .ok_or_else(|| {
            public::not_found(
                format!("No published exam '{slug}' has a correction PDF."),
                uri.path(),
            )
        })?;
    let name = format!("{slug}-corrige");
    exam_file(
        &state,
        member,
        &caller,
        meta,
        &slug,
        &name,
        disposition,
        &headers,
    )
    .await
}

/// Serves an exam PDF; a download (`attachment`) is counted, unless it
/// resumes a partial one, and never cached.
#[allow(clippy::too_many_arguments)]
async fn exam_file(
    state: &AppState,
    member: Option<Member>,
    caller: &Caller,
    meta: FileMeta,
    slug: &str,
    name: &str,
    disposition: &str,
    headers: &HeaderMap,
) -> PublicResult {
    let download = disposition == "attachment";
    let resumed = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !v.trim().starts_with("bytes=0-"));
    if download && !resumed {
        super::corrections::count_download(state, slug, member.map(|m| m.id()), caller).await?;
    }
    // Exam files are PDF whatever was recorded at upload.
    let mut response = serve(state, meta, "application/pdf", name, disposition, headers).await?;
    if download {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
    }
    Ok(response)
}

pub async fn resource_file(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let disposition = disposition(query.as_deref())?;
    let meta = files::resource(&state.db, &slug).await?.ok_or_else(|| {
        public::not_found(
            format!("No published resource '{slug}' has a hosted file."),
            uri.path(),
        )
    })?;
    let mime = meta.mime.clone();
    serve(&state, meta, &mime, &slug, disposition, &headers).await
}

fn disposition(query: Option<&str>) -> Result<&'static str, ApiError> {
    let mut params = Params::parse(query);
    let disposition = params.one_of("disposition", DISPOSITIONS);
    params.finish()?;
    Ok(disposition.unwrap_or("inline"))
}

async fn serve(
    state: &AppState,
    meta: FileMeta,
    mime: &str,
    slug: &str,
    disposition: &str,
    request: &HeaderMap,
) -> PublicResult {
    let size = u64::try_from(meta.size).unwrap_or(0);
    let etag = format!(
        "\"f{}-{}-{size}\"",
        meta.file_id,
        meta.updated_at.map_or(0, |at| at.timestamp())
    );

    let not_modified = request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .any(|value| public::matches_etag(value, &etag));
    if not_modified {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        common_headers(response.headers_mut(), &etag);
        return Ok(response);
    }

    // `If-Range` with another validator: the client's partial copy is stale,
    // send the whole file (RFC 9110 §13.1.5).
    let range_applies = request
        .get(header::IF_RANGE)
        .is_none_or(|value| value.to_str().is_ok_and(|v| v.trim() == etag));
    let range = match request.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        Some(value) if range_applies => parse_range(value, size)?,
        _ => None,
    };

    let (status, start, len) = match range {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end - start + 1),
        None => (StatusCode::OK, 0, size),
    };
    let bytes = if len == 0 {
        Vec::new()
    } else {
        files::read(
            &state.db,
            meta.file_id,
            i64::try_from(start).unwrap_or(i64::MAX),
            i64::try_from(len).unwrap_or(i64::MAX),
        )
        .await?
    };

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    common_headers(headers, &etag);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    if let Ok(value) = HeaderValue::from_str(&content_disposition(disposition, slug, mime)) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    if status == StatusCode::PARTIAL_CONTENT
        && let Ok(value) =
            HeaderValue::from_str(&format!("bytes {start}-{}/{size}", start + len - 1))
    {
        headers.insert(header::CONTENT_RANGE, value);
    }
    Ok(response)
}

fn common_headers(headers: &mut HeaderMap, etag: &str) {
    if let Ok(value) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE_FILE));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(FILE_CSP),
    );
}

/// `inline; filename="bac-2026.pdf"`: the slug is ASCII (letters, digits,
/// dashes), so it needs no quoting beyond the quotes.
fn content_disposition(disposition: &str, slug: &str, mime: &str) -> String {
    let safe: String = slug
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    let extension = if mime == "application/pdf" {
        ".pdf"
    } else {
        ""
    };
    format!("{disposition}; filename=\"{safe}{extension}\"")
}

/// One byte range of a file of `size` bytes (RFC 9110 §14.1.2), as
/// inclusive `(start, end)`. `None`: serve the whole file (no range, several
/// ranges, another unit, or a syntax error, which the RFC says to ignore).
/// A range that starts past the end is a 416.
fn parse_range(value: &str, size: u64) -> Result<Option<(u64, u64)>, ApiError> {
    let Some(spec) = value.trim().strip_prefix("bytes=") else {
        return Ok(None);
    };
    if spec.contains(',') {
        return Ok(None);
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return Ok(None);
    };
    let (first, last) = (first.trim(), last.trim());
    let range = match (first.is_empty(), last.is_empty()) {
        // `bytes=-500`: the last 500 bytes.
        (true, false) => {
            let Ok(suffix) = last.parse::<u64>() else {
                return Ok(None);
            };
            if suffix == 0 || size == 0 {
                return Err(ApiError::RangeNotSatisfiable { size });
            }
            (size.saturating_sub(suffix), size - 1)
        }
        // `bytes=500-` or `bytes=500-999`.
        (false, _) => {
            let Ok(start) = first.parse::<u64>() else {
                return Ok(None);
            };
            let end = if last.is_empty() {
                u64::MAX
            } else {
                match last.parse::<u64>() {
                    Ok(end) if end >= start => end,
                    _ => return Ok(None),
                }
            };
            if start >= size {
                return Err(ApiError::RangeNotSatisfiable { size });
            }
            (start, end.min(size - 1))
        }
        (true, true) => return Ok(None),
    };
    Ok(Some(range))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(value: &str, size: u64) -> Option<(u64, u64)> {
        parse_range(value, size).expect("satisfiable")
    }

    #[test]
    fn single_ranges_are_clamped_to_the_file() {
        assert_eq!(range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(range("bytes=900-5000", 1000), Some((900, 999)));
        assert_eq!(range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(range("bytes=-5000", 1000), Some((0, 999)));
        assert_eq!(range(" bytes= 10 - 19 ", 1000), Some((10, 19)));
    }

    #[test]
    fn what_cannot_be_served_as_one_range_is_the_whole_file() {
        for value in [
            "bytes=0-1,5-6",
            "items=0-1",
            "bytes=",
            "bytes=-",
            "bytes=a-b",
            "bytes=20-10",
            "0-10",
        ] {
            assert_eq!(range(value, 1000), None, "{value}");
        }
    }

    #[test]
    fn a_range_past_the_end_is_416() {
        for value in ["bytes=1000-", "bytes=5000-6000", "bytes=-0"] {
            assert!(
                matches!(
                    parse_range(value, 1000),
                    Err(ApiError::RangeNotSatisfiable { size: 1000 })
                ),
                "{value}"
            );
        }
    }

    #[test]
    fn file_names_come_from_the_slug() {
        assert_eq!(
            content_disposition("inline", "bac-2026", "application/pdf"),
            "inline; filename=\"bac-2026.pdf\""
        );
        assert_eq!(
            content_disposition("attachment", "a\"b/c", "text/plain"),
            "attachment; filename=\"abc\""
        );
    }
}
