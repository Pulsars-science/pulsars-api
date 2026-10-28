//! The member's own account settings (`openapi.yaml`, tag `profile`), as
//! Laravel's `ProfilController`, `OnboardingController` and
//! `LangueController`, plus the avatar (Laravel had the column but no
//! upload; the limits are those of a suggestion's image).
//!
//! - `GET` / `PATCH /v1/me/profile`: the profile form. As Laravel's form,
//!   the body is the whole form: a field left out is cleared (except
//!   `brains_public`, left as is).
//! - `POST /v1/me/onboarding`: the welcome questions; sets `onboarding_at`.
//! - `PUT /v1/me/locale`: the language of the account.
//! - `PUT` / `DELETE /v1/me/avatar`: an image (`multipart/form-data`,
//!   field `avatar`), served by `GET /v1/storage/{path}`.
//! - `GET /v1/roles?category=matiere`: the roles a form offers.

use axum::extract::{OriginalUri, Path, RawQuery, Request, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::auth::{check_pseudo, empty};
use super::params::Params;
use super::site::{avatar_url, ok};
use super::upload;
use crate::auth::member::{Member, Verified};
use crate::db::members::{self, MemberRole};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::state::AppState;

/// `Support\Onboarding::objectifs()`.
pub const GOALS: &[&str] = &[
    "reviser-examen",
    "rattraper",
    "approfondir",
    "curiosite",
    "enseigner",
];
/// `Support\Onboarding::niveaux()`.
pub const SCHOOL_LEVELS: &[&str] = &["college", "lycee", "superieur", "adulte"];
/// `config/localisation.php`, `langues`.
pub const LOCALES: &[&str] = &["fr", "en", "ru"];
const CATEGORIES: &[&str] = &["permission", "matiere", "distinction"];

/// An avatar, as a suggestion's image: at most 4 MiB, 6000 px a side.
const AVATAR_MAX_BYTES: usize = 4096 * 1024;
const IMAGE_MAX_SIDE: u32 = 6000;

#[derive(Debug, Serialize)]
pub struct Profile {
    pub pseudo: String,
    pub email: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub goals: Vec<String>,
    pub school_level: Option<String>,
    pub class: Option<String>,
    pub locale: Option<String>,
    pub member_since: Option<String>,
    pub roles: Vec<Role>,
    /// Slugs of the member's subject roles.
    pub interests: Vec<String>,
    /// An account created through Google has none: the form offers to set
    /// one rather than asking for the current one.
    pub has_password: bool,
    pub brains_public: bool,
    pub linked_accounts: Vec<LinkedAccount>,
}

#[derive(Debug, Serialize)]
pub struct Role {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
}

impl From<MemberRole> for Role {
    fn from(r: MemberRole) -> Self {
        Self {
            slug: r.slug,
            name: r.name,
            category: r.category,
            color: r.color,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LinkedAccount {
    pub provider: String,
    pub email: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    pseudo: String,
    email: String,
    bio: Option<String>,
    avatar: Option<String>,
    goals: Option<sqlx::types::Json<serde_json::Value>>,
    school_level: Option<String>,
    class: Option<String>,
    locale: Option<String>,
    member_since: Option<DateTime<Utc>>,
    has_password: bool,
    brains_public: bool,
}

/// `GET /v1/me/profile`.
pub async fn show(
    State(state): State<AppState>,
    Verified(member): Verified,
) -> Result<Response, ApiError> {
    let id = member.id();
    let row: ProfileRow = sqlx::query_as(
        "SELECT pseudo::text, email::text, bio::text, avatar::text, objectif::jsonb AS goals,
                niveau_scolaire::text AS school_level, classe::text AS class, locale::text,
                created_at AT TIME ZONE 'UTC' AS member_since,
                (password IS NOT NULL AND password <> '') AS has_password,
                cerveaux_publics AS brains_public
           FROM site.users WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let roles = members::roles(&state.db, id).await?;
    let linked: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT fournisseur::text, email::text FROM site.comptes_sociaux
          WHERE user_id = $1 ORDER BY fournisseur",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(ok(Profile {
        avatar_url: avatar_url(&state, row.avatar.as_deref()),
        pseudo: row.pseudo,
        email: row.email,
        bio: row.bio,
        goals: row
            .goals
            .and_then(|g| g.0.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect(),
        school_level: row.school_level,
        class: row.class,
        locale: row.locale,
        member_since: row.member_since.map(super::courses::timestamp),
        interests: roles
            .iter()
            .filter(|r| r.category == "matiere")
            .map(|r| r.slug.clone())
            .collect(),
        roles: roles.into_iter().map(Role::from).collect(),
        has_password: row.has_password,
        brains_public: row.brains_public,
        linked_accounts: linked
            .into_iter()
            .map(|(provider, email)| LinkedAccount { provider, email })
            .collect(),
    }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileBody {
    pseudo: Option<String>,
    bio: Option<String>,
    goals: Option<Vec<String>>,
    school_level: Option<String>,
    class: Option<String>,
    interests: Option<Vec<String>>,
    brains_public: Option<bool>,
}

/// Trimmed, empty as absent (Laravel's `TrimStrings` and
/// `ConvertEmptyStringsToNull`).
fn clean(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

fn too_long(errors: &mut Vec<ParameterError>, field: &str, value: Option<&str>, max: usize) {
    if value.is_some_and(|v| v.chars().count() > max) {
        errors.push(ParameterError::field(
            field,
            "too_long",
            format!("must be at most {max} characters"),
        ));
    }
}

/// Goals: known keys, without duplicates, in the order given.
fn check_goals(errors: &mut Vec<ParameterError>, goals: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for (i, goal) in goals.iter().enumerate() {
        if !GOALS.contains(&goal.as_str()) {
            errors.push(ParameterError::field(
                &format!("goals/{i}"),
                "invalid",
                format!("must be one of {}", GOALS.join(", ")),
            ));
        } else if !kept.contains(goal) {
            kept.push(goal.clone());
        }
    }
    kept
}

fn check_level(errors: &mut Vec<ParameterError>, level: Option<&str>) {
    if let Some(level) = level
        && !SCHOOL_LEVELS.contains(&level)
    {
        errors.push(ParameterError::field(
            "school_level",
            "invalid",
            format!("must be one of {}", SCHOOL_LEVELS.join(", ")),
        ));
    }
}

/// `PATCH /v1/me/profile`.
pub async fn update(
    State(state): State<AppState>,
    Verified(member): Verified,
    Json(body): Json<ProfileBody>,
) -> Result<Response, ApiError> {
    let id = member.id();
    let pseudo = clean(body.pseudo).unwrap_or_default();
    let bio = clean(body.bio);
    let class = clean(body.class);
    let level = clean(body.school_level);
    let mut errors = Vec::new();
    if check_pseudo(&pseudo, &mut errors) {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM site.users WHERE lower(pseudo) = lower($1) AND id <> $2)",
        )
        .bind(&pseudo)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if taken {
            errors.push(ParameterError::field(
                "pseudo",
                "taken",
                "is already used by another account",
            ));
        }
    }
    too_long(&mut errors, "bio", bio.as_deref(), 280);
    let goals = check_goals(&mut errors, &body.goals.unwrap_or_default());
    check_level(&mut errors, level.as_deref());
    too_long(&mut errors, "class", class.as_deref(), 60);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }

    let mut tx = state.db.begin().await?;
    let updated = sqlx::query(
        "UPDATE site.users
            SET pseudo = $2, name = $2, bio = $3, objectif = $4, niveau_scolaire = $5,
                classe = $6, cerveaux_publics = coalesce($7, cerveaux_publics),
                updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1",
    )
    .bind(id)
    .bind(&pseudo)
    .bind(&bio)
    .bind(sqlx::types::Json(&goals))
    .bind(&level)
    .bind(&class)
    .bind(body.brains_public)
    .execute(&mut *tx)
    .await;
    if let Err(error) = updated {
        let constraint = error.as_database_error().and_then(|e| e.constraint());
        if constraint == Some("users_pseudo_unique") {
            return Err(ApiError::InvalidFields(vec![ParameterError::field(
                "pseudo",
                "taken",
                "is already used by another account",
            )]));
        }
        return Err(error.into());
    }
    sync_subjects(&mut tx, id, &body.interests.unwrap_or_default(), true).await?;
    tx.commit().await?;
    show(State(state), Verified(member)).await
}

/// The member's subject roles become `slugs`: only roles of the `matiere`
/// category are touched, an unknown slug is ignored (no permission can be
/// granted this way). `replace`: the others are removed (the profile);
/// else they stay (the welcome steps).
async fn sync_subjects(
    tx: &mut sqlx::PgConnection,
    user_id: i64,
    slugs: &[String],
    replace: bool,
) -> Result<(), sqlx::Error> {
    if replace {
        sqlx::query(
            "DELETE FROM site.role_user ru USING site.roles r
              WHERE ru.role_id = r.id AND ru.user_id = $1 AND r.categorie = 'matiere'
                AND NOT (r.slug = ANY($2))",
        )
        .bind(user_id)
        .bind(slugs)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO site.role_user (role_id, user_id)
         SELECT r.id, $1 FROM site.roles r
          WHERE r.categorie = 'matiere' AND r.slug = ANY($2)
            AND NOT EXISTS (SELECT 1 FROM site.role_user ru
                             WHERE ru.role_id = r.id AND ru.user_id = $1)",
    )
    .bind(user_id)
    .bind(slugs)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnboardingBody {
    goals: Option<Vec<String>>,
    school_level: Option<String>,
    class: Option<String>,
    interests: Option<Vec<String>>,
}

/// `POST /v1/me/onboarding`: the two questions that orient the courses are
/// required, the class and interests optional. Replayable, as Laravel's.
pub async fn onboarding(
    State(state): State<AppState>,
    member: Member,
    Json(body): Json<OnboardingBody>,
) -> Result<Response, ApiError> {
    let level = clean(body.school_level);
    let class = clean(body.class);
    let mut errors = Vec::new();
    let goals = match body.goals {
        Some(goals) if !goals.is_empty() => check_goals(&mut errors, &goals),
        _ => {
            errors.push(ParameterError::field(
                "goals",
                "required",
                "choose at least one",
            ));
            Vec::new()
        }
    };
    if level.is_none() {
        errors.push(ParameterError::field(
            "school_level",
            "required",
            "is required",
        ));
    }
    check_level(&mut errors, level.as_deref());
    too_long(&mut errors, "class", class.as_deref(), 60);
    if !errors.is_empty() {
        return Err(ApiError::InvalidFields(errors));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE site.users
            SET objectif = $2, niveau_scolaire = $3, classe = $4,
                onboarding_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1",
    )
    .bind(member.id())
    .bind(sqlx::types::Json(&goals))
    .bind(&level)
    .bind(&class)
    .execute(&mut *tx)
    .await?;
    sync_subjects(
        &mut tx,
        member.id(),
        &body.interests.unwrap_or_default(),
        false,
    )
    .await?;
    tx.commit().await?;
    Ok(empty())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LocaleBody {
    locale: String,
}

/// `PUT /v1/me/locale`: the account follows its member from device to
/// device. A visitor's choice stays in the site's cookie.
pub async fn locale(
    State(state): State<AppState>,
    member: Member,
    Json(body): Json<LocaleBody>,
) -> Result<Response, ApiError> {
    if !LOCALES.contains(&body.locale.as_str()) {
        let code = if body.locale.is_empty() {
            "required"
        } else {
            "invalid"
        };
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "locale",
            code,
            "must be fr, en or ru",
        )]));
    }
    sqlx::query(
        "UPDATE site.users SET locale = $2, updated_at = now() AT TIME ZONE 'UTC' WHERE id = $1",
    )
    .bind(member.id())
    .bind(&body.locale)
    .execute(&state.db)
    .await?;
    Ok(empty())
}

/// `PUT /v1/me/avatar`: a JPEG, PNG, WebP or GIF of at most 4 MiB and
/// 6000 px a side, recognised by its content. The previous one is deleted.
pub async fn put_avatar(
    State(state): State<AppState>,
    Verified(member): Verified,
    request: Request,
) -> Result<Response, ApiError> {
    let form = upload::read(request, &state, AVATAR_MAX_BYTES).await?;
    let field = |code, detail: &str| {
        ApiError::InvalidFields(vec![ParameterError::field("avatar", code, detail)])
    };
    if form.too_large.iter().any(|f| f == "avatar") {
        return Err(field("too_large", "must be at most 4 MiB"));
    }
    let file = form
        .files
        .get("avatar")
        .ok_or_else(|| field("required", "is required"))?;
    let image = upload::image(&file.bytes)
        .ok_or_else(|| field("invalid_image", "must be a JPEG, PNG, WebP or GIF image"))?;
    if image.width > IMAGE_MAX_SIDE || image.height > IMAGE_MAX_SIDE {
        return Err(field(
            "too_large_dimensions",
            "must be at most 6000 pixels a side",
        ));
    }
    let path = format!("avatars/{}.{}", upload::random_name(40)?, image.extension);
    let mut tx = state.db.begin().await?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar::text FROM site.users WHERE id = $1 FOR UPDATE")
            .bind(member.id())
            .fetch_one(&mut *tx)
            .await?;
    upload::store(&mut *tx, "public", &path, &file.bytes, image.mime).await?;
    sqlx::query(
        "UPDATE site.users SET avatar = $2, updated_at = now() AT TIME ZONE 'UTC' WHERE id = $1",
    )
    .bind(member.id())
    .bind(&path)
    .execute(&mut *tx)
    .await?;
    delete_file(&mut tx, old.as_deref()).await?;
    tx.commit().await?;
    Ok(ok(
        serde_json::json!({ "avatar_url": avatar_url(&state, Some(&path)) }),
    ))
}

/// `DELETE /v1/me/avatar`.
pub async fn delete_avatar(
    State(state): State<AppState>,
    Verified(member): Verified,
) -> Result<Response, ApiError> {
    let mut tx = state.db.begin().await?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar::text FROM site.users WHERE id = $1 FOR UPDATE")
            .bind(member.id())
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "UPDATE site.users SET avatar = NULL, updated_at = now() AT TIME ZONE 'UTC' WHERE id = $1",
    )
    .bind(member.id())
    .execute(&mut *tx)
    .await?;
    delete_file(&mut tx, old.as_deref()).await?;
    tx.commit().await?;
    Ok(empty())
}

async fn delete_file(tx: &mut sqlx::PgConnection, path: Option<&str>) -> Result<(), sqlx::Error> {
    if let Some(path) = path.filter(|p| p.starts_with("avatars/")) {
        sqlx::query("DELETE FROM contenu.fichiers WHERE disque = 'public' AND chemin = $1")
            .bind(path)
            .execute(tx)
            .await?;
    }
    Ok(())
}

/// `GET /v1/storage/{*path}`.
pub async fn storage(
    State(state): State<AppState>,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    upload::public_file(&state.db, &path, uri.path()).await
}

/// `GET /v1/roles?category=`: roles by display order (`slug`, `name`,
/// `category`, `color`), e.g. the subjects the forms offer.
pub async fn roles(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let category = params.one_of("category", CATEGORIES);
    params.finish()?;
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT slug::text, nom::text, categorie::text, couleur::text FROM site.roles
          WHERE $1::text IS NULL OR categorie = $1
          ORDER BY niveau DESC, ordre, slug",
    )
    .bind(category)
    .fetch_all(&state.db)
    .await?;
    Ok(ok(rows
        .into_iter()
        .map(|(slug, name, category, color)| Role {
            slug,
            name,
            category,
            color,
        })
        .collect::<Vec<_>>()))
}
