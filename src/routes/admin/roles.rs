//! `/v1/admin/members` and `/v1/admin/members/{id}/roles/{slug}` (Laravel's
//! `GestionRolesController`): giving roles to accounts.
//!
//! Against escalation, a permission role (moderator, admin, super admin)
//! is given or taken by a super admin only: an admin manages subjects and
//! distinctions, but makes no other admin and cannot raise themself. Nobody
//! takes their own super admin role away (and locks themself out).

use std::collections::BTreeMap;

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use serde::Serialize;

use super::{Listing, not_found, offset, ok, page, refused};
use crate::auth::member::{AdminMember, LEVEL_SUPER_ADMIN};
use crate::error::ApiError;
use crate::routes::auth::no_store;
use crate::routes::params::{Params, like_pattern};
use crate::state::AppState;

const PER_PAGE: u32 = 25;

#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    id: i64,
    pseudo: String,
    email: String,
    roles: Vec<String>,
    goals: Option<serde_json::Value>,
    niveau_scolaire: Option<String>,
    classe: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Account {
    pub id: String,
    pub pseudo: String,
    pub email: String,
    /// Slugs of every role the account has.
    pub roles: Vec<String>,
    /// The goals chosen at sign-up (keys of `Onboarding::objectifs()`).
    pub goals: Vec<String>,
    /// `college`, `lycee`, `superieur`…
    pub school_level: Option<String>,
    pub class: Option<String>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct CatalogRole {
    pub slug: String,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    #[serde(skip)]
    categorie: String,
}

#[derive(Debug, Serialize)]
struct RolesPage {
    #[serde(flatten)]
    listing: Listing<Account, Empty>,
    /// Roles by category (`permission`, `matiere`, `distinction`), highest
    /// level first.
    catalog: BTreeMap<String, Vec<CatalogRole>>,
    /// Whether the reader may give or take permission roles.
    can_manage_permissions: bool,
}

#[derive(Debug, Serialize)]
struct Empty {}

const SELECT: &str = "SELECT u.id, u.pseudo, u.email,
        ARRAY(SELECT r.slug::text FROM site.role_user ru JOIN site.roles r ON r.id = ru.role_id
               WHERE ru.user_id = u.id
               ORDER BY r.niveau DESC, r.ordre, r.nom) AS roles,
        u.objectif::jsonb AS goals, u.niveau_scolaire, u.classe
   FROM site.users u";

fn to_account(row: AccountRow) -> Account {
    // A single goal was once stored as a bare string.
    let goals = match row.goals {
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Some(serde_json::Value::String(goal)) => vec![goal],
        _ => Vec::new(),
    };
    Account {
        id: row.id.to_string(),
        pseudo: row.pseudo,
        email: row.email,
        roles: row.roles,
        goals,
        school_level: row.niveau_scolaire,
        class: row.classe,
    }
}

/// `GET /v1/admin/members?q=&page=`: accounts by pseudo (`q` searches the
/// pseudo and the address), with the role catalogue.
pub async fn list(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let q = params.text("q", 60);
    let page = page(&mut params);
    params.finish()?;
    let pattern = q.as_deref().map(like_pattern);

    let rows: Vec<AccountRow> = sqlx::query_as(sql!(
        "{SELECT} WHERE ($1::text IS NULL OR u.pseudo ILIKE $1 OR u.email ILIKE $1)
          ORDER BY u.pseudo, u.id LIMIT $2 OFFSET $3"
    ))
    .bind(&pattern)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM site.users WHERE $1::text IS NULL OR pseudo ILIKE $1 OR email ILIKE $1",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await?;
    let roles: Vec<CatalogRole> = sqlx::query_as(
        "SELECT slug, nom AS name, couleur AS color, description, categorie
           FROM site.roles ORDER BY niveau DESC, ordre, nom",
    )
    .fetch_all(&state.db)
    .await?;
    let mut catalog: BTreeMap<String, Vec<CatalogRole>> = BTreeMap::new();
    for role in roles {
        catalog
            .entry(role.categorie.clone())
            .or_default()
            .push(role);
    }
    Ok(no_store(
        axum::http::StatusCode::OK,
        &RolesPage {
            listing: Listing {
                data: rows.into_iter().map(to_account).collect(),
                page,
                per_page: PER_PAGE,
                total,
                counters: Empty {},
            },
            catalog,
            can_manage_permissions: admin.level >= LEVEL_SUPER_ADMIN,
        },
    ))
}

/// `PUT /v1/admin/members/{id}/roles/{slug}`: gives the role (nothing
/// changes when the account has it already).
pub async fn grant(
    state: State<AppState>,
    admin: AdminMember,
    path: Path<(i64, String)>,
    uri: OriginalUri,
) -> Result<Response, ApiError> {
    change(state, admin, path, uri, true).await
}

/// `DELETE /v1/admin/members/{id}/roles/{slug}`: takes the role away. 422
/// `own_super_admin` for one's own super admin role.
pub async fn revoke(
    state: State<AppState>,
    admin: AdminMember,
    path: Path<(i64, String)>,
    uri: OriginalUri,
) -> Result<Response, ApiError> {
    change(state, admin, path, uri, false).await
}

async fn change(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Path((user_id, slug)): Path<(i64, String)>,
    OriginalUri(uri): OriginalUri,
    give: bool,
) -> Result<Response, ApiError> {
    let role: Option<(i64, String)> =
        sqlx::query_as("SELECT id, categorie FROM site.roles WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&state.db)
            .await?;
    let Some((role_id, category)) = role else {
        return Err(ApiError::NotFound {
            detail: format!("No role has the slug '{slug}'."),
            instance: Some(uri.path().to_owned()),
        });
    };
    if category == "permission" && admin.level < LEVEL_SUPER_ADMIN {
        return Err(ApiError::Forbidden {
            detail: "Only a super admin gives or takes permission roles.".to_owned(),
            instance: Some(uri.path().to_owned()),
        });
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM site.users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if !exists {
        return Err(not_found("account", user_id, uri.path()));
    }
    if give {
        sqlx::query(
            "INSERT INTO site.role_user (role_id, user_id) VALUES ($1, $2)
             ON CONFLICT (role_id, user_id) DO NOTHING",
        )
        .bind(role_id)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    } else {
        if user_id == admin.id() && slug == "super-admin" {
            return Err(refused(
                "own_super_admin",
                "You cannot take away your own super admin role.",
            ));
        }
        sqlx::query("DELETE FROM site.role_user WHERE role_id = $1 AND user_id = $2")
            .bind(role_id)
            .bind(user_id)
            .execute(&state.db)
            .await?;
    }
    let row: AccountRow = sqlx::query_as(sql!("{SELECT} WHERE u.id = $1"))
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    Ok(ok(to_account(row)))
}
