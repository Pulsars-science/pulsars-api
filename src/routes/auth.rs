//! `/v1/auth/*` (`openapi.yaml`, tag `auth`; `docs/contract/auth-sessions.md`):
//! logging in, signing up, sessions and passwords of the site's members.
//!
//! The site (pulsars-web) calls these from its server, with the service key
//! and the visitor's address in `X-Forwarded-For`. A session is an opaque
//! token, given back in `Pulsars-Session`; only its SHA-256 is stored.
//! Passwords are argon2id (`crate::auth::password`).

use axum::extract::{FromRequestParts, OriginalUri, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::courses::timestamp;
use crate::auth::password;
use crate::db::accounts::{self, AccountRow, CreateError};
use crate::db::sessions::{self, SessionRow};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::journal::{self, Security};
use crate::middleware as mw;
use crate::rate_limit;
use crate::state::AppState;

pub const SESSION_HEADER: HeaderName = HeaderName::from_static("pulsars-session");
pub const VISITOR_HEADER: HeaderName = HeaderName::from_static("pulsars-visitor");

/// Login attempts per minute for one login from one address.
const LOGIN_LIMIT: u32 = 5;
/// Sign-ups per hour from one address.
const REGISTER_LIMIT: u32 = 10;
const REGISTER_WINDOW_SECS: i64 = 3600;

/// Length of a token: 32 random bytes in base64url, without padding.
const TOKEN_CHARS: usize = 43;

const LOCALES: &[&str] = &["fr", "en", "ru"];
const USER_AGENT_MAX_CHARS: usize = 500;
/// The longest identifier kept for a failed login on an unknown account.
const IDENTIFIER_MAX_CHARS: usize = 255;

// --- Shapes ----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AccountUser {
    pub id: String,
    pub pseudo: String,
    pub email: String,
    pub email_verified: bool,
    pub onboarded: bool,
    pub locale: Option<String>,
}

impl From<AccountRow> for AccountUser {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id.to_string(),
            pseudo: row.pseudo,
            email: row.email,
            email_verified: row.email_verified_at.is_some(),
            onboarded: row.onboarding_at.is_some(),
            locale: row.locale,
        }
    }
}

/// A session as its creator sees it: the only time the token is shown.
#[derive(Debug, Serialize)]
pub struct NewSession {
    pub token: String,
    pub expires_at: String,
    pub remember: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub expires_at: String,
    pub remember: bool,
}

#[derive(Debug, Serialize)]
struct Opened {
    session: NewSession,
    user: AccountUser,
}

#[derive(Debug, Serialize)]
struct Current {
    user: AccountUser,
    session: SessionInfo,
}

