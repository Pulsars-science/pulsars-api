//! Google accounts (`docs/contract/auth-sessions.md`, « Google »). The site
//! still talks to Google (Socialite, its redirect URL); it hands the
//! identity it got to the API, which finds, links or creates the account
//! with the rules of the site's `ConnexionSociale::resoudre` and
//! `ConnexionSocialeController::rattacher`, then opens the session.

use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::auth::{
    AccountUser, Caller, Data, NewSession, account_of, current, empty, new_session, no_store,
    require_service,
};
use crate::db::accounts::{self, AccountRow, CreateError};
use crate::db::social;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::state::AppState;

/// The providers the site knows. Only Google is offered today; Discord's
/// rules are the same.
const PROVIDERS: &[&str] = &["google", "discord"];

/// Pseudos tried per query while looking for a free one.
const PSEUDO_BATCH: u32 = 100;

/// Tries when another request creates the same account at the same time.
const ATTEMPTS: usize = 3;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResolveBody {
    provider: String,
    /// The provider's id of the person (`sub` for Google).
    subject: String,
    email: Option<String>,
    /// Whether the provider says the address is the person's.
    email_verified: bool,
    name: Option<String>,
    nickname: Option<String>,
    remember: bool,
    /// The member already logged in on the site: the identity is linked
    /// to their account, and no session is opened.
    current_user_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct Resolved {
    user: AccountUser,
    session: Option<NewSession>,
    /// A new account was created.
    created: bool,
    /// The identity was linked to an account that existed before.
    linked: bool,
}

/// `POST /v1/auth/oauth/resolve`. Service key only.
pub async fn resolve(
    State(state): State<AppState>,
    caller: Caller,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<ResolveBody>,
) -> Result<Response, ApiError> {
    require_service(&state, &headers, uri.path())?;
    let mut errors = Vec::new();
    if !PROVIDERS.contains(&body.provider.as_str()) {
        errors.push(ParameterError::field(
            "provider",
            "invalid",
            "must be google or discord",
        ));
    }
    if body.subject.trim().is_empty() || body.subject.chars().count() > 255 {
        errors.push(ParameterError::field(
            "subject",
            "invalid",
            "must be the provider's id, 1 to 255 characters",
        ));
    }
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let provider = body.provider.as_str();
    let subject = body.subject.as_str();
    let email = body
        .email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());

    if let Some(user_id) = &body.current_user_id {
        let account = account_of(&state, user_id, uri.path()).await?;
        return attach(&state, account, provider, subject, email).await;
    }

    for _ in 0..ATTEMPTS {
        if let Some(owner) = social::owner(&state.db, provider, subject).await? {
            let account = accounts::find(&state.db, owner)
                .await?
                .ok_or_else(|| ApiError::Internal(format!("linked account {owner} is gone")))?;
            return opened(&state, account, &body, &caller, false, false).await;
        }
        let Some(email) = email else {
            return Err(ApiError::InvalidFields(vec![ParameterError::field(
                "email",
                "email_missing",
                "the provider shares no e-mail address",
            )]));
        };
        let existing = accounts::find_by_email(&state.db, email).await?;
        if existing.is_some() && !body.email_verified {
            // Linking here would give the account to whoever claims its
            // address at the provider.
            return Err(ApiError::Conflict {
                reason: "email_unverified",
                detail: format!(
                    "An account already uses this address, and {} does not confirm it is the person's.",
                    title(provider)
                ),
            });
        }
        let pseudo = match &existing {
            Some(_) => None,
            None => Some(free_pseudo(&state, &body).await?),
        };
        match create_and_link(&state, existing, pseudo, provider, subject, email, &body).await {
            Ok((account, created)) => {
                return opened(&state, account, &body, &caller, created, !created).await;
            }
            // Someone else linked this identity, or took this address or
            // pseudo, in between: look again.
            Err(Race::Again) => continue,
            Err(Race::Error(error)) => return Err(error),
        }
    }
    Err(ApiError::Internal(
        "could not resolve a provider identity after several tries".to_owned(),
    ))
}

/// `DELETE /v1/auth/oauth/{provider}`: unlinks the provider from the
/// session's account, unless it is the last way to log in.
pub async fn unlink(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider): Path<String>,
) -> Result<Response, ApiError> {
    let (_, account) = current(&state, &headers).await?;
    let others = social::providers(&state.db, account.id)
        .await?
        .into_iter()
        .any(|p| p != provider);
    if !others && account.password.is_none() {
        return Err(ApiError::Conflict {
            reason: "last_login_method",
            detail: "This is the account's only way to log in: set a password first.".to_owned(),
        });
    }
    social::unlink(&state.db, account.id, &provider).await?;
    Ok(empty())
}

/// Links the identity to the account already logged in on the site.
async fn attach(
    state: &AppState,
    account: AccountRow,
    provider: &str,
    subject: &str,
    email: Option<&str>,
) -> Result<Response, ApiError> {
    if let Some(owner) = social::owner(&state.db, provider, subject).await?
        && owner != account.id
    {
        return Err(taken(provider));
    }
    let mut tx = state.db.begin().await?;
    match social::link(&mut tx, account.id, provider, subject, email).await {
        Ok(()) => tx.commit().await?,
        Err(error) if is_unique(&error) => return Err(taken(provider)),
        Err(error) => return Err(error.into()),
    }
    Ok(no_store(
        StatusCode::OK,
        &Data {
            data: Resolved {
                user: account.into(),
                session: None,
                created: false,
                linked: true,
            },
        },
    ))
}

