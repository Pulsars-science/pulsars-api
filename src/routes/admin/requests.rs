//! `/v1/admin/resource-requests` (Laravel's
//! `GestionDemandeRessourceController`): the resources members propose.
//! The admin sees everything (who, what, which licence, which credit),
//! reads the PDF, then accepts (which publishes the resource) or refuses
//! with a reason. Nothing is deleted: a request keeps its history.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    DISK, Fields, Listing, conflict, listing, not_found, offset, ok, page, pdf_response,
    unique_slug,
};
use crate::auth::member::AdminMember;
use crate::error::ApiError;
use crate::extract::Json;
use crate::routes::courses::timestamp;
use crate::routes::params::Params;
use crate::routes::resources::{License, license};
use crate::state::AppState;

const PER_PAGE: u32 = 20;
const STATUSES: &[&str] = &["en_attente", "acceptee", "refusee"];

#[derive(Debug, sqlx::FromRow)]
struct Row {
    id: i64,
    titre: String,
    auteur_nom: String,
    licence: String,
    licence_personnalisee: Option<String>,
    license_name: Option<String>,
    license_url: Option<String>,
    description: Option<String>,
    categorie: Option<String>,
    langue: Option<String>,
    taille: i64,
    nom_fichier_original: String,
    statut: String,
    motif: Option<String>,
    account: Option<String>,
    handled_by: Option<String>,
    traite_le: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
    resource_slug: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResourceRequest {
    pub id: i64,
    pub title: String,
    /// Who the resource is credited to.
    pub author_name: String,
    pub license: Option<License>,
    pub description: Option<String>,
    pub category: Option<String>,
    /// The document's language.
    pub lang: Option<String>,
    pub size_bytes: i64,
    pub original_filename: String,
    /// `GET /v1/admin/resource-requests/{id}/file`.
    pub file_url: String,
    /// `en_attente`, `acceptee` or `refusee`.
    pub status: String,
    pub reason: Option<String>,
    /// The member who proposed it.
    pub requested_by: Option<String>,
    pub handled_by: Option<String>,
    pub handled_at: Option<String>,
    pub created_at: Option<String>,
    /// The resource published from it.
    pub resource_slug: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub all: i64,
    pub en_attente: i64,
    pub acceptee: i64,
    pub refusee: i64,
}

const SELECT: &str = "SELECT d.id, d.titre, d.auteur_nom, d.licence, d.licence_personnalisee,
        l.nom AS license_name, l.url AS license_url, d.description, d.categorie, d.langue,
        d.taille, d.nom_fichier_original, d.statut, d.motif, a.pseudo AS account,
        t.pseudo AS handled_by, d.traite_le AT TIME ZONE 'UTC' AS traite_le,
        d.created_at AT TIME ZONE 'UTC' AS created_at, r.slug AS resource_slug
   FROM site.demandes_ressources d
   LEFT JOIN contenu.licences l ON l.cle = d.licence
   LEFT JOIN site.users a ON a.id = d.user_id
   LEFT JOIN site.users t ON t.id = d.traite_par
   LEFT JOIN contenu.ressources r ON r.id = d.ressource_id";

fn to_request(row: Row) -> ResourceRequest {
    ResourceRequest {
        license: license(
            Some(row.licence),
            row.licence_personnalisee,
            row.license_name,
            row.license_url,
        ),
        file_url: format!("/v1/admin/resource-requests/{}/file", row.id),
        id: row.id,
        title: row.titre,
        author_name: row.auteur_nom,
        description: row.description,
        category: row.categorie,
        lang: row.langue,
        size_bytes: row.taille,
        original_filename: row.nom_fichier_original,
        status: row.statut,
        reason: row.motif,
        requested_by: row.account,
        handled_by: row.handled_by,
        handled_at: row.traite_le.map(timestamp),
        created_at: row.created_at.map(timestamp),
        resource_slug: row.resource_slug,
    }
}

async fn find(state: &AppState, id: i64, path: &str) -> Result<ResourceRequest, ApiError> {
    sqlx::query_as::<_, Row>(sql!("{SELECT} WHERE d.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .map(to_request)
        .ok_or_else(|| not_found("resource request", id, path))
}

/// `GET /v1/admin/resource-requests?status=&page=`: newest first.
pub async fn list(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let status = params.one_of("status", STATUSES);
    let page = page(&mut params);
    params.finish()?;

    let rows: Vec<Row> = sqlx::query_as(sql!(
        "{SELECT} WHERE ($1::text IS NULL OR d.statut = $1)
          ORDER BY d.created_at DESC NULLS LAST, d.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind(status)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let (total, all, en_attente, acceptee, refusee): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE $1::text IS NULL OR statut = $1), count(*),
                count(*) FILTER (WHERE statut = 'en_attente'),
                count(*) FILTER (WHERE statut = 'acceptee'),
                count(*) FILTER (WHERE statut = 'refusee')
           FROM site.demandes_ressources",
    )
    .bind(status)
    .fetch_one(&state.db)
    .await?;
    Ok(listing(Listing {
        data: rows.into_iter().map(to_request).collect(),
        page,
        per_page: PER_PAGE,
        total,
        counters: Counts {
            all,
            en_attente,
            acceptee,
            refusee,
        },
    }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AcceptBody {
    /// To refine the title when publishing.
    title: Option<String>,
    category: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct Pending {
    titre: String,
    statut: String,
    fichier: String,
    categorie: Option<String>,
    has_file: bool,
}

async fn pending(state: &AppState, id: i64, path: &str) -> Result<Pending, ApiError> {
    sqlx::query_as(
        "SELECT d.titre, d.statut, d.fichier, d.categorie,
                EXISTS (SELECT 1 FROM contenu.fichiers f WHERE f.disque = $2 AND f.chemin = d.fichier)
                    AS has_file
           FROM site.demandes_ressources d WHERE d.id = $1",
    )
    .bind(id)
    .bind(DISK)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| not_found("resource request", id, path))
}

/// `POST /v1/admin/resource-requests/{id}/accept`: publishes the resource
/// from a copy of the proposed file, with its credit and licence, and links
/// the two. 409 `already_handled` unless the request is pending.
pub async fn accept(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Path(id): Path<i64>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<AcceptBody>,
) -> Result<Response, ApiError> {
    let mut fields = Fields::default();
    let title = fields.optional("title", body.title.as_deref(), 200);
    let category = fields.optional("category", body.category.as_deref(), 120);
    let reason = fields.optional("reason", body.reason.as_deref(), 1000);
    fields.finish()?;

    let request = pending(&state, id, uri.path()).await?;
    if request.statut != "en_attente" {
        return Err(conflict(
            "already_handled",
            "This request has already been handled.",
        ));
    }
    if !request.has_file {
        return Err(ApiError::NotFound {
            detail: format!("The file of resource request {id} is not stored."),
            instance: Some(uri.path().to_owned()),
        });
    }
    let title = title.unwrap_or(request.titre);
    let slug = unique_slug(
        &state.db,
        "contenu.ressources",
        &title,
        Some(60),
        "ressource",
        None,
    )
    .await?;
    // A copy: the request keeps its own file, the two lives stay apart.
    let copy = format!("ressources/{}.pdf", uuid::Uuid::new_v4());

    let mut tx = state.db.begin().await?;
    // Locks the request: two admins accepting at once publish it once.
    let still_pending: Option<String> =
        sqlx::query_scalar("SELECT statut FROM site.demandes_ressources WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if still_pending.as_deref() != Some("en_attente") {
        return Err(conflict(
            "already_handled",
            "This request has already been handled.",
        ));
    }
    sqlx::query(
        "INSERT INTO contenu.fichiers (disque, chemin, contenu, type_mime, taille, created_at, updated_at)
         SELECT disque, $2, contenu, type_mime, taille, date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC')
           FROM contenu.fichiers WHERE disque = $3 AND chemin = $1",
    )
    .bind(&request.fichier)
    .bind(&copy)
    .bind(DISK)
    .execute(&mut *tx)
    .await?;
    let resource_id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.ressources (titre, slug, description, categorie, langue, auteur_nom,
                                         licence, licence_personnalisee, fichier,
                                         nom_fichier_original, taille, statut, ajoute_par,
                                         created_at, updated_at)
         SELECT $2, $3, d.description, $4, d.langue, d.auteur_nom, d.licence,
                d.licence_personnalisee, $5, d.nom_fichier_original, d.taille, 'publie', d.user_id,
                date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC')
           FROM site.demandes_ressources d WHERE d.id = $1
         RETURNING id",
    )
    .bind(id)
    .bind(&title)
    .bind(&slug)
    .bind(category.or(request.categorie))
    .bind(&copy)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE site.demandes_ressources
            SET statut = 'acceptee', traite_par = $2, traite_le = date_trunc('second', now() AT TIME ZONE 'UTC'),
                motif = coalesce($3, motif), ressource_id = $4,
                updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1",
    )
    .bind(id)
    .bind(admin.id())
    .bind(&reason)
    .bind(resource_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ok(find(&state, id, uri.path()).await?))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RejectBody {
    reason: Option<String>,
}

/// `POST /v1/admin/resource-requests/{id}/reject`: with a reason for the
/// member (required). 409 `already_accepted` for an accepted request.
pub async fn reject(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Path(id): Path<i64>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<RejectBody>,
) -> Result<Response, ApiError> {
    let mut fields = Fields::default();
    let reason = fields.text("reason", body.reason.as_deref(), 1, 1000);
    fields.finish()?;

    let request = pending(&state, id, uri.path()).await?;
    if request.statut == "acceptee" {
        return Err(conflict(
            "already_accepted",
            "This request has already been accepted.",
        ));
    }
    sqlx::query(
        "UPDATE site.demandes_ressources
            SET statut = 'refusee', traite_par = $2, traite_le = date_trunc('second', now() AT TIME ZONE 'UTC'),
                motif = $3, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1 AND statut <> 'acceptee'",
    )
    .bind(id)
    .bind(admin.id())
    .bind(&reason)
    .execute(&state.db)
    .await?;
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `GET /v1/admin/resource-requests/{id}/file`: the proposed PDF, shown
/// in the browser, to read before deciding.
pub async fn preview(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(id): Path<i64>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let (path, name): (String, String) = sqlx::query_as(
        "SELECT fichier, nom_fichier_original FROM site.demandes_ressources WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| not_found("resource request", id, uri.path()))?;
    pdf_response(&state.db, &path, &name)
        .await?
        .ok_or_else(|| ApiError::NotFound {
            detail: format!("The file of resource request {id} is not stored."),
            instance: Some(uri.path().to_owned()),
        })
}
