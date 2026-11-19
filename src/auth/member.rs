//! The member behind a request of the site: `Pulsars-Session` (the token of
//! `/v1/auth/login`, relayed by pulsars-web from its `pulsars_auth` cookie),
//! looked up in `site.auth_sessions`, with the account's permission level
//! read from its roles. Every route of the site's members uses one of these
//! extractors; the games keep their short JWT (`AuthUser`).
//!
//! - [`Member`]: any live session; 401 `session-invalid` without one.
//! - [`MaybeMember`]: the same, or nobody, for public pages that change
//!   with the reader (a vote already cast, a course already started).
//! - [`Verified`]: a member whose address is verified and who has finished
//!   the welcome steps, like Laravel's `verified` and `onboarding`
//!   middlewares; 403 otherwise.
//! - [`ModeratorMember`], [`AdminMember`], [`SuperAdminMember`]: a
//!   verified member with that permission level (`site.roles.niveau`).
//! - [`EsgiMember`]: a verified member whose address belongs to myskolae.fr,
//!   a site admin, or a member given the `esgi` role (read access only;
//!   imports stay with `admin_esgi`).
//! - [`EsgiAdminMember`]: a verified member carrying the isolated
//!   `admin_esgi` role (or a super admin).

use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::request::Parts;
use chrono::{DateTime, Utc};

use crate::db::accounts::AccountRow;
use crate::error::ApiError;
use crate::routes::auth::{live_session, session_token};
use crate::state::AppState;

/// Permission levels of `site.roles.niveau` (Laravel's `Role::NIVEAU_*`).
pub const LEVEL_MODERATOR: i16 = 1;
pub const LEVEL_ADMIN: i16 = 2;
pub const LEVEL_SUPER_ADMIN: i16 = 3;

#[derive(Debug, Clone)]
pub struct Member {
    pub account: AccountRow,
    /// Highest `niveau` of the member's permission roles, 0 for none.
    pub level: i16,
    /// Muted by a moderator until then: may read, not post.
    pub muted_until: Option<DateTime<Utc>>,
}

impl Member {
    pub fn id(&self) -> i64 {
        self.account.id
    }

    pub fn is_moderator(&self) -> bool {
        self.level >= LEVEL_MODERATOR
    }

    pub fn is_admin(&self) -> bool {
        self.level >= LEVEL_ADMIN
    }

    /// Whether a moderator's mute still holds.
    pub fn is_muted(&self) -> bool {
        self.muted_until.is_some_and(|until| until > Utc::now())
    }

    /// 403 `forbidden` while the member is muted (forum posts, messages).
    pub fn require_unmuted(&self, instance: &str) -> Result<(), ApiError> {
        if self.is_muted() {
            return Err(forbidden(
                instance,
                "This member is muted and may not post for now.",
            ));
        }
        Ok(())
    }
}

async fn load(parts: &Parts, state: &AppState) -> Result<Option<Member>, ApiError> {
    let Some(token) = session_token(&parts.headers) else {
        return Ok(None);
    };
    let (_, account) = live_session(state, token).await?;
    let (level, muted_until): (i16, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT COALESCE((SELECT max(r.niveau) FROM site.role_user ru
                            JOIN site.roles r ON r.id = ru.role_id
                           WHERE ru.user_id = u.id AND r.categorie = 'permission'), 0)::smallint,
                u.mute_jusqu_a AT TIME ZONE 'UTC'
           FROM site.users u WHERE u.id = $1",
    )
    .bind(account.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Some(Member {
        account,
        level,
        muted_until,
    }))
}

fn instance(parts: &Parts) -> String {
    parts
        .extensions
        .get::<OriginalUri>()
        .map_or_else(|| parts.uri.path().to_owned(), |u| u.path().to_owned())
}

fn forbidden(instance: &str, detail: &str) -> ApiError {
    ApiError::Forbidden {
        detail: detail.to_owned(),
        instance: Some(instance.to_owned()),
    }
}

impl FromRequestParts<AppState> for Member {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        load(parts, state).await?.ok_or(ApiError::SessionInvalid)
    }
}

/// A member, or nobody. An invalid token counts as nobody: the page is
/// public anyway.
#[derive(Debug, Clone)]
pub struct MaybeMember(pub Option<Member>);

