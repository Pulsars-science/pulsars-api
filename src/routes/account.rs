//! Links sent by mail (`docs/contract/auth-sessions.md`, « Mails »): the
//! address verification link and the forgotten password link, as Laravel's
//! `VerifyEmail` and `ResetPassword` notifications did.
//!
//! The links point to the site (`/email/verifier/{token}`,
//! `/reset-password/{token}?email=`), which posts the token back here. Only
//! the SHA-256 of a token is stored.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;

use super::auth::{
    self, AccountUser, Caller, Data, check_password, current, digest, empty, hash, limit,
    new_token, no_store,
};
use crate::db::accounts::{self, AccountRow};
use crate::db::email_tokens::{self, Kind};
use crate::db::sessions;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::links;
use crate::mail::{self, Lang, Mail};
use crate::state::AppState;

/// A verification link works a day.
const VERIFY_MINUTES: i64 = 24 * 60;
/// A reset link works an hour (Laravel's `auth.passwords.users.expire`).
const RESET_MINUTES: i64 = 60;
/// At most one reset mail a minute per account (`throttle`, 60 s).
const RESET_THROTTLE_SECS: i64 = 60;
/// Verification mails per minute per account (`throttle:6,1`).
const VERIFY_LIMIT: u32 = 6;
/// Forgotten password requests per minute per address.
const FORGOT_LIMIT: u32 = 5;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LangBody {
    /// The mail's language when the account has none.
    lang: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TokenBody {
    token: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ForgotBody {
    email: String,
    lang: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResetWithTokenBody {
    email: String,
    token: String,
    password: String,
    password_confirmation: String,
}

#[derive(Debug, serde::Serialize)]
struct Verified {
    user: AccountUser,
}

/// `POST /v1/auth/email/verification`: sends the verification link again.
/// Nothing is sent for an address already verified.
pub async fn send_again(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LangBody>,
) -> Result<Response, ApiError> {
    let (_, account) = current(&state, &headers).await?;
    if account.email_verified_at.is_some() {
        return Ok(empty());
    }
    limit(
        &state,
        "verification",
        &account.id.to_string(),
        VERIFY_LIMIT,
        crate::rate_limit::WINDOW_SECS,
    )
    .await?;
    send_verification(&state, &account, body.lang.as_deref()).await?;
    Ok(empty())
}

/// `POST /v1/auth/email/verify`: the token of a verification link.
pub async fn verify(
    State(state): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<Response, ApiError> {
    if !auth::is_token(&body.token) {
        return Err(ApiError::InvalidToken);
    }
    let user_id = email_tokens::verify_email(&state.db, &digest(&body.token))
        .await?
        .ok_or(ApiError::InvalidToken)?;
    let account = accounts::find(&state.db, user_id)
        .await?
        .ok_or(ApiError::InvalidToken)?;
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Verified {
                user: account.into(),
            },
        },
    ))
}

/// `POST /v1/auth/password/forgot`: always 204, so the answer never tells
/// whether an account has this address.
pub async fn forgot(
    State(state): State<AppState>,
    caller: Caller,
    Json(body): Json<ForgotBody>,
) -> Result<Response, ApiError> {
    let ip = caller.ip.unwrap_or_else(|| "unknown".to_owned());
    limit(
        &state,
        "forgot",
        &ip,
        FORGOT_LIMIT,
        crate::rate_limit::WINDOW_SECS,
    )
    .await?;
    let email = body.email.trim();
    if email.is_empty() {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "email",
            "required",
            "is required",
        )]));
    }
    let Some(account) = accounts::find_by_email(&state.db, email).await? else {
        return Ok(empty());
    };
    if email_tokens::sent_within(&state.db, account.id, Kind::Reset, RESET_THROTTLE_SECS).await? {
        return Ok(empty());
    }
    let token = new_token()?;
    email_tokens::create(
        &state.db,
        account.id,
        Kind::Reset,
        &digest(&token),
        &account.email,
        RESET_MINUTES,
    )
    .await?;
    let lang = Lang::pick([account.locale.as_deref(), body.lang.as_deref()]);
    state.mailer.send(Mail {
        kind: mail::Kind::Reset,
        user_id: account.id,
        to: account.email.clone(),
        content: mail::templates::reset(
            lang,
            &links::password_reset(&token, &account.email),
            RESET_MINUTES,
        ),
    });
    Ok(empty())
}

/// `POST /v1/auth/password/reset-with-token`: the form of a reset link.
/// Every session of the account is revoked.
pub async fn reset_with_token(
    State(state): State<AppState>,
    Json(body): Json<ResetWithTokenBody>,
) -> Result<Response, ApiError> {
    let mut errors = Vec::new();
    check_password(&body.password, &body.password_confirmation, &mut errors);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    if !auth::is_token(&body.token) || body.email.trim().is_empty() {
        return Err(ApiError::InvalidToken);
    }
    let user_id = email_tokens::use_reset(&state.db, &digest(&body.token), body.email.trim())
        .await?
        .ok_or(ApiError::InvalidToken)?;
    let hash = hash(state.config.bcrypt_cost, body.password).await?;
    accounts::set_password(&state.db, user_id, &hash).await?;
    sessions::revoke_others(&state.db, user_id, None).await?;
    Ok(empty())
}

/// Stores a verification link for `account` and mails it, in the account's
/// language, else `lang`, else French.
pub(crate) async fn send_verification(
    state: &AppState,
    account: &AccountRow,
    lang: Option<&str>,
) -> Result<(), ApiError> {
    let token = new_token()?;
    email_tokens::create(
        &state.db,
        account.id,
        Kind::Verify,
        &digest(&token),
        &account.email,
        VERIFY_MINUTES,
    )
    .await?;
    let lang = Lang::pick([account.locale.as_deref(), lang]);
    state.mailer.send(Mail {
        kind: mail::Kind::Verification,
        user_id: account.id,
        to: account.email.clone(),
        content: mail::templates::verification(lang, &links::email_verification(&token)),
    });
    Ok(())
}
