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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParameterError {
    pub parameter: String,
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid parameter")]
    InvalidParameter(Vec<ParameterError>),
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
    /// Over the per-minute limit; `retry_after` is in seconds.
    #[error("rate limited")]
    RateLimited { limit: u32, retry_after: u64 },
    #[error("request timed out")]
    Timeout,
    #[error("service unavailable: {0}")]
    Unavailable(String),
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
    /// For 401 only: why the token was refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidParameter(_) | Self::InvalidCursor => StatusCode::BAD_REQUEST,
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::UpgradeRequired => StatusCode::UPGRADE_REQUIRED,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Timeout => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn to_problem(&self) -> Problem {
        let (slug, title) = match self {
            Self::InvalidParameter(_) => ("invalid-parameter", "Invalid parameter"),
            Self::InvalidCursor => ("invalid-cursor", "Invalid cursor"),
            Self::Unauthorized { .. } => ("unauthorized", "Invalid token"),
            Self::Forbidden { .. } => ("forbidden", "Forbidden"),
            Self::UpgradeRequired => ("upgrade-required", "Upgrade required"),
            Self::NotFound { .. } => ("not-found", "Resource not found"),
            Self::MethodNotAllowed => ("method-not-allowed", "Method not allowed"),
            Self::NotAcceptable => ("not-acceptable", "Not acceptable"),
            Self::PayloadTooLarge => ("payload-too-large", "Payload too large"),
            Self::RateLimited { .. } => ("rate-limited", "Too many requests"),
            Self::Timeout => ("unavailable", "Request timed out"),
            Self::Unavailable(_) => ("unavailable", "Service unavailable"),
            Self::Internal(_) => ("internal", "Internal error"),
        };
        let detail = match self {
            Self::InvalidParameter(_) => {
                Some("One or more query parameters are invalid.".to_owned())
            }
            Self::InvalidCursor => {
                Some("The cursor cannot be decoded, or was issued for other filters.".to_owned())
            }
            Self::NotAcceptable => Some("This API only produces application/json.".to_owned()),
            Self::RateLimited { limit, .. } => {
                Some(format!("Limit of {limit} requests per minute reached."))
            }
            Self::Unauthorized { reason, .. } => Some(reason.detail().to_owned()),
            Self::NotFound { detail, .. } | Self::Forbidden { detail, .. } => Some(detail.clone()),
            Self::UpgradeRequired => {
                Some("This endpoint only accepts WebSocket upgrades.".to_owned())
            }
            Self::Timeout => Some("The request took too long to process.".to_owned()),
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
                | Self::Forbidden { instance, .. } => instance.clone(),
                _ => None,
            },
            errors: match self {
                Self::InvalidParameter(errors) => Some(errors.clone()),
                _ => None,
            },
            reason: match self {
                Self::Unauthorized { reason, .. } => Some(reason.code()),
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
        if let Self::RateLimited { retry_after, .. } = &self {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(*retry_after));
        }
        if let Self::UpgradeRequired = &self {
            headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
            headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
        }
        if status == StatusCode::METHOD_NOT_ALLOWED {
            // Phase 1 only serves GET routes; refined per route later.
            headers.insert(
                header::ALLOW,
                HeaderValue::from_static("GET, HEAD, OPTIONS"),
            );
        }
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
    fn invalid_cursor_has_its_own_type() {
        let json = serde_json::to_value(ApiError::InvalidCursor.to_problem()).unwrap_or_default();
        assert_eq!(
            json["type"],
            "https://api.pulsars.science/problems/invalid-cursor"
        );
        assert_eq!(json["status"], 400);
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
