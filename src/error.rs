//! Errors returned to clients as RFC 9457 problem details
//! (`application/problem+json`), as described in `docs/contract/README.md`.
//!
//! The response never carries internal details (SQL, file paths, upstream
//! messages): those are logged with the request's trace id, and a 500 body
//! carries that trace id so a report can be matched to the logs.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::auth::Reason;
use crate::telemetry;

/// `realm` of the `WWW-Authenticate` challenge.
pub const AUTH_REALM: &str = "api.pulsars.science";

/// Base of every problem `type` URI.
pub const PROBLEM_BASE: &str = "https://api.pulsars.science/problems/";

pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// `reason` of a 403 for a token whose roles do not allow the action.
pub const INSUFFICIENT_SCOPE: &str = "insufficient_scope";

const INSUFFICIENT_SCOPE_CHALLENGE: &str =
    "Bearer realm=\"api.pulsars.science\", error=\"insufficient_scope\"";

/// One entry of `errors`. For a query parameter, `parameter` is its name;
/// for a JSON body, the JSON Pointer (RFC 6901) of the field at fault, `""`
/// for the body itself (a missing field is reported there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParameterError {
    pub parameter: String,
    pub detail: String,
    /// A stable code for clients that translate the message (`taken`,
    /// `too_short`…; `auth-sessions.md`). Absent where there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
}