impl FromRequestParts<AppState> for MaybeMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        match load(parts, state).await {
            Ok(member) => Ok(Self(member)),
            Err(ApiError::SessionInvalid) => Ok(Self(None)),
            Err(error) => Err(error),
        }
    }
}

/// A member whose address is verified and who has finished the welcome
/// steps (`POST /v1/me/onboarding`).
#[derive(Debug, Clone)]
pub struct Verified(pub Member);

async fn verified(parts: &Parts, state: &AppState) -> Result<Member, ApiError> {
    let member = load(parts, state).await?.ok_or(ApiError::SessionInvalid)?;
    if member.account.email_verified_at.is_none() {
        return Err(forbidden(
            &instance(parts),
            "Verify your e-mail address first.",
        ));
    }
    if member.account.onboarding_at.is_none() {
        return Err(forbidden(
            &instance(parts),
            "Finish the welcome steps first.",
        ));
    }
    Ok(member)
}

async fn with_level(
    parts: &Parts,
    state: &AppState,
    level: i16,
    needed: &str,
) -> Result<Member, ApiError> {
    let member = verified(parts, state).await?;
    if member.level < level {
        return Err(forbidden(
            &instance(parts),
            &format!("This endpoint needs {needed}."),
        ));
    }
    Ok(member)
}

impl FromRequestParts<AppState> for Verified {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        verified(parts, state).await.map(Self)
    }
}

#[derive(Debug, Clone)]
pub struct ModeratorMember(pub Member);

impl FromRequestParts<AppState> for ModeratorMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        with_level(parts, state, LEVEL_MODERATOR, "a moderator")
            .await
            .map(Self)
    }
}

#[derive(Debug, Clone)]
pub struct AdminMember(pub Member);

impl FromRequestParts<AppState> for AdminMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        with_level(parts, state, LEVEL_ADMIN, "an admin")
            .await
            .map(Self)
    }
}

#[derive(Debug, Clone)]
pub struct SuperAdminMember(pub Member);

impl FromRequestParts<AppState> for SuperAdminMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        with_level(parts, state, LEVEL_SUPER_ADMIN, "a super admin")
            .await
            .map(Self)
    }
}

/// Whether an address belongs exactly to the ESGI account domain.
pub fn is_esgi_email(email: &str) -> bool {
    email.trim().to_ascii_lowercase().ends_with("@myskolae.fr")
}

#[derive(Debug, Clone)]
pub struct EsgiMember(pub Member);

async fn has_role(member: &Member, slug: &str, state: &AppState) -> Result<bool, ApiError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM site.role_user ru
            JOIN site.roles r ON r.id = ru.role_id
            WHERE ru.user_id = $1 AND r.slug = $2
        )",
    )
    .bind(member.id())
    .bind(slug)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)
}

async fn can_admin_esgi(member: &Member, state: &AppState) -> Result<bool, ApiError> {
    if member.level >= LEVEL_SUPER_ADMIN {
        return Ok(true);
    }
    has_role(member, "admin_esgi", state).await
}

impl FromRequestParts<AppState> for EsgiMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let member = verified(parts, state).await?;
        // Site admins can read the space too, to see what students see, and
        // give the `esgi` role to someone without a school address.
        let allowed = is_esgi_email(&member.account.email)
            || member.level >= LEVEL_ADMIN
            || has_role(&member, "esgi", state).await?
            || can_admin_esgi(&member, state).await?;
        if !allowed {
            return Err(forbidden(
                &instance(parts),
                "This endpoint is reserved for @myskolae.fr accounts.",
            ));
        }
        Ok(Self(member))
    }
}

#[derive(Debug, Clone)]
pub struct EsgiAdminMember(pub Member);

impl FromRequestParts<AppState> for EsgiAdminMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let member = verified(parts, state).await?;
        let allowed = can_admin_esgi(&member, state).await?;
        if !allowed {
            return Err(forbidden(
                &instance(parts),
                "This endpoint needs the admin_esgi role.",
            ));
        }
        Ok(Self(member))
    }
}

#[cfg(test)]
mod esgi_tests {
    use super::is_esgi_email;

    #[test]
    fn esgi_domain_is_exact_and_case_insensitive() {
        assert!(is_esgi_email("Ada@MYSKOLAE.FR"));
        assert!(!is_esgi_email("ada@myskolae.fr.example.org"));
        assert!(!is_esgi_email("ada@fake-myskolae.fr"));
    }
}
