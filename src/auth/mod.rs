//! Member authentication with the game tokens the API signs itself
//! (`POST /v1/auth/token`, `docs/contract/auth-jwt.md`).
//!
//! Handlers take an [`AuthUser`] argument; the extractor verifies the
//! `Authorization: Bearer` token against the API's own signing key and
//! rejects the request with a 401 problem (or 503 when no key is
//! configured, which only development allows). Rejections are logged with
//! their `reason`, the `kid` and the client IP, never with the token.
//!
//! [`Admin`] does the same, then refuses with a 403 `insufficient-scope` a
//! token whose `roles` include neither `admin` nor `super-admin`.
//!
//! The site's own routes identify members by their session instead
//! (`member.rs`).

pub mod member;
pub mod password;
pub mod token;

use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::config::{API_AUDIENCE, API_ISSUER, TokenSigner};
use crate::error::ApiError;
use crate::middleware as mw;
use crate::state::AppState;
use crate::telemetry::names;

pub use token::{Claims, Expected, Reason};

/// What a valid token must carry: the API's issuer and audience.
pub const EXPECTED: Expected<'static> = Expected {
    issuer: API_ISSUER,
    audience: API_AUDIENCE,
};

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
    /// No signing key is configured (development only): 503.
    NoKey,
}

/// Verifies an `Authorization` header value with the API's signing key
/// (`signer`), by `kid`, without any network. Shared by the HTTP extractor,
/// the rate limiter and the WebSocket `auth` frame. `now` is in Unix
/// seconds.
pub fn authenticate(
    header: Option<&[u8]>,
    signer: Option<&TokenSigner>,
    now: i64,
) -> Result<Claims, Failure> {
    let rejected = |reason, kid: Option<&str>| Failure::Rejected {
        reason,
        kid: kid.map(str::to_owned),
    };
    let token = token::bearer(header).map_err(|r| rejected(r, None))?;
    let unverified = token::decode(token).map_err(|r| rejected(r, None))?;
    let signer = signer.ok_or(Failure::NoKey)?;
    let kid = unverified.kid().to_owned();
    if kid != signer.kid {
        return Err(rejected(Reason::UnknownKid, Some(&kid)));
    }
    unverified
        .verify(&signer.key.verifying_key(), EXPECTED, now)
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
                let now = chrono::Utc::now().timestamp();
                authenticate(header, state.config.token_signer.as_ref(), now)
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
            Err(Failure::NoKey) => {
                metrics::counter!(names::AUTH_REJECTIONS_TOTAL, "reason" => "no_key").increment(1);
                Err(ApiError::Unavailable(
                    "no signing key configured: cannot verify tokens".to_owned(),
                ))
            }
        }
    }
}

/// Roles of the game token that may use the admin endpoints.
pub const ADMIN_ROLES: &[&str] = &["admin", "super-admin"];

/// A member whose token has the `admin` or `super-admin` role.
#[derive(Debug, Clone)]
pub struct Admin(pub Claims);

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if has_any_role(&claims, ADMIN_ROLES) {
            return Ok(Self(claims));
        }
        let instance = parts
            .extensions
            .get::<OriginalUri>()
            .map_or_else(|| parts.uri.path(), |uri| uri.path())
            .to_owned();
        Err(ApiError::InsufficientScope {
            detail: "This endpoint needs the admin or super-admin role.".to_owned(),
            instance: Some(instance),
        })
    }
}

/// Whether the token's `roles` include one of `roles`.
pub fn has_any_role(claims: &Claims, roles: &[&str]) -> bool {
    claims
        .roles
        .iter()
        .any(|role| roles.contains(&role.as_str()))
}

/// The account id of a token's `sub`: a positive decimal id without sign or
/// leading zero, as the API writes `site.users.id`. Anything else cannot
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
    use crate::auth::token::testing::{self, claims, sign, signing_key};
    use serde_json::json;

    fn with_roles(roles: &[&str]) -> Claims {
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
        assert!(has_any_role(&with_roles(&["super-admin"]), ADMIN_ROLES));
        assert!(has_any_role(&with_roles(&["admin"]), ADMIN_ROLES));
        assert!(!has_any_role(&with_roles(&["moderateur"]), ADMIN_ROLES));
        assert!(!has_any_role(&with_roles(&[]), ADMIN_ROLES));
        assert!(!has_any_role(
            &with_roles(&["Admin", "admins"]),
            ADMIN_ROLES
        ));
    }

    #[test]
    fn kids_are_sanitized_for_logs() {
        assert_eq!(loggable("2026-10-ed25519-1"), "2026-10-ed25519-1");
        assert_eq!(loggable("a\nb\u{1b}[31mc é"), "a?b?[31mc??");
        assert_eq!(loggable(&"k".repeat(500)).len(), 64);
    }

    fn token(key: &ed25519_dalek::SigningKey, kid: &str, iss: &str, now: i64) -> String {
        let mut claims = claims("2", now);
        claims["iss"] = json!(iss);
        sign(
            key,
            &json!({"alg": "EdDSA", "typ": "JWT", "kid": kid}),
            &claims,
        )
    }

    fn check(signer: Option<&TokenSigner>, token: &str, now: i64) -> Result<Claims, Failure> {
        let header = format!("Bearer {token}");
        authenticate(Some(header.as_bytes()), signer, now)
    }

    fn reason(result: Result<Claims, Failure>) -> Option<Reason> {
        match result {
            Err(Failure::Rejected { reason, .. }) => Some(reason),
            _ => None,
        }
    }

    #[test]
    fn the_api_verifies_its_own_tokens() {
        let signer = testing::signer();
        let now = chrono::Utc::now().timestamp();
        let claims = check(
            Some(&signer),
            &token(&signer.key, testing::KID, API_ISSUER, now),
            now,
        )
        .expect("own token");
        assert_eq!(claims.sub, "2");
    }

    #[test]
    fn a_token_needs_the_api_kid_key_and_issuer() {
        let signer = testing::signer();
        let now = chrono::Utc::now().timestamp();
        let other_kid = check(
            Some(&signer),
            &token(&signer.key, "2026-10-ed25519-2", API_ISSUER, now),
            now,
        );
        assert_eq!(reason(other_kid), Some(Reason::UnknownKid));
        let other_key = check(
            Some(&signer),
            &token(&signing_key(), testing::KID, API_ISSUER, now),
            now,
        );
        assert_eq!(reason(other_key), Some(Reason::BadSignature));
        let expired = check(
            Some(&signer),
            &token(&signer.key, testing::KID, API_ISSUER, now - 1000),
            now,
        );
        assert_eq!(reason(expired), Some(Reason::Expired));
        // The old site's issuer, or anyone else's, is refused even when
        // signed with the API's key.
        for iss in ["https://pulsars.science", "https://evil.example"] {
            let result = check(
                Some(&signer),
                &token(&signer.key, testing::KID, iss, now),
                now,
            );
            assert_eq!(reason(result), Some(Reason::BadIssuer), "{iss}");
        }
    }

    #[test]
    fn without_a_signing_key_tokens_cannot_be_verified() {
        let signer = testing::signer();
        let now = chrono::Utc::now().timestamp();
        let valid = token(&signer.key, testing::KID, API_ISSUER, now);
        assert_eq!(check(None, &valid, now), Err(Failure::NoKey));
        // A missing or malformed header is still a 401.
        assert_eq!(reason(authenticate(None, None, now)), Some(Reason::Missing));
        assert_eq!(reason(check(None, "abc", now)), Some(Reason::Malformed));
    }
}