impl ParameterError {
    /// A body field at fault, with its code.
    pub fn field(parameter: &str, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            parameter: parameter.to_owned(),
            detail: detail.into(),
            code: Some(code),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid parameter")]
    InvalidParameter(Vec<ParameterError>),
    /// A request body that is not JSON (`errors` empty) or does not fit what
    /// the endpoint expects (one entry per field at fault).
    #[error("invalid body")]
    InvalidBody(Vec<ParameterError>),
    /// A well-formed body whose values break the endpoint's rules (a pseudo
    /// already taken, a password too short): 422, one entry per field.
    #[error("invalid fields")]
    InvalidFields(Vec<ParameterError>),
    /// Unknown login, wrong password, or an account without a password.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// A `Pulsars-Session` that is missing, unknown, expired or revoked.
    #[error("session invalid")]
    SessionInvalid,
    /// A `cursor` that cannot be decoded, or was issued for another endpoint
    /// or another set of filters.
    #[error("invalid cursor")]
    InvalidCursor,
    /// Missing or invalid game token (`docs/contract/auth-jwt.md` §5).
    #[error("unauthorized: {}", reason.code())]
    Unauthorized {
        reason: Reason,
        instance: Option<String>,
    },
    /// The request is understood and refused (a WebSocket from an origin
    /// that is not allowed).
    #[error("forbidden: {detail}")]
    Forbidden {
        detail: String,
        instance: Option<String>,
    },
    /// A valid token whose roles do not allow the action
    /// (`docs/contract/auth-jwt.md` §5, RFC 6750 `insufficient_scope`).
    #[error("insufficient scope: {detail}")]
    InsufficientScope {
        detail: String,
        instance: Option<String>,
    },
    /// A plain request on the WebSocket endpoint.
    #[error("upgrade required")]
    UpgradeRequired,
    #[error("not found: {detail}")]
    NotFound {
        detail: String,
        instance: Option<String>,
    },
    #[error("method not allowed")]
    MethodNotAllowed,
    /// `Accept` excludes `application/json`.
    #[error("not acceptable")]
    NotAcceptable,
    #[error("payload too large")]
    PayloadTooLarge,
    /// A `Range` that starts past the end of a file of `size` bytes.
    #[error("range not satisfiable")]
    RangeNotSatisfiable { size: u64 },
    /// A body whose `Content-Type` is not `application/json`.
    #[error("unsupported media type")]
    UnsupportedMediaType,
    /// Over the per-minute limit; `retry_after` is in seconds.
    #[error("rate limited")]
    RateLimited { limit: u32, retry_after: u64 },
    /// Over a per-action limit (login attempts, sign-ups).
    #[error("too many attempts")]
    TooManyAttempts { retry_after: u64 },
    #[error("request timed out")]
    Timeout,
    #[error("service unavailable: {0}")]
    Unavailable(String),
    /// `POST /v1/auth/token` without a signing key configured.
    #[error("token unavailable")]
    TokenUnavailable,
    /// The message is logged, never sent.
    #[error("internal error: {0}")]
    Internal(String),
}

/// The JSON body of a problem response.
#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: &'static str,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ParameterError>>,
    /// For 401: why the token was refused; for a 403 `insufficient_scope`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidParameter(_) | Self::InvalidBody(_) | Self::InvalidCursor => {
                StatusCode::BAD_REQUEST
            }
            Self::InvalidFields(_) | Self::InvalidCredentials => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthorized { .. } | Self::SessionInvalid => StatusCode::UNAUTHORIZED,
            Self::Forbidden { .. } | Self::InsufficientScope { .. } => StatusCode::FORBIDDEN,
            Self::UpgradeRequired => StatusCode::UPGRADE_REQUIRED,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RangeNotSatisfiable { .. } => StatusCode::RANGE_NOT_SATISFIABLE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::RateLimited { .. } | Self::TooManyAttempts { .. } => {
                StatusCode::TOO_MANY_REQUESTS
            }
            Self::Timeout => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unavailable(_) | Self::TokenUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn to_problem(&self) -> Problem {
        let (slug, title) = match self {
            Self::InvalidParameter(_) => ("invalid-parameter", "Invalid parameter"),
            Self::InvalidBody(_) | Self::InvalidFields(_) => ("invalid-body", "Invalid body"),
            Self::InvalidCredentials => ("invalid-credentials", "Invalid credentials"),
            Self::SessionInvalid => ("session-invalid", "Invalid session"),
            Self::InvalidCursor => ("invalid-cursor", "Invalid cursor"),
            Self::Unauthorized { .. } => ("unauthorized", "Invalid token"),
            Self::Forbidden { .. } => ("forbidden", "Forbidden"),
            Self::InsufficientScope { .. } => ("insufficient-scope", "Insufficient scope"),
            Self::UpgradeRequired => ("upgrade-required", "Upgrade required"),
            Self::NotFound { .. } => ("not-found", "Resource not found"),
            Self::MethodNotAllowed => ("method-not-allowed", "Method not allowed"),
            Self::NotAcceptable => ("not-acceptable", "Not acceptable"),
            Self::PayloadTooLarge => ("payload-too-large", "Payload too large"),
            Self::RangeNotSatisfiable { .. } => ("range-not-satisfiable", "Range not satisfiable"),
            Self::UnsupportedMediaType => ("unsupported-media-type", "Unsupported media type"),
            Self::RateLimited { .. } | Self::TooManyAttempts { .. } => {
                ("rate-limited", "Too many requests")
            }
            Self::Timeout => ("unavailable", "Request timed out"),
            Self::Unavailable(_) => ("unavailable", "Service unavailable"),
            Self::TokenUnavailable => ("token-unavailable", "Game tokens unavailable"),
            Self::Internal(_) => ("internal", "Internal error"),
        };
        let detail = match self {
            Self::InvalidParameter(_) => {
                Some("One or more query parameters are invalid.".to_owned())
            }
            Self::InvalidBody(errors) if errors.is_empty() => {
                Some("The request body is not valid JSON.".to_owned())
            }
            Self::InvalidBody(_) => {
                Some("The request body does not match what this endpoint expects.".to_owned())
            }
            Self::InvalidFields(_) => Some("One or more fields are invalid.".to_owned()),
            Self::InvalidCredentials => {
                Some("These credentials do not match any account.".to_owned())
            }
            Self::SessionInvalid => {
                Some("The session is missing, unknown, expired or revoked.".to_owned())
            }
            Self::TokenUnavailable => Some("Game tokens are temporarily unavailable.".to_owned()),
            Self::TooManyAttempts { retry_after } => Some(format!(
                "Too many attempts. Try again in {retry_after} seconds."
            )),
            Self::UnsupportedMediaType => {
                Some("This endpoint only accepts application/json bodies.".to_owned())
            }
            Self::InvalidCursor => {
                Some("The cursor cannot be decoded, or was issued for other filters.".to_owned())
            }
            Self::NotAcceptable => Some("This API only produces application/json.".to_owned()),
            Self::RateLimited { limit, .. } => {
                Some(format!("Limit of {limit} requests per minute reached."))
            }
            Self::Unauthorized { reason, .. } => Some(reason.detail().to_owned()),
            Self::NotFound { detail, .. }
            | Self::Forbidden { detail, .. }
            | Self::InsufficientScope { detail, .. } => Some(detail.clone()),
            Self::UpgradeRequired => {
                Some("This endpoint only accepts WebSocket upgrades.".to_owned())
            }
            Self::Timeout => Some("The request took too long to process.".to_owned()),
            Self::RangeNotSatisfiable { size } => Some(format!(
                "The file has {size} bytes; the range starts past its end."
            )),
            _ => None,
        };
        Problem {
            kind: format!("{PROBLEM_BASE}{slug}"),
            title,
            status: self.status().as_u16(),
            detail,
            instance: match self {
                Self::NotFound { instance, .. }
                | Self::Unauthorized { instance, .. }
                | Self::Forbidden { instance, .. }
                | Self::InsufficientScope { instance, .. } => instance.clone(),
                _ => None,
            },
            errors: match self {
                Self::InvalidParameter(errors) => Some(errors.clone()),
                Self::InvalidBody(errors) if !errors.is_empty() => Some(errors.clone()),
                Self::InvalidFields(errors) => Some(errors.clone()),
                _ => None,
            },
            reason: match self {
                Self::Unauthorized { reason, .. } => Some(reason.code()),
                Self::InsufficientScope { .. } => Some(INSUFFICIENT_SCOPE),
                _ => None,
            },
            trace_id: match self {
                Self::Internal(_) | Self::Unavailable(_) | Self::Timeout => {
                    telemetry::current_trace_id()
                }
                _ => None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match &self {
            Self::Internal(message) => tracing::error!(error = %message, "internal error"),
            Self::Unavailable(message) => {
                tracing::warn!(error = %message, "dependency unavailable")
            }
            _ => {}
        }
        let status = self.status();
        let mut response = (status, Json(self.to_problem())).into_response();
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if let Self::Unauthorized { reason, .. } = &self {
            headers.insert(header::WWW_AUTHENTICATE, www_authenticate(*reason));
        }
        if let Self::InsufficientScope { .. } = &self {
            headers.insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static(INSUFFICIENT_SCOPE_CHALLENGE),
            );
        }
        if let Self::RateLimited { retry_after, .. } | Self::TooManyAttempts { retry_after } = &self
        {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(*retry_after));
        }
        if let Self::SessionInvalid = &self {
            headers.insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Session realm=\"api.pulsars.science\""),
            );
        }
        if let Self::RangeNotSatisfiable { size } = &self
            && let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}"))
        {
            headers.insert(header::CONTENT_RANGE, value);
        }
        if let Self::UpgradeRequired = &self {
            headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
            headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
        }
        // No `Allow` on a 405: the router adds the methods of the matched
        // route itself, but only when the response has none yet.
        response
    }
}

