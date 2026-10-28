//! Uploads (`multipart/form-data`): the resource a member proposes, the
//! image of a suggestion, an avatar. Files are stored as `bytea` in
//! `contenu.fichiers`, under a random name, on Laravel's disks (`local`:
//! never served directly; `public`: served by `GET /v1/storage/{path}`).
//!
//! Body limits: these routes have their own (`UPLOAD_BODY_LIMIT`), above
//! the largest file allowed, so that a file a little too large gets its
//! 422 `too_large`, and anything larger a 413. nginx must let them through
//! (`client_max_body_size`, `docs/deployment.md`).

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::multipart::{Multipart, MultipartError};
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use crate::error::{ApiError, ParameterError};

/// Body limit of the upload routes: the largest file (20 MiB, a proposed
/// resource) and 1 MiB for the other fields and the multipart framing.
pub const UPLOAD_BODY_LIMIT: usize = 21 * 1024 * 1024;

/// Longest text field read.
const FIELD_MAX_BYTES: usize = 64 * 1024;

/// A file of the form.
#[derive(Debug, Clone)]
pub struct Upload {
    /// The name the browser gave, if any.
    pub file_name: Option<String>,
    pub bytes: Vec<u8>,
}

/// The fields and files of a form read whole ([`read`]). A file over its
/// limit is listed in `too_large` instead.
#[derive(Debug, Default)]
pub struct Fields {
    pub fields: HashMap<String, String>,
    pub files: HashMap<String, Upload>,
    pub too_large: Vec<String>,
}

impl Fields {
    /// A text field, trimmed (Laravel's `TrimStrings`); empty is absent.
    pub fn text(&self, name: &str) -> Option<String> {
        self.fields
            .get(name)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }
}

/// Reads a `multipart/form-data` body; each file at most `max_file`
/// bytes. 415 for another content type.
pub async fn read<S: Send + Sync>(
    request: Request,
    state: &S,
    max_file: usize,
) -> Result<Fields, ApiError> {
    let declared = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > UPLOAD_BODY_LIMIT) {
        return Err(ApiError::PayloadTooLarge);
    }
    let Form(mut multipart) = Form::from_request(request, state).await?;
    let mut form = Fields::default();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => return Err(multipart_error(error)),
        };
        let name = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().map(str::to_owned);
        let is_file = file_name.is_some();
        let max = if is_file { max_file } else { FIELD_MAX_BYTES };
        let mut bytes = Vec::new();
        let mut over = false;
        let mut field = field;
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    if !over && bytes.len() + chunk.len() > max {
                        over = true;
                        bytes = Vec::new();
                    }
                    if !over {
                        bytes.extend_from_slice(&chunk);
                    }
                }
                Ok(None) => break,
                Err(error) => return Err(multipart_error(error)),
            }
        }
        if over {
            form.too_large.push(name);
        } else if is_file {
            // A file input left empty is sent as a part with no content.
            if !bytes.is_empty() {
                form.files.insert(name, Upload { file_name, bytes });
            }
        } else {
            let text = String::from_utf8(bytes).map_err(|_| {
                ApiError::InvalidBody(vec![ParameterError {
                    parameter: name.clone(),
                    detail: "must be UTF-8 text".to_owned(),
                    code: None,
                }])
            })?;
            form.fields.insert(name, text);
        }
    }
    Ok(form)
}

/// Laravel's `local` disk, stored in `contenu.fichiers`.
pub(crate) const DISK: &str = "local";

/// A `multipart/form-data` body, to read field by field. Any other
/// `Content-Type` is a 415, like a non-JSON body elsewhere.
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

/// A multipart body that cannot be read: 413 when cut by the route's limit
/// (without `Content-Length`, the limit is only met while reading; the
/// cause is then deep in the error's chain), 400 otherwise.
pub(crate) fn multipart_error(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE || cut_by_limit(&error) {
        ApiError::PayloadTooLarge
    } else {
        ApiError::InvalidBody(Vec::new())
    }
}

/// Whether an error, or one of its causes, is the body limit's.
fn cut_by_limit(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(e) = current {
        if e.to_string().contains("length limit") {
            return true;
        }
        current = e.source();
    }
    false
}

/// A PDF, by its magic number (`%PDF-`), as `finfo` detects
/// `application/pdf`.
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

/// An image of the web formats Laravel accepted (`mimes:jpg,jpeg,png,webp,
/// gif`), by its real content: its type, extension and dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image {
    pub mime: &'static str,
    pub extension: &'static str,
    pub width: u32,
    pub height: u32,
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(*b.get(at)?) << 8 | u32::from(*b.get(at + 1)?))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(*b.get(at)?) | u32::from(*b.get(at + 1)?) << 8)
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
    Some(le16(b, at)? | u32::from(*b.get(at + 2)?) << 16)
}