#[derive(Debug, Serialize)]
pub(crate) struct Data<T> {
    pub(crate) data: T,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginBody {
    login: String,
    password: String,
    #[serde(default)]
    remember: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RegisterBody {
    pseudo: String,
    email: String,
    password: String,
    password_confirmation: String,
    locale: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenBody {
    user_id: String,
    remember: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PasswordBody {
    current_password: Option<String>,
    password: String,
    password_confirmation: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResetBody {
    user_id: String,
    password: String,
    password_confirmation: String,
}

/// Who is calling: the visitor's address (last `X-Forwarded-For` entry,
/// set by the site or by nginx) and browser.
pub struct Caller {
    pub(crate) ip: Option<String>,
    pub(crate) user_agent: Option<String>,
    /// The anonymous visitor id the site relays (`Pulsars-Visitor`, set
    /// only after consent; `crate::journal`): a UUID v4, else none.
    pub(crate) visitor: Option<uuid::Uuid>,
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        let user_agent = parts
            .headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.chars().take(USER_AGENT_MAX_CHARS).collect());
        let visitor = crate::journal::visitor_id(
            parts
                .headers
                .get(VISITOR_HEADER)
                .and_then(|v| v.to_str().ok()),
        );
        Ok(Self {
            ip: mw::client_ip(parts).map(|ip| ip.to_string()),
            user_agent,
            visitor,
        })
    }
}

// --- Handlers --------------------------------------------------------------

/// `POST /v1/auth/login`.
pub async fn login(
    State(state): State<AppState>,
    caller: Caller,
    Json(body): Json<LoginBody>,
) -> Result<Response, ApiError> {
    let who = format!(
        "{}|{}",
        body.login.to_lowercase(),
        caller.ip.as_deref().unwrap_or("unknown")
    );
    limit(&state, "login", &who, LOGIN_LIMIT, rate_limit::WINDOW_SECS).await?;

    let account = accounts::find_by_login(&state.db, &body.login).await?;
    // Same work whether or not the account exists, or has a password.
    let hash = account
        .as_ref()
        .and_then(|a| a.password.clone())
        .unwrap_or_else(|| password::dummy(state.config.argon2_memory_kib).to_owned());
    let matches = password::verify(body.password, hash).await;
    let account = match account {
        Some(account) if matches && account.password.is_some() => account,
        Some(account) => {
            journal::security(
                &state.db,
                Some(account.id),
                Security::LoginFailed,
                &caller,
                json!({}),
            )
            .await;
            return Err(ApiError::InvalidCredentials);
        }
        None => {
            let identifier: String = body
                .login
                .trim()
                .chars()
                .take(IDENTIFIER_MAX_CHARS)
                .collect();
            journal::security(
                &state.db,
                None,
                Security::LoginFailed,
                &caller,
                json!({"identifier": identifier}),
            )
            .await;
            return Err(ApiError::InvalidCredentials);
        }
    };
    journal::security(
        &state.db,
        Some(account.id),
        Security::Login,
        &caller,
        json!({}),
    )
    .await;
    open(&state, account, body.remember, &caller, StatusCode::OK).await
}

/// `POST /v1/auth/register`.
pub async fn register(
    State(state): State<AppState>,
    caller: Caller,
    Json(body): Json<RegisterBody>,
) -> Result<Response, ApiError> {
    let ip = caller.ip.clone().unwrap_or_else(|| "unknown".to_owned());
    let mut errors = Vec::new();
    let pseudo_ok = check_pseudo(&body.pseudo, &mut errors);
    let email_ok = check_email(&body.email, &mut errors);
    check_password(&body.password, &body.password_confirmation, &mut errors);
    if let Some(locale) = &body.locale
        && !LOCALES.contains(&locale.as_str())
    {
        errors.push(ParameterError::field(
            "locale",
            "invalid",
            "must be fr, en, ru or null",
        ));
    }
    if pseudo_ok || email_ok {
        let (pseudo_taken, email_taken) =
            accounts::taken(&state.db, &body.pseudo, &body.email).await?;
        if pseudo_ok && pseudo_taken {
            errors.push(taken("pseudo"));
        }
        if email_ok && email_taken {
            errors.push(taken("email"));
        }
    }
    if !errors.is_empty() {
        errors.sort_by_key(|e| field_order(&e.parameter));
        return Err(ApiError::InvalidFields(errors));
    }
    // Counted once the request is valid: a form sent back with mistakes
    // does not use up the visitor's sign-ups.
    limit(
        &state,
        "register",
        &ip,
        REGISTER_LIMIT,
        REGISTER_WINDOW_SECS,
    )
    .await?;

    let hash = password::hash(state.config.argon2_memory_kib, body.password).await?;
    let account = accounts::create(
        &state.db,
        &body.pseudo,
        &body.email,
        &hash,
        body.locale.as_deref(),
    )
    .await
    .map_err(|error| match error {
        CreateError::PseudoTaken => ApiError::InvalidFields(vec![taken("pseudo")]),
        CreateError::EmailTaken => ApiError::InvalidFields(vec![taken("email")]),
        CreateError::Database(error) => error.into(),
    })?;
    journal::security(
        &state.db,
        Some(account.id),
        Security::Signup,
        &caller,
        json!({}),
    )
    .await;
    super::account::send_verification(&state, &account, body.locale.as_deref()).await?;
    open(&state, account, false, &caller, StatusCode::CREATED).await
}

/// `GET /v1/auth/session`.
pub async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (session, account) = current(&state, &headers).await?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Current {
                user: account.into(),
                session: SessionInfo {
                    expires_at: timestamp(session.expires_at),
                    remember: session.remember,
                },
            },
        },
    ))
}

