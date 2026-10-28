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

use crate::config::TokenSigner;
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

/// Verifies an `Authorization` header value. Shared by the HTTP extractor,
/// the rate limiter and the WebSocket `auth` frame.
///
/// Two issuers are accepted during the migration (`auth-jwt.md` §1): a token
/// that claims the API's own issuer is verified with the API's own key, by
/// `kid`, without any network; any other token with the site's JWKS and the
/// site's issuer (`expected`), as before.
pub async fn authenticate(
    header: Option<&[u8]>,
    jwks: &Jwks,
    expected: Expected<'_>,
    own: Option<&TokenSigner>,
    now: i64,
) -> Result<Claims, Failure> {
    let rejected = |reason, kid: Option<&str>| Failure::Rejected {
        reason,
        kid: kid.map(str::to_owned),
    };
    let token = token::bearer(header).map_err(|r| rejected(r, None))?;
    let unverified = token::decode(token).map_err(|r| rejected(r, None))?;
    let kid = unverified.kid().to_owned();
    if let Some(own) = own.filter(|own| unverified.claimed_issuer() == Some(own.issuer.as_str())) {
        if kid != own.kid {
            return Err(rejected(Reason::UnknownKid, Some(&kid)));
        }
        let own_expected = Expected {
            issuer: &own.issuer,
            audience: expected.audience,
        };
        return unverified
            .verify(&own.key.verifying_key(), own_expected, now)
            .map_err(|r| rejected(r, Some(&kid)));
    }
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
                let own = state.config.token_signer.as_ref();
                authenticate(header, &state.jwks, expected, own, now).await
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

    mod two_issuers {
        use super::super::*;
        use crate::auth::token::testing::{claims, sign, signing_key};
        use serde_json::json;

        const OWN: &str = "https://api.pulsars.science";
        const SITE: &str = "https://pulsars.science";
        const KID: &str = "2026-10-ed25519-9";

        fn signer() -> TokenSigner {
            TokenSigner {
                kid: KID.to_owned(),
                key: signing_key(),
                issuer: OWN.to_owned(),
                ttl_secs: 300,
            }
        }

        fn dead_jwks() -> Jwks {
            Jwks::with_settings(
                "http://127.0.0.1:1/jwks.json",
                false,
                JwksSettings {
                    fetch_timeout: std::time::Duration::from_millis(200),
                    ..JwksSettings::default()
                },
            )
            .unwrap()
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

        async fn check(own: &TokenSigner, token: &str, now: i64) -> Result<Claims, Failure> {
            let header = format!("Bearer {token}");
            let expected = Expected {
                issuer: SITE,
                audience: "api.pulsars.science",
            };
            authenticate(
                Some(header.as_bytes()),
                &dead_jwks(),
                expected,
                Some(own),
                now,
            )
            .await
        }

        #[tokio::test]
        async fn the_api_verifies_its_own_tokens_without_the_site() {
            let own = signer();
            let now = chrono::Utc::now().timestamp();
            let claims = check(&own, &token(&own.key, KID, OWN, now), now)
                .await
                .expect("own token");
            assert_eq!(claims.sub, "2");
        }

        #[tokio::test]
        async fn an_own_issuer_token_needs_the_own_kid_and_key() {
            let own = signer();
            let now = chrono::Utc::now().timestamp();
            let other_kid = check(&own, &token(&own.key, "2026-10-ed25519-1", OWN, now), now).await;
            assert!(matches!(
                other_kid,
                Err(Failure::Rejected {
                    reason: Reason::UnknownKid,
                    ..
                })
            ));
            let other_key = check(&own, &token(&signing_key(), KID, OWN, now), now).await;
            assert!(matches!(
                other_key,
                Err(Failure::Rejected {
                    reason: Reason::BadSignature,
                    ..
                })
            ));
            let expired = check(&own, &token(&own.key, KID, OWN, now - 1000), now).await;
            assert!(matches!(
                expired,
                Err(Failure::Rejected {
                    reason: Reason::Expired,
                    ..
                })
            ));
        }

        #[tokio::test]
        async fn the_api_key_does_not_sign_for_the_site_or_anyone_else() {
            let own = signer();
            let now = chrono::Utc::now().timestamp();
            // Claiming the site: checked against the site's keys only, which
            // never hold the API's key.
            for iss in [SITE, "https://evil.example"] {
                let result = check(&own, &token(&own.key, KID, iss, now), now).await;
                assert!(result.is_err(), "{iss}");
            }
        }
    }
}
