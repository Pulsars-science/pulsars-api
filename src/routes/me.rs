//! `GET /v1/me`: public profile of the token's member (see `openapi.yaml`,
//! schema `Me`).
//!
//! What `https://pulsars.science/membres/{pseudo}` shows to anyone, minus
//! school level and class. With a game token, never e-mail, locale, social
//! logins or moderation state.
//!
//! With the site's session (`Pulsars-Session`, for the site's header), the
//! owner's own account state is added: `email`, `email_verified`,
//! `onboarded`, `locale` and `forum_muted_until`.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use chrono::SecondsFormat;
use serde::Serialize;

use crate::auth::member::MaybeMember;
use crate::auth::{AuthUser, user_id as parse_user_id};
use crate::db::members::{self, Member, MemberRole};
use crate::error::ApiError;
use crate::links::{self, encode};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub data: Me,
}

#[derive(Debug, Serialize)]
pub struct Me {
    pub id: String,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub member_since: String,
    pub roles: Vec<Role>,
    pub xp: Xp,
    /// Brains balance (`brains.md` §4): the token's owner always sees it.
    pub brains: i64,
    pub profile_url: String,
    /// With a session only.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
}

/// The owner's account state, for the site's header.
#[derive(Debug, Serialize)]
pub struct Account {
    pub email: String,
    pub email_verified: bool,
    pub onboarded: bool,
    pub locale: Option<String>,
    /// Muted by a moderator until then (forum, messages).
    pub forum_muted_until: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Role {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Xp {
    pub total: i64,
    pub level: i64,
    /// XP at which the current level starts.
    pub level_start: i64,
    /// XP at which the next level starts.
    pub next_level_at: i64,
}

impl Xp {
    /// Level `n` starts at `50 × (n − 1) × n` XP (`Experience::seuil`). The
    /// level itself comes from `site.member_xp`.
    pub fn new(total: i32, level: i32) -> Self {
        let level = i64::from(level.max(1));
        Self {
            total: i64::from(total),
            level,
            level_start: 50 * (level - 1) * level,
            next_level_at: 50 * level * (level + 1),
        }
    }
}

pub async fn me(
    State(state): State<AppState>,
    MaybeMember(session): MaybeMember,
    token: Result<AuthUser, ApiError>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let gone = || ApiError::NotFound {
        detail: "The account of this token no longer exists.".to_owned(),
        instance: Some(uri.path().to_owned()),
    };
    let (id, account) = match session {
        Some(member) => {
            let account = Account {
                email: member.account.email.clone(),
                email_verified: member.account.email_verified_at.is_some(),
                onboarded: member.account.onboarding_at.is_some(),
                locale: member.account.locale.clone(),
                forum_muted_until: member
                    .muted_until
                    .filter(|_| member.is_muted())
                    .map(|at| at.to_rfc3339_opts(SecondsFormat::Secs, true)),
            };
            (member.id(), Some(account))
        }
        None => {
            let AuthUser(claims) = token?;
            // `sub` is the numeric `users.id` written in decimal; anything
            // else cannot name an account.
            (parse_user_id(&claims.sub).ok_or_else(gone)?, None)
        }
    };

    let member = members::find(&state.db, id).await?.ok_or_else(gone)?;
    let roles = members::roles(&state.db, id).await?;

    let mut me = to_me(member, roles, &state.config.public_storage_url)?;
    me.account = account;
    let body = MeResponse { data: me };
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.append(header::VARY, HeaderValue::from_static("Authorization"));
    headers.append(header::VARY, HeaderValue::from_static("Pulsars-Session"));
    Ok(response)
}

fn to_me(member: Member, roles: Vec<MemberRole>, storage_url: &str) -> Result<Me, ApiError> {
    // `created_at` is always set; the schema has no null here.
    let member_since = member
        .member_since
        .ok_or_else(|| ApiError::Internal(format!("member {} has no created_at", member.id)))?
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    Ok(Me {
        id: member.id.to_string(),
        profile_url: links::member(&member.pseudo),
        avatar_url: member
            .avatar_path
            .filter(|p| !p.is_empty())
            .map(|p| format!("{storage_url}/{}", encode(p.trim_start_matches('/'), true))),
        pseudo: member.pseudo,
        bio: member.bio,
        member_since,
        roles: roles
            .into_iter()
            .map(|r| Role {
                slug: r.slug,
                name: r.name,
                category: r.category,
                color: r.color,
            })
            .collect(),
        xp: Xp::new(member.xp_total, member.xp_level),
        brains: i64::from(member.brains),
        account: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_matches_the_laravel_scale() {
        assert_eq!(
            Xp::new(0, 1),
            Xp {
                total: 0,
                level: 1,
                level_start: 0,
                next_level_at: 100
            }
        );
        assert_eq!(
            Xp::new(102, 2),
            Xp {
                total: 102,
                level: 2,
                level_start: 100,
                next_level_at: 300
            }
        );
        let xp = Xp::new(300, 3);
        assert_eq!((xp.level_start, xp.next_level_at), (300, 600));
    }

    #[test]
    fn only_canonical_ids_name_an_account() {
        assert_eq!(parse_user_id("42"), Some(42));
        for bad in [
            "",
            "0",
            "042",
            "+42",
            "-1",
            "4 2",
            "42a",
            "99999999999999999999",
        ] {
            assert_eq!(parse_user_id(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn profile_matches_the_schema() {
        let member = Member {
            id: 1,
            pseudo: "jikuji".to_owned(),
            avatar_path: Some("avatars/1.webp".to_owned()),
            bio: None,
            member_since: chrono::DateTime::parse_from_rfc3339("2026-08-07T14:44:15Z")
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc)),
            xp_total: 0,
            xp_level: 1,
            brains: 42,
        };
        let roles = vec![MemberRole {
            slug: "super-admin".to_owned(),
            name: "Super admin".to_owned(),
            category: "permission".to_owned(),
            color: "#d1544f".to_owned(),
        }];
        let me = to_me(member, roles, "https://pulsars.science/storage").expect("valid");
        let json = serde_json::to_value(&me).unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "1",
                "pseudo": "jikuji",
                "avatar_url": "https://pulsars.science/storage/avatars/1.webp",
                "bio": null,
                "member_since": "2026-08-07T14:44:15Z",
                "roles": [{"slug": "super-admin", "name": "Super admin",
                           "category": "permission", "color": "#d1544f"}],
                "xp": {"total": 0, "level": 1, "level_start": 0, "next_level_at": 100},
                "brains": 42,
                "profile_url": "https://pulsars.science/membres/jikuji",
            })
        );
    }
}