/// `POST /v1/auth/logout`: always 204.
pub async fn logout(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(token) = session_token(&headers)
        && let Some(user_id) = sessions::revoke(&state.db, &digest(token)).await?
    {
        journal::security(
            &state.db,
            Some(user_id),
            Security::Logout,
            &caller,
            json!({}),
        )
        .await;
    }
    Ok(empty())
}

/// `POST /v1/auth/sessions`: the site opens a session for an account it
/// authenticated otherwise (Google, a reset link). Service key only.
pub async fn open_for(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<OpenBody>,
) -> Result<Response, ApiError> {
    require_service(&state, &headers, uri.path())?;
    let account = account_of(&state, &body.user_id, uri.path()).await?;
    journal::security(
        &state.db,
        Some(account.id),
        Security::Login,
        &caller,
        json!({"via": "site"}),
    )
    .await;
    open(&state, account, body.remember, &caller, StatusCode::CREATED).await
}

/// `PUT /v1/auth/password`: the member changes their password. Every other
/// session of the account is revoked.
pub async fn change_password(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
    Json(body): Json<PasswordBody>,
) -> Result<Response, ApiError> {
    let (session, account) = current(&state, &headers).await?;
    let mut errors = Vec::new();
    // An account created through Google has no password to confirm.
    if let Some(hash) = account.password.clone() {
        match body.current_password.filter(|p| !p.is_empty()) {
            None => errors.push(ParameterError::field(
                "current_password",
                "required",
                "is required",
            )),
            Some(current) => {
                if !password::verify(current, hash).await {
                    errors.push(ParameterError::field(
                        "current_password",
                        "incorrect",
                        "does not match the current password",
                    ));
                }
            }
        }
    }
    check_password(&body.password, &body.password_confirmation, &mut errors);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let hash = password::hash(state.config.argon2_memory_kib, body.password).await?;
    accounts::set_password(&state.db, account.id, &hash).await?;
    let revoked = sessions::revoke_others(&state.db, account.id, Some(session.id)).await?;
    journal::security(
        &state.db,
        Some(account.id),
        Security::PasswordChanged,
        &caller,
        json!({"sessions_revoked": revoked}),
    )
    .await;
    Ok(empty())
}

/// `POST /v1/auth/password/reset`: the site checked a reset link. Every
/// session of the account is revoked. Service key only.
pub async fn reset_password(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<ResetBody>,
) -> Result<Response, ApiError> {
    require_service(&state, &headers, uri.path())?;
    let mut errors = Vec::new();
    check_password(&body.password, &body.password_confirmation, &mut errors);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let account = account_of(&state, &body.user_id, uri.path()).await?;
    let hash = password::hash(state.config.argon2_memory_kib, body.password).await?;
    accounts::set_password(&state.db, account.id, &hash).await?;
    let revoked = sessions::revoke_others(&state.db, account.id, None).await?;
    journal::security(
        &state.db,
        Some(account.id),
        Security::PasswordReset,
        &caller,
        json!({"via": "site", "sessions_revoked": revoked}),
    )
    .await;
    Ok(empty())
}

// --- Sessions --------------------------------------------------------------

pub(crate) async fn open(
    state: &AppState,
    account: AccountRow,
    remember: bool,
    caller: &Caller,
    status: StatusCode,
) -> Result<Response, ApiError> {
    let session = new_session(state, account.id, remember, caller).await?;
    Ok(no_store(
        status,
        &Data {
            data: Opened {
                session,
                user: account.into(),
            },
        },
    ))
}

