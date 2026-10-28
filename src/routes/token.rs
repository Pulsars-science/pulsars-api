//! `POST /v1/auth/token` and `GET /.well-known/jwks.json` (`openapi.yaml`,
//! tag `auth`; `docs/contract/auth-jwt.md`): the API issues the game token
//! itself, from the member's session, instead of Laravel's `/jeu/jeton`.
//!
//! The browser calls it from the game front end with `credentials:
//! "include"`: the session comes from the `pulsars_auth` cookie the site sets
//! on `.pulsars.science`, or, from a server, from `Pulsars-Session`. The
//! route is protected like Laravel's `VerifierOrigineJeu` (contract §2): an
//! allowed `Origin`, `Content-Type: application/json`, and a same-site
//! `Sec-Fetch-Site` when the browser sends one. The token is never logged.

use axum::extract::{OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::Signer as _;
use serde::Serialize;
use serde_json::json;

use super::auth::{self, is_token, live_session};
use crate::config::TokenSigner;
use crate::db::accounts;
use crate::error::ApiError;
use crate::rate_limit;
use crate::state::AppState;

/// The cookie the site sets on `.pulsars.science` (`auth-sessions.md`).
pub const SESSION_COOKIE: &str = "pulsars_auth";

/// Tokens per minute for one member, as Laravel's `throttle:30,1`.
const TOKENS_PER_MINUTE: u32 = 30;

#[derive(Debug, Serialize)]
pub struct IssuedToken {
    pub token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub expires_at: String,
}

/// `POST /v1/auth/token`.
pub async fn issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let signer = state
        .config
        .token_signer
        .as_ref()
        .ok_or(ApiError::TokenUnavailable)?;
    let token = cookie_session(&headers)
        .or_else(|| auth::session_token(&headers))
        .ok_or(ApiError::SessionInvalid)?;
    let (_, account) = live_session(&state, token).await?;

    // The same gates as Laravel's `['auth', 'onboarding', 'verified']`.
    let refused = |detail: &str| ApiError::Forbidden {
        detail: detail.to_owned(),
        instance: Some(uri.path().to_owned()),
    };
    if account.onboarding_at.is_none() {
        return Err(refused("Onboarding required."));
    }
    if account.email_verified_at.is_none() {
        return Err(refused("Your email address is not verified."));
    }

    let who = account.id.to_string();
    if let Some(decision) =
        rate_limit::attempt(&state.redis, "token", &who, TOKENS_PER_MINUTE, 60).await
        && !decision.allowed()
    {
        return Err(ApiError::TooManyAttempts {
            retry_after: decision.reset.max(1),
        });
    }

    let roles = accounts::permission_roles(&state.db, account.id).await?;
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iss": signer.issuer,
        "aud": state.config.jwt_audience,
        "sub": who,
        "iat": now,
        "nbf": now,
        "exp": now + signer.ttl_secs,
        "jti": uuid::Uuid::now_v7().to_string(),
        "name": account.pseudo,
        "locale": account.locale,
        "roles": roles,
    });
    let expires_at = chrono::DateTime::from_timestamp(now + signer.ttl_secs, 0)
        .map(super::courses::timestamp)
        .unwrap_or_default();
    Ok(auth::no_store(
        StatusCode::OK,
        &IssuedToken {
            token: sign(signer, &claims),
            token_type: "Bearer",
            expires_in: signer.ttl_secs,
            expires_at,
        },
    ))
}

/// A compact JWS of `claims` (`alg` EdDSA, `kid` of the signer).
pub fn sign(signer: &TokenSigner, claims: &serde_json::Value) -> String {
    let header = json!({"alg": "EdDSA", "typ": "JWT", "kid": signer.kid});
    let input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    let signature = signer.key.sign(input.as_bytes());
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}

/// `pulsars_auth` from the `Cookie` headers, if it has a token's shape.
fn cookie_session(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.trim().trim_matches('"'))
        .filter(|value| is_token(value))
}

/// Middleware of `/v1/auth/token`: answers the preflight, refuses what a
/// browser on another site could send, and adds the credentialed CORS
/// headers for the allowed origin.
pub async fn browser_guard(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let origin = headers.get(header::ORIGIN).cloned();
    let allowed = origin
        .as_ref()
        .is_some_and(|o| state.config.cors_allowed_origins.contains(o));
    let path = request.uri().path().to_owned();
    let forbidden = |detail: &str| {
        ApiError::Forbidden {
            detail: detail.to_owned(),
            instance: Some(path.clone()),
        }
        .into_response()
    };

    if request.method() == Method::OPTIONS {
        if !allowed {
            return forbidden("Origin not allowed.");
        }
        let mut response = StatusCode::NO_CONTENT.into_response();
        let h = response.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("POST"),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type, Accept"),
        );
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        return with_cors(response, origin);
    }

    // A server calling with `Pulsars-Session` sends no `Origin`; a browser
    // always does on a cross-origin POST.
    if origin.is_none()
        && auth::session_token(headers).is_some()
        && cookie_session(headers).is_none()
    {
        return next.run(request).await;
    }
    if !allowed {
        return forbidden("Origin not allowed.");
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return with_cors(ApiError::UnsupportedMediaType.into_response(), origin);
    }
    let fetch_site_ok = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v == "same-site" || v == "same-origin");
    if !fetch_site_ok {
        return forbidden("Cross-site request refused.");
    }
    with_cors(next.run(request).await, origin)
}

fn with_cors(mut response: Response, origin: Option<HeaderValue>) -> Response {
    let h = response.headers_mut();
    if let Some(origin) = origin {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        h.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }
    h.append(header::VARY, HeaderValue::from_static("Origin"));
    response
}

/// `GET /.well-known/jwks.json`: the key the API signs game tokens with.
/// Empty when it signs none.
pub async fn jwks(State(state): State<AppState>) -> Response {
    let keys: Vec<serde_json::Value> = state
        .config
        .token_signer
        .iter()
        .map(|signer| {
            json!({
                "kty": "OKP",
                "crv": "Ed25519",
                "x": URL_SAFE_NO_PAD.encode(signer.key.verifying_key().as_bytes()),
                "kid": signer.kid,
                "use": "sig",
                "alg": "EdDSA",
            })
        })
        .collect();
    let mut response = axum::Json(json!({"keys": keys})).into_response();
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/jwk-set+json"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookies(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, value.parse().unwrap());
        headers
    }

    #[test]
    fn the_session_cookie_is_found_among_others() {
        let token = "a".repeat(43);
        assert_eq!(
            cookie_session(&cookies(&format!("x=1; pulsars_auth={token}; y=2"))),
            Some(token.as_str())
        );
        assert_eq!(
            cookie_session(&cookies(&format!("pulsars_auth=\"{token}\""))),
            Some(token.as_str())
        );
        assert_eq!(cookie_session(&cookies("pulsars_auth=short")), None);
        assert_eq!(
            cookie_session(&cookies(&format!("pulsars-session={token}"))),
            None
        );
    }
}