enum Race {
    Again,
    Error(ApiError),
}

/// In one transaction: the account (`existing`, or a new one with
/// `pseudo`) and its link to the identity.
async fn create_and_link(
    state: &AppState,
    existing: Option<AccountRow>,
    pseudo: Option<String>,
    provider: &str,
    subject: &str,
    email: &str,
    body: &ResolveBody,
) -> Result<(AccountRow, bool), Race> {
    let mut tx = state.db.begin().await.map_err(|e| Race::Error(e.into()))?;
    let (account, created) = match (existing, pseudo) {
        (Some(account), _) => (account, false),
        (None, Some(pseudo)) => {
            match accounts::create_social(&mut tx, &pseudo, email, body.email_verified).await {
                Ok(account) => (account, true),
                Err(CreateError::PseudoTaken | CreateError::EmailTaken) => return Err(Race::Again),
                Err(CreateError::Database(error)) => return Err(Race::Error(error.into())),
            }
        }
        (None, None) => return Err(Race::Error(ApiError::Internal("no pseudo".to_owned()))),
    };
    match social::link(&mut tx, account.id, provider, subject, Some(email)).await {
        Ok(()) => {}
        Err(error) if is_unique(&error) => return Err(Race::Again),
        Err(error) => return Err(Race::Error(error.into())),
    }
    tx.commit().await.map_err(|e| Race::Error(e.into()))?;
    Ok((account, created))
}

async fn opened(
    state: &AppState,
    account: AccountRow,
    body: &ResolveBody,
    caller: &Caller,
    created: bool,
    linked: bool,
) -> Result<Response, ApiError> {
    let session = new_session(state, account.id, body.remember, caller).await?;
    Ok(no_store(
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        &Data {
            data: Resolved {
                user: account.into(),
                session: Some(session),
                created,
                linked,
            },
        },
    ))
}

/// A free pseudo from the provider's profile, as the site's
/// `ConnexionSociale::pseudoLibre`: the nickname, else the name, spaces
/// made dashes, other characters dropped, 30 at most, `membre` when too
/// short; then `-2`, `-3`… within 30 characters; then `membre-` and 12
/// random characters.
async fn free_pseudo(state: &AppState, body: &ResolveBody) -> Result<String, ApiError> {
    let base = base_pseudo(body.nickname.as_deref(), body.name.as_deref());
    let mut start = 1u32;
    while start < 10_000 {
        let end = (start + PSEUDO_BATCH).min(10_000);
        let candidates: Vec<String> = (start..end).map(|n| candidate(&base, n)).collect();
        let taken = accounts::taken_pseudos(&state.db, &candidates).await?;
        if let Some(free) = candidates
            .into_iter()
            .find(|c| !taken.contains(&c.to_lowercase()))
        {
            return Ok(free);
        }
        start = end;
    }
    Ok(format!("membre-{}", random_lower(12)?))
}

fn base_pseudo(nickname: Option<&str>, name: Option<&str>) -> String {
    let wanted = [nickname, name]
        .into_iter()
        .flatten()
        .find(|v| !v.is_empty())
        .unwrap_or("");
    let base: String = wanted
        .trim()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphabetic() || c.is_numeric() || *c == '_' || *c == '-')
        .take(30)
        .collect();
    if base.chars().count() < 3 {
        "membre".to_owned()
    } else {
        base
    }
}

/// The `n`-th try: the base itself, then `base-n` within 30 characters.
fn candidate(base: &str, n: u32) -> String {
    if n == 1 {
        return base.to_owned();
    }
    let suffix = n.to_string();
    let keep = 30 - suffix.len() - 1;
    format!("{}-{suffix}", base.chars().take(keep).collect::<String>())
}

fn random_lower(len: usize) -> Result<String, ApiError> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness for a pseudo: {e}")))?;
    Ok(bytes
        .into_iter()
        .map(|b| char::from(ALPHABET[usize::from(b) % ALPHABET.len()]))
        .collect())
}

fn is_unique(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|e| e.code().as_deref() == Some("23505"))
}

fn taken(provider: &str) -> ApiError {
    ApiError::Conflict {
        reason: "taken",
        detail: format!(
            "This {} account is already linked to another Pulsars account.",
            title(provider)
        ),
    }
}

fn title(provider: &str) -> String {
    let mut chars = provider.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudos_come_from_the_profile_like_the_site() {
        assert_eq!(base_pseudo(Some("ada_l"), Some("Ada")), "ada_l");
        assert_eq!(
            base_pseudo(Some(""), Some(" Ada Lovelace ")),
            "Ada-Lovelace"
        );
        assert_eq!(base_pseudo(None, Some("Zoé d'Arc!")), "Zoé-dArc");
        assert_eq!(base_pseudo(None, Some("Пушкин")), "Пушкин");
        assert_eq!(base_pseudo(None, Some("A.")), "membre");
        assert_eq!(base_pseudo(None, None), "membre");
        assert_eq!(base_pseudo(None, Some(&"x".repeat(40))).len(), 30);
    }

    #[test]
    fn suffixes_keep_pseudos_within_30_characters() {
        assert_eq!(candidate("ada", 1), "ada");
        assert_eq!(candidate("ada", 2), "ada-2");
        let long = "y".repeat(30);
        assert_eq!(candidate(&long, 2), format!("{}-2", "y".repeat(28)));
        assert_eq!(candidate(&long, 9999).chars().count(), 30);
        assert_eq!(title("google"), "Google");
    }
}