/// Opens a session for `user_id`; its token is in the result, once.
pub(crate) async fn new_session(
    state: &AppState,
    user_id: i64,
    remember: bool,
    caller: &Caller,
) -> Result<NewSession, ApiError> {
    let token = new_token()?;
    // The visitor's anonymous page views are the member's from now on.
    journal::link_visitor(&state.db, user_id, caller.visitor).await;
    let row = sessions::create(
        &state.db,
        user_id,
        &digest(&token),
        remember,
        caller.ip.as_deref(),
        caller.user_agent.as_deref(),
    )
    .await?;
    Ok(NewSession {
        token,
        expires_at: timestamp(row.expires_at),
        remember: row.remember,
    })
}

/// The live session of `Pulsars-Session` and its account.
pub(crate) async fn current(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(SessionRow, AccountRow), ApiError> {
    let token = session_token(headers).ok_or(ApiError::SessionInvalid)?;
    live_session(state, token).await
}

/// The live session of `token` and its account, extended by its lifetime.
pub(crate) async fn live_session(
    state: &AppState,
    token: &str,
) -> Result<(SessionRow, AccountRow), ApiError> {
    let session = sessions::use_token(&state.db, &digest(token))
        .await?
        .ok_or(ApiError::SessionInvalid)?;
    let account = accounts::find(&state.db, session.user_id)
        .await?
        .ok_or(ApiError::SessionInvalid)?;
    Ok((session, account))
}

/// The token of `Pulsars-Session`, if it has a token's shape.
pub(crate) fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|t| is_token(t))
}

/// Whether `t` has the shape of a session token.
pub(crate) fn is_token(t: &str) -> bool {
    t.len() == TOKEN_CHARS
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(crate) fn new_token() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness for a session token: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub(crate) async fn account_of(
    state: &AppState,
    user_id: &str,
    path: &str,
) -> Result<AccountRow, ApiError> {
    let Ok(id) = user_id.parse::<i64>() else {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "user_id",
            "invalid",
            "must be the id of an account, as a string",
        )]));
    };
    accounts::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound {
            detail: format!("No account has the id {id}."),
            instance: Some(path.to_owned()),
        })
}

pub(crate) fn require_service(
    state: &AppState,
    headers: &HeaderMap,
    path: &str,
) -> Result<(), ApiError> {
    if rate_limit::is_service(state, headers) {
        Ok(())
    } else {
        Err(ApiError::Forbidden {
            detail: "This endpoint is reserved to the site (Pulsars-Service-Key).".to_owned(),
            instance: Some(path.to_owned()),
        })
    }
}

/// A per-action limit, counted even for the site: it forwards its
/// visitors' attempts.
pub(crate) async fn limit(
    state: &AppState,
    action: &str,
    who: &str,
    limit: u32,
    window: i64,
) -> Result<(), ApiError> {
    match rate_limit::attempt(&state.redis, action, who, limit, window).await {
        Some(decision) if !decision.allowed() => Err(ApiError::TooManyAttempts {
            retry_after: decision.reset.max(1),
        }),
        _ => Ok(()),
    }
}

// --- Validation (Laravel's CreateNewUser and Password::defaults) ------------

/// 3 to 30 letters, digits, `_` or `-` (`/^[\pL\pN_-]+$/u`).
pub(crate) fn check_pseudo(pseudo: &str, errors: &mut Vec<ParameterError>) -> bool {
    let chars = pseudo.chars().count();
    let error = if pseudo.is_empty() {
        Some(("required", "is required"))
    } else if chars < 3 {
        Some(("too_short", "must be at least 3 characters"))
    } else if chars > 30 {
        Some(("too_long", "must be at most 30 characters"))
    } else if !pseudo
        .chars()
        .all(|c| c.is_alphabetic() || c.is_numeric() || c == '_' || c == '-')
    {
        Some((
            "invalid_format",
            "may only contain letters, digits, `_` and `-`",
        ))
    } else {
        None
    };
    push(errors, "pseudo", error)
}

fn check_email(email: &str, errors: &mut Vec<ParameterError>) -> bool {
    let error = if email.is_empty() {
        Some(("required", "is required"))
    } else if email.chars().count() > 255 {
        Some(("too_long", "must be at most 255 characters"))
    } else if !is_email(email) {
        Some(("invalid_email", "must be a valid e-mail address"))
    } else {
        None
    };
    push(errors, "email", error)
}

