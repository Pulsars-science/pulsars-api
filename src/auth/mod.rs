//! Member authentication with the game tokens Laravel issues
//! (`docs/contract/auth-jwt.md`).
//!
//! Handlers take an [`AuthUser`] argument; the extractor verifies the
//! `Authorization: Bearer` token and rejects the request with a 401 problem
//! (or 503 while no key is loaded). Rejections are logged with their
//! `reason`, the `kid` and the client IP, never with the token.
//!
//! [`Admin`] and [`Moderator`] do the same, then refuse with a 403
//! `insufficient-scope` a token whose `roles` do not include one of theirs.

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

/// The outcome of verifying a request's `Authorization` header, left in the
/// request extensions by whoever verified it first (the rate limiter), so
/// the token is verified once per request.
#[derive(Debug, Clone)]
pub struct TokenCheck(pub Result<Claims, Failure>);

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
        let result = match parts.extensions.get::<TokenCheck>() {
            // Already verified by the rate limiter.
            Some(TokenCheck(result)) => result.clone(),
            None => {
                let header = parts.headers.get(AUTHORIZATION).map(|v| v.as_bytes());
                let expected = Expected {
                    issuer: &state.config.jwt_issuer,
                    audience: &state.config.jwt_audience,
                };
                let now = chrono::Utc::now().timestamp();
                authenticate(header, &state.jwks, expected, now).await
            }
        };
        match result {
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

/// Roles of the game token that may use the admin endpoints.
pub const ADMIN_ROLES: &[&str] = &["admin", "super-admin"];

/// Roles that may moderate: the admins, and the moderators.
pub const MODERATOR_ROLES: &[&str] = &["moderateur", "admin", "super-admin"];

/// A member whose token has the `admin` or `super-admin` role.
#[derive(Debug, Clone)]
pub struct Admin(pub Claims);

/// A member whose token has the `moderateur`, `admin` or `super-admin` role.
#[derive(Debug, Clone)]
pub struct Moderator(pub Claims);

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let claims = with_role(parts, state, ADMIN_ROLES, "the admin or super-admin role").await?;
        Ok(Self(claims))
    }
}

impl FromRequestParts<AppState> for Moderator {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let claims = with_role(
            parts,
            state,
            MODERATOR_ROLES,
            "the moderateur, admin or super-admin role",
        )
        .await?;
        Ok(Self(claims))
    }
}

/// The verified claims, if they carry one of `roles`; `needed` completes
/// the 403's detail ("This endpoint needs …").
async fn with_role(
    parts: &mut Parts,
    state: &AppState,
    roles: &[&str],
    needed: &str,
) -> Result<Claims, ApiError> {
    let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
    if has_any_role(&claims, roles) {
        return Ok(claims);
    }
    let instance = parts
        .extensions
        .get::<OriginalUri>()
        .map_or_else(|| parts.uri.path(), |uri| uri.path())
        .to_owned();
    Err(ApiError::InsufficientScope {
        detail: format!("This endpoint needs {needed}."),
        instance: Some(instance),
    })
}

/// Whether the token's `roles` include one of `roles`.
pub fn has_any_role(claims: &Claims, roles: &[&str]) -> bool {
    claims
        .roles
        .iter()
        .any(|role| roles.contains(&role.as_str()))
}

/// The account id of a token's `sub`: a positive decimal id without sign or
/// leading zero, exactly as Laravel writes `users.id`. Anything else cannot
/// name an account.
pub fn user_id(sub: &str) -> Option<i64> {
    let canonical =
        !sub.is_empty() && !sub.starts_with('0') && sub.bytes().all(|b| b.is_ascii_digit());
    if canonical { sub.parse().ok() } else { None }
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

    fn claims(roles: &[&str]) -> Claims {
        Claims {
            sub: "1".to_owned(),
            jti: "j".to_owned(),
            exp: 0,
            name: None,
            locale: None,
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        }
    }

    #[test]
    fn roles_are_matched_exactly() {
        assert!(has_any_role(&claims(&["super-admin"]), ADMIN_ROLES));
        assert!(has_any_role(&claims(&["admin"]), MODERATOR_ROLES));
        assert!(has_any_role(&claims(&["moderateur"]), MODERATOR_ROLES));
        assert!(!has_any_role(&claims(&["moderateur"]), ADMIN_ROLES));
        assert!(!has_any_role(&claims(&[]), MODERATOR_ROLES));
        assert!(!has_any_role(&claims(&["Admin", "admins"]), ADMIN_ROLES));
    }

    #[test]
    fn kids_are_sanitized_for_logs() {
        assert_eq!(loggable("2026-10-ed25519-1"), "2026-10-ed25519-1");
        assert_eq!(loggable("a\nb\u{1b}[31mc é"), "a?b?[31mc??");
        assert_eq!(loggable(&"k".repeat(500)).len(), 64);
    }
}