impl From<sqlx::Error> for ApiError {
    /// A database that cannot be reached is a 503; anything else is a bug or
    /// a contract mismatch (a view changed shape): 500.
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::PoolTimedOut | sqlx::Error::Io(_) => {
                Self::Unavailable(format!("database: {error}"))
            }
            _ => Self::Internal(format!("database: {error}")),
        }
    }
}

/// The RFC 6750 challenge: just the realm when no token was sent, the
/// `invalid_token` error otherwise.
fn www_authenticate(reason: Reason) -> HeaderValue {
    let value = match reason {
        Reason::Missing => format!("Bearer realm=\"{AUTH_REALM}\""),
        _ => format!(
            "Bearer realm=\"{AUTH_REALM}\", error=\"invalid_token\", error_description=\"{}\"",
            reason.description()
        ),
    };
    // Built from constants only; the fallback is never used.
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("Bearer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_serializes_as_rfc9457() {
        let problem = ApiError::NotFound {
            detail: "No published course has the slug 'x'.".to_owned(),
            instance: Some("/v1/courses/x".to_owned()),
        }
        .to_problem();
        let json = serde_json::to_value(&problem).unwrap_or_default();
        assert_eq!(
            json["type"],
            "https://api.pulsars.science/problems/not-found"
        );
        assert_eq!(json["status"], 404);
        assert_eq!(json["instance"], "/v1/courses/x");
        assert!(json.get("errors").is_none());
        assert!(json.get("trace_id").is_none());
    }

    #[test]
    fn internal_errors_hide_their_message() {
        let problem =
            ApiError::Internal("password authentication failed for user x".to_owned()).to_problem();
        let json = serde_json::to_string(&problem).unwrap_or_default();
        assert!(!json.contains("password"));
        assert!(json.contains("\"status\":500"));
    }

    #[test]
    fn invalid_parameter_lists_errors() {
        let problem = ApiError::InvalidParameter(vec![ParameterError {
            parameter: "limit".to_owned(),
            detail: "must be between 1 and 100".to_owned(),
            code: None,
        }])
        .to_problem();
        let json = serde_json::to_value(&problem).unwrap_or_default();
        assert_eq!(json["errors"][0]["parameter"], "limit");
    }

    #[test]
    fn unauthorized_carries_the_reason_and_a_challenge() {
        let response = ApiError::Unauthorized {
            reason: Reason::Expired,
            instance: Some("/v1/me".to_owned()),
        }
        .into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            "Bearer realm=\"api.pulsars.science\", error=\"invalid_token\", \
             error_description=\"token expired\""
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let problem = ApiError::Unauthorized {
            reason: Reason::Expired,
            instance: Some("/v1/me".to_owned()),
        }
        .to_problem();
        let json = serde_json::to_value(&problem).unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "https://api.pulsars.science/problems/unauthorized",
                "title": "Invalid token",
                "status": 401,
                "detail": "The token has expired.",
                "instance": "/v1/me",
                "reason": "expired",
            })
        );
    }

    #[test]
    fn rate_limited_sets_retry_after() {
        let response = ApiError::RateLimited {
            limit: 60,
            retry_after: 12,
        }
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "12");
        let json = serde_json::to_value(
            ApiError::RateLimited {
                limit: 60,
                retry_after: 12,
            }
            .to_problem(),
        )
        .unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "https://api.pulsars.science/problems/rate-limited",
                "title": "Too many requests",
                "status": 429,
                "detail": "Limit of 60 requests per minute reached.",
            })
        );
    }

    #[test]
    fn invalid_body_names_fields_only_when_it_can() {
        let json = serde_json::to_value(ApiError::InvalidBody(Vec::new()).to_problem())
            .unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "https://api.pulsars.science/problems/invalid-body",
                "title": "Invalid body",
                "status": 400,
                "detail": "The request body is not valid JSON.",
            })
        );
        let json = serde_json::to_value(
            ApiError::InvalidBody(vec![ParameterError {
                parameter: "/email".to_owned(),
                detail: "invalid type: integer `3`, expected a string".to_owned(),
                code: None,
            }])
            .to_problem(),
        )
        .unwrap_or_default();
        assert_eq!(json["errors"][0]["parameter"], "/email");
    }

    #[test]
    fn unsupported_media_type_is_a_415() {
        let response = ApiError::UnsupportedMediaType.into_response();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let json =
            serde_json::to_value(ApiError::UnsupportedMediaType.to_problem()).unwrap_or_default();
        assert_eq!(
            json["type"],
            "https://api.pulsars.science/problems/unsupported-media-type"
        );
    }

    #[test]
    fn invalid_cursor_has_its_own_type() {
        let json = serde_json::to_value(ApiError::InvalidCursor.to_problem()).unwrap_or_default();
        assert_eq!(
            json["type"],
            "https://api.pulsars.science/problems/invalid-cursor"
        );
        assert_eq!(json["status"], 400);
    }

    #[test]
    fn insufficient_scope_is_a_403_with_its_challenge() {
        let error = || ApiError::InsufficientScope {
            detail: "Admins only.".to_owned(),
            instance: Some("/v1/game/admin/stats".to_owned()),
        };
        let response = error().into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            "Bearer realm=\"api.pulsars.science\", error=\"insufficient_scope\""
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let json = serde_json::to_value(error().to_problem()).unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "https://api.pulsars.science/problems/insufficient-scope",
                "title": "Insufficient scope",
                "status": 403,
                "detail": "Admins only.",
                "instance": "/v1/game/admin/stats",
                "reason": "insufficient_scope",
            })
        );
    }

    #[test]
    fn a_missing_token_gets_a_bare_challenge() {
        let response = ApiError::Unauthorized {
            reason: Reason::Missing,
            instance: None,
        }
        .into_response();
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            "Bearer realm=\"api.pulsars.science\""
        );
    }
}