/// One `@`, a local part, a domain with a dot that does not start or end
/// it, no whitespace or control character.
fn is_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// 8 to 255 characters, confirmed. A mismatch is reported on `password`,
/// as Laravel's `confirmed` rule does.
pub(crate) fn check_password(password: &str, confirmation: &str, errors: &mut Vec<ParameterError>) {
    let chars = password.chars().count();
    let error = if password.is_empty() {
        Some(("required", "is required"))
    } else if chars < 8 {
        Some(("too_short", "must be at least 8 characters"))
    } else if chars > 255 {
        Some(("too_long", "must be at most 255 characters"))
    } else if password != confirmation {
        Some(("mismatch", "does not match password_confirmation"))
    } else {
        None
    };
    push(errors, "password", error);
}

fn push(
    errors: &mut Vec<ParameterError>,
    field: &str,
    error: Option<(&'static str, &str)>,
) -> bool {
    match error {
        Some((code, detail)) => {
            errors.push(ParameterError::field(field, code, detail));
            false
        }
        None => true,
    }
}

fn taken(field: &str) -> ParameterError {
    ParameterError::field(field, "taken", "is already used by another account")
}

/// Errors in the order of the form.
fn field_order(field: &str) -> usize {
    ["pseudo", "email", "password", "locale"]
        .iter()
        .position(|f| *f == field)
        .unwrap_or(usize::MAX)
}

// --- Responses -------------------------------------------------------------

pub(crate) fn no_store<T: Serialize>(status: StatusCode, body: &T) -> Response {
    let mut response = (status, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(crate) fn empty() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(check: impl Fn(&mut Vec<ParameterError>)) -> Vec<(String, &'static str)> {
        let mut errors = Vec::new();
        check(&mut errors);
        errors
            .into_iter()
            .map(|e| (e.parameter, e.code.unwrap_or_default()))
            .collect()
    }

    #[test]
    fn pseudos_follow_laravel_rules() {
        for (pseudo, code) in [
            ("", Some("required")),
            ("ab", Some("too_short")),
            (&"a".repeat(31) as &str, Some("too_long")),
            ("a b", Some("invalid_format")),
            ("a.b", Some("invalid_format")),
            ("jikuji", None),
            ("Élève_42-x", None),
            ("Пушкин", None),
        ] {
            let got = codes(|e| {
                check_pseudo(pseudo, e);
            });
            assert_eq!(got.first().map(|(_, c)| *c), code, "{pseudo}");
        }
    }

    #[test]
    fn emails_need_a_local_part_and_a_dotted_domain() {
        for (email, ok) in [
            ("a@b.c", true),
            ("ada+test@example.org", true),
            ("a@b", false),
            ("@b.c", false),
            ("a@@b.c", false),
            ("a b@c.d", false),
            ("a@.b", false),
            ("a@b.", false),
        ] {
            assert_eq!(is_email(email), ok, "{email}");
        }
    }

    #[test]
    fn passwords_are_8_to_255_and_confirmed() {
        let code = |p: &str, c: &str| {
            codes(|e| check_password(p, c, e))
                .first()
                .map(|(_, code)| *code)
        };
        assert_eq!(code("", ""), Some("required"));
        assert_eq!(code("1234567", "1234567"), Some("too_short"));
        assert_eq!(code("12345678", "12345679"), Some("mismatch"));
        assert_eq!(code(&"x".repeat(256), &"x".repeat(256)), Some("too_long"));
        assert_eq!(code("12345678", "12345678"), None);
    }

    #[test]
    fn tokens_are_43_url_safe_characters() {
        let token = new_token().unwrap();
        assert_eq!(token.len(), TOKEN_CHARS);
        let mut headers = HeaderMap::new();
        headers.insert(SESSION_HEADER, token.parse().unwrap());
        assert_eq!(session_token(&headers), Some(token.as_str()));
        headers.insert(SESSION_HEADER, "short".parse().unwrap());
        assert_eq!(session_token(&headers), None);
        assert_ne!(new_token().unwrap(), token);
    }
}