pub fn image(bytes: &[u8]) -> Option<Image> {
    let b = bytes;
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let width = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
        let height = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
        return Some(Image {
            mime: "image/png",
            extension: "png",
            width,
            height,
        });
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Some(Image {
            mime: "image/gif",
            extension: "gif",
            width: le16(b, 6)?,
            height: le16(b, 8)?,
        });
    }
    if b.starts_with(b"\xff\xd8\xff") {
        // Segments up to the first frame header (SOFn).
        let mut at = 2;
        while at + 9 < b.len() {
            if b[at] != 0xff {
                return None;
            }
            let marker = b[at + 1];
            if marker == 0xff {
                at += 1;
                continue;
            }
            let len = be16(b, at + 2)? as usize;
            let sof = matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
            if sof {
                return Some(Image {
                    mime: "image/jpeg",
                    extension: "jpg",
                    height: be16(b, at + 5)?,
                    width: be16(b, at + 7)?,
                });
            }
            at += 2 + len;
        }
        return None;
    }
    if b.len() >= 30 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        let (width, height) = match &b[12..16] {
            b"VP8 " => (le16(b, 26)? & 0x3fff, le16(b, 28)? & 0x3fff),
            b"VP8L" => {
                let (b1, b2, b3, b4) = (
                    u32::from(b[21]),
                    u32::from(b[22]),
                    u32::from(b[23]),
                    u32::from(b[24]),
                );
                (
                    1 + (((b2 & 0x3f) << 8) | b1),
                    1 + (((b4 & 0x0f) << 10) | (b3 << 2) | ((b2 & 0xc0) >> 6)),
                )
            }
            b"VP8X" => (1 + le24(b, 24)?, 1 + le24(b, 27)?),
            _ => return None,
        };
        return Some(Image {
            mime: "image/webp",
            extension: "webp",
            width,
            height,
        });
    }
    None
}

/// `count` random letters and digits, as Laravel's `Str::random()` names
/// stored files.
pub(crate) fn random_name(count: usize) -> Result<String, ApiError> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut name = String::with_capacity(count);
    while name.len() < count {
        let mut bytes = vec![0u8; count];
        getrandom::fill(&mut bytes)
            .map_err(|e| ApiError::Internal(format!("no randomness for a file name: {e}")))?;
        // 248 = 4 × 62: no modulo bias.
        for byte in bytes.into_iter().filter(|b| *b < 248) {
            if name.len() < count {
                name.push(char::from(ALPHABET[usize::from(byte % 62)]));
            }
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
    store(db, DISK, path, content, mime).await
}

/// Stores a file on a disk (`local` or `public`) under `path`.
pub async fn store(
    conn: impl sqlx::PgExecutor<'_>,
    disk: &str,
    path: &str,
    bytes: &[u8],
    mime: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO contenu.fichiers (disque, chemin, contenu, type_mime, taille, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, date_trunc('second', now() AT TIME ZONE 'UTC'),
                 date_trunc('second', now() AT TIME ZONE 'UTC'))",
    )
    .bind(disk)
    .bind(path)
    .bind(bytes)
    .bind(mime)
    .bind(i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    .execute(conn)
    .await?;
    Ok(())
}

/// `GET /v1/storage/{path}`: a file of Laravel's `public` disk (avatars,
/// suggestion images), as `FichierPublicController`. Its name is random and
/// its content never changes: cached for a year. Uploaded by a member:
/// never interpreted as a page.
pub async fn public_file(pool: &PgPool, path: &str, instance: &str) -> Response {
    let row: Result<Option<(Vec<u8>, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT contenu, type_mime::text FROM contenu.fichiers
          WHERE disque = 'public' AND chemin = $1",
    )
    .bind(path)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some((bytes, mime))) => {
            let mut response = Response::new(Body::from(bytes));
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&mime)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'none'; sandbox"),
            );
            response
        }
        Ok(None) => ApiError::NotFound {
            detail: "No public file has this path.".to_owned(),
            instance: Some(instance.to_owned()),
        }
        .into_response(),
        Err(error) => ApiError::from(error).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_known_by_their_content() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        let found = image(&png).expect("a PNG");
        assert_eq!(
            (found.mime, found.width, found.height),
            ("image/png", 640, 480)
        );

        let gif = b"GIF89a\x10\x00\x20\x00".to_vec();
        assert_eq!(image(&gif).map(|i| (i.width, i.height)), Some((16, 32)));

        // SOI, an APP0 of 16 bytes, then SOF0 of 300 x 200.
        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10];
        jpeg.extend_from_slice(&[0; 14]);
        jpeg.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0xc8, 0x01, 0x2c, 0x03]);
        jpeg.extend_from_slice(&[0; 8]);
        let found = image(&jpeg).expect("a JPEG");
        assert_eq!(
            (found.width, found.height, found.extension),
            (300, 200, "jpg")
        );

        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0".to_vec();
        webp.extend_from_slice(&[0x7f, 0x02, 0x00, 0xdf, 0x01, 0x00]);
        assert_eq!(image(&webp).map(|i| (i.width, i.height)), Some((640, 480)));

        assert_eq!(image(b"%PDF-1.4"), None);
        assert_eq!(image(b"<svg></svg>"), None);
        assert!(is_pdf(b"%PDF-1.7 ..."));
        assert!(!is_pdf(b"<html>"));
    }
}
