//! Request body extractors.
//!
//! [`Json`] replaces axum's own: same use (`Json(body): Json<T>` as the
//! last handler argument), but every rejection is an RFC 9457 problem in
//! this API's vocabulary (`docs/contract/README.md`, "Errors"), and a field
//! that does not fit `T` is named by its JSON Pointer.

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::{StatusCode, header};
use serde::de::DeserializeOwned;

use crate::error::{ApiError, ParameterError};

/// A JSON request body, deserialized into `T`.
///
/// Refuses, before reading the body, a `Content-Type` other than
/// `application/json` (415); then a body over the size limit (413), one that
/// is not JSON (400 `invalid-body`, no `errors`), and one that does not fit
/// `T` (400 `invalid-body`, one entry in `errors`). Put
/// `#[serde(deny_unknown_fields)]` on `T` so a misspelt field is refused
/// instead of silently ignored.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let is_json = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_json_media_type);
        if !is_json {
            return Err(ApiError::UnsupportedMediaType);
        }
        // The body limit layer is what refuses an oversized body: reading it
        // fails with a 413 rejection.
        let bytes =
            Bytes::from_request(request, state)
                .await
                .map_err(|rejection| match rejection.status() {
                    StatusCode::PAYLOAD_TOO_LARGE => ApiError::PayloadTooLarge,
                    _ => ApiError::InvalidBody(Vec::new()),
                })?;
        parse(&bytes).map(Self)
    }
}

/// Deserializes a JSON body, naming the field at fault.
pub fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let inner = error.into_inner();
        if inner.is_data() {
            ApiError::InvalidBody(vec![ParameterError {
                parameter: pointer(&path),
                detail: detail(&inner),
                code: None,
            }])
        } else {
            // Syntax error or truncated body: no field to name.
            ApiError::InvalidBody(Vec::new())
        }
    })?;
    // Trailing characters after the value (`{} x`) are a syntax error too.
    deserializer
        .end()
        .map_err(|_| ApiError::InvalidBody(Vec::new()))?;
    Ok(value)
}

/// `application/json`, with or without parameters (`; charset=utf-8`).
fn is_json_media_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}

/// serde_path_to_error writes `.` for the root and `a.b[0].c` below it; the
/// contract names fields by JSON Pointer (RFC 6901): `""` and `/a/b/0/c`.
fn pointer(path: &str) -> String {
    if path == "." {
        return String::new();
    }
    let mut pointer = String::new();
    for segment in path.split('.') {
        let mut rest = segment;
        // `name[0][1]`: the name, then each index.
        let name_end = rest.find('[').unwrap_or(rest.len());
        let (name, indexes) = rest.split_at(name_end);
        if !name.is_empty() {
            pointer.push('/');
            pointer.push_str(&name.replace('~', "~0").replace('/', "~1"));
        }
        rest = indexes;
        while let Some(index) = rest.strip_prefix('[') {
            let end = index.find(']').unwrap_or(index.len());
            pointer.push('/');
            pointer.push_str(&index[..end]);
            rest = index.get(end + 1..).unwrap_or_default();
        }
    }
    pointer
}

/// serde's message without the ` at line 1 column 42` suffix, which says
/// nothing more than the pointer.
fn detail(error: &serde_json::Error) -> String {
    let message = error.to_string();
    match message.rfind(" at line ") {
        Some(end) => message[..end].to_owned(),
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Login {
        email: String,
        remember: Option<bool>,
        tags: Option<Vec<Tag>>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Tag {
        name: String,
    }

    fn errors(body: &str) -> Vec<ParameterError> {
        match parse::<Login>(body.as_bytes()) {
            Err(ApiError::InvalidBody(errors)) => errors,
            other => panic!("expected invalid-body, got {other:?}"),
        }
    }

    /// The extractor behind a body limit, as in the real router.
    async fn post(content_type: Option<&str>, body: &'static str) -> (StatusCode, String) {
        use axum::Router;
        use axum::body::Body;
        use axum::response::IntoResponse;
        use axum::routing::post;
        use tower::ServiceExt;
        use tower_http::limit::RequestBodyLimitLayer;

        let app = Router::new()
            .route(
                "/",
                post(|Json(login): Json<Login>| async move { login.email }),
            )
            .layer(RequestBodyLimitLayer::new(64));
        let mut request = Request::post("/");
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        let response = app
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn the_extractor_answers_with_problems() {
        let json = Some("application/json");
        assert_eq!(
            post(json, r#"{"email":"a@b.c"}"#).await,
            (StatusCode::OK, "a@b.c".to_owned())
        );

        let (status, body) = post(Some("text/plain"), r#"{"email":"a@b.c"}"#).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert!(body.contains("/problems/unsupported-media-type"));
        let (status, _) = post(None, r#"{"email":"a@b.c"}"#).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let (status, body) = post(json, r#"{"email":3}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("/problems/invalid-body"));
        assert!(body.contains(r#""parameter":"/email""#));

        let big =
            r#"{"email":"0123456789012345678901234567890123456789012345678901234567890123456789"}"#;
        let (status, body) = post(json, big).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(body.contains("/problems/payload-too-large"));
    }

    #[test]
    fn a_valid_body_parses() {
        let login: Login = parse(br#"{"email":"a@b.c","remember":true}"#).unwrap();
        assert_eq!(login.email, "a@b.c");
    }

    #[test]
    fn a_missing_field_is_named_by_the_root_pointer() {
        let errors = errors(r#"{"remember":true}"#);
        assert_eq!(errors[0].parameter, "");
        assert_eq!(errors[0].detail, "missing field `email`");
    }

    #[test]
    fn a_wrong_type_is_named_by_its_pointer() {
        let errors = errors(r#"{"email":"a@b.c","tags":[{"name":"x"},{"name":3}]}"#);
        assert_eq!(errors[0].parameter, "/tags/1/name");
        assert!(errors[0].detail.starts_with("invalid type: integer `3`"));
        assert!(!errors[0].detail.contains("line"));
    }

    #[test]
    fn an_unknown_field_is_refused() {
        let errors = errors(r#"{"email":"a@b.c","admin":true}"#);
        assert_eq!(errors[0].parameter, "/admin");
        assert!(errors[0].detail.starts_with("unknown field `admin`"));
    }

    #[test]
    fn broken_json_names_no_field() {
        assert!(errors(r#"{"email":"#).is_empty());
        assert!(errors("not json").is_empty());
        assert!(errors(r#"{"email":"a@b.c"} trailing"#).is_empty());
    }

    #[test]
    fn pointers_escape_and_index() {
        assert_eq!(pointer("."), "");
        assert_eq!(pointer("a"), "/a");
        assert_eq!(pointer("a.b[0].c"), "/a/b/0/c");
        assert_eq!(pointer("m[0][2]"), "/m/0/2");
        assert_eq!(pointer("a~b/c"), "/a~0b~1c");
    }

    #[test]
    fn only_application_json_is_json() {
        assert!(is_json_media_type("application/json"));
        assert!(is_json_media_type("Application/JSON; charset=utf-8"));
        assert!(!is_json_media_type("text/plain"));
        assert!(!is_json_media_type("application/x-www-form-urlencoded"));
        assert!(!is_json_media_type("application/jsonx"));
    }
}
