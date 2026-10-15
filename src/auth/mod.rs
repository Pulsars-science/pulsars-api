//! Member authentication with the game tokens Laravel issues
//! (`docs/contract/auth-jwt.md`).
//!
//! Handlers take an [`AuthUser`] argument; the extractor verifies the
//! `Authorization: Bearer` token and rejects the request with a 401 problem
//! (or 503 while no key is loaded). Rejections are logged with their
//! `reason`, the `kid` and the client IP, never with the token.

pub mod jwks;
pub mod token;

use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::middleware as mw;
use crate::state::AppState;
use crate::telemetry::names;

pub use jwks::{Jwks, JwksSettings, Lookup};
pub use token::{Claims, Expected, Reason};

/// The member a request acts for: the claims of its verified game token.
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

/// Why [`authenticate`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The token is refused: 401 with this reason. `kid` is the header's,
    /// when the token got that far.
    Rejected { reason: Reason, kid: Option<String> },
    /// No usable key set (never loaded, or stale for 24 hours): 503.
    NoKeys,
}

/// Verifies an `Authorization` header value against the cached JWKS. Shared
/// by the HTTP extractor and, later, the WebSocket `auth` frame.
pub async fn authenticate(
    header: Option<&[u8]>,
    jwks: &Jwks,
    expected: Expected<'_>,
    now: i64,
) -> Result<Claims, Failure> {
    let rejected = |reason, kid: Option<&str>| Failure::Rejected {
        reason,
        kid: kid.map(str::to_owned),
    };
    let token = token::bearer(header).map_err(|r| rejected(r, None))?;
    let unverified = token::decode(token).map_err(|r| rejected(r, None))?;
    let kid = unverified.kid().to_owned();
    let key = match jwks.key(&kid).await {
        Lookup::Found(key) => key,
        Lookup::UnknownKid => return Err(rejected(Reason::UnknownKid, Some(&kid))),
        Lookup::Unavailable => return Err(Failure::NoKeys),
    };
    unverified
        .verify(&key, expected, now)
        .map_err(|r| rejected(r, Some(&kid)))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let header = parts.headers.get(AUTHORIZATION).map(|v| v.as_bytes());
        let expected = Expected {
            issuer: &state.config.jwt_issuer,
            audience: &state.config.jwt_audience,
        };
        let now = chrono::Utc::now().timestamp();
        match authenticate(header, &state.jwks, expected, now).await {
            Ok(claims) => Ok(Self(claims)),
            Err(Failure::Rejected { reason, kid }) => {
                metrics::counter!(names::AUTH_REJECTIONS_TOTAL, "reason" => reason.code())
                    .increment(1);
                tracing::info!(
                    reason = reason.code(),
                    kid = kid.as_deref().map(loggable),
                    client_ip = mw::client_ip(parts).map(|ip| ip.to_string()),
                    "token rejected"
                );
                let instance = parts
                    .extensions
                    .get::<OriginalUri>()
                    .map_or_else(|| parts.uri.path(), |uri| uri.path())
                    .to_owned();
                Err(ApiError::Unauthorized {
                    reason,
                    instance: Some(instance),
                })
            }
            Err(Failure::NoKeys) => {
                metrics::counter!(names::AUTH_REJECTIONS_TOTAL, "reason" => "no_keys").increment(1);
                Err(ApiError::Unavailable(
                    "no usable JWKS key: cannot verify tokens".to_owned(),
                ))
            }
        }
    }
}

/// The `kid` comes from an unverified token: keep it short and printable
/// before it reaches the logs.
fn loggable(kid: &str) -> String {
    kid.chars()
        .take(64)
        .map(|c| if c.is_ascii_graphic() { c } else { '?' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kids_are_sanitized_for_logs() {
        assert_eq!(loggable("2026-10-ed25519-1"), "2026-10-ed25519-1");
        assert_eq!(loggable("a\nb\u{1b}[31mc é"), "a?b?[31mc??");
        assert_eq!(loggable(&"k".repeat(500)).len(), 64);
    }
}
