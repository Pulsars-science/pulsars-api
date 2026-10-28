//! `/v1/admin/news` (Laravel's `GestionActualiteController`): the
//! announcements of the Actualité page. One is written as a draft, then
//! published: `published_at` decides everything (null: invisible; set: shown
//! from that date). Unpublishing clears the date and keeps the text.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Fields, Listing, created, listing, not_found, offset, ok, page, unique_slug};
use crate::auth::member::AdminMember;
use crate::error::ApiError;
use crate::extract::Json;
use crate::links;
use crate::routes::auth::empty;
use crate::routes::courses::timestamp;
use crate::routes::params::{Params, like_pattern};
use crate::state::AppState;

const PER_PAGE: u32 = 25;

#[derive(Debug, sqlx::FromRow)]
struct Row {
    id: i64,
    titre: String,
    slug: String,
    chapeau: Option<String>,
    contenu_md: String,
    published_at: Option<DateTime<Utc>>,
    author: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminNews {
    pub id: i64,
    pub title: String,
    pub slug: String,
    /// The lead paragraph (`chapeau`).
    pub lead: Option<String>,
    pub content_md: String,
    /// Published and its date reached.
    pub published: bool,
    pub published_at: Option<String>,
    pub author: Option<String>,
    pub updated_at: Option<String>,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub all: i64,
    pub published: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewsBody {
    title: Option<String>,
    lead: Option<String>,
    content_md: Option<String>,
}

const SELECT: &str = "SELECT a.id, a.titre, a.slug, a.chapeau, a.contenu_md,
        a.published_at AT TIME ZONE 'UTC' AS published_at, u.pseudo AS author,
        a.updated_at AT TIME ZONE 'UTC' AS updated_at
   FROM contenu.actualites a
   LEFT JOIN site.users u ON u.id = a.auteur_id";

fn to_news(row: Row) -> AdminNews {
    AdminNews {
        published: row.published_at.is_some_and(|at| at <= Utc::now()),
        published_at: row.published_at.map(timestamp),
        web_url: links::news(&row.slug),
        id: row.id,
        title: row.titre,
        slug: row.slug,
        lead: row.chapeau,
        content_md: row.contenu_md,
        author: row.author,
        updated_at: row.updated_at.map(timestamp),
    }
}

async fn find(state: &AppState, id: i64, path: &str) -> Result<AdminNews, ApiError> {
    sqlx::query_as::<_, Row>(sql!("{SELECT} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .map(to_news)
        .ok_or_else(|| not_found("news item", id, path))
}

/// `GET /v1/admin/news?q=&page=`: every announcement, last changed first.
pub async fn list(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let q = params.text("q", 100);
    let page = page(&mut params);
    params.finish()?;
    let pattern = q.as_deref().map(like_pattern);

    let rows: Vec<Row> = sqlx::query_as(sql!(
        "{SELECT} WHERE ($1::text IS NULL OR a.titre ILIKE $1)
          ORDER BY a.updated_at DESC NULLS LAST, a.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind(&pattern)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let (total, all, published): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE $1::text IS NULL OR titre ILIKE $1),
                count(*),
                count(*) FILTER (WHERE published_at <= date_trunc('second', now() AT TIME ZONE 'UTC'))
           FROM contenu.actualites",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await?;
    Ok(listing(Listing {
        data: rows.into_iter().map(to_news).collect(),
        page,
        per_page: PER_PAGE,
        total,
        counters: Counts { all, published },
    }))
}

fn check(body: &NewsBody) -> Result<(String, Option<String>, String), ApiError> {
    let mut fields = Fields::default();
    let title = fields.text("title", body.title.as_deref(), 5, 150);
    let summary = fields.optional("lead", body.lead.as_deref(), 500);
    let content = fields.text("content_md", body.content_md.as_deref(), 20, 20_000);
    fields.finish()?;
    Ok((title, summary, content))
}

/// `POST /v1/admin/news`: always a draft; publishing is a separate step.
pub async fn create(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<NewsBody>,
) -> Result<Response, ApiError> {
    let (title, summary, content) = check(&body)?;
    let slug = unique_slug(
        &state.db,
        "contenu.actualites",
        &title,
        Some(60),
        "actualite",
        None,
    )
    .await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.actualites (titre, slug, chapeau, contenu_md, auteur_id, published_at,
                                         created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, NULL, date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC'))
         RETURNING id",
    )
    .bind(&title)
    .bind(&slug)
    .bind(&summary)
    .bind(&content)
    .bind(admin.id())
    .fetch_one(&state.db)
    .await?;
    Ok(created(find(&state, id, uri.path()).await?))
}

/// `PUT /v1/admin/news/{slug}`: the text. The slug never changes: it is
/// already in shared links.
pub async fn update(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<NewsBody>,
) -> Result<Response, ApiError> {
    let id = super::resolve(
        &state.db,
        "contenu.actualites",
        "news item",
        &slug,
        uri.path(),
    )
    .await?;
    let (title, summary, content) = check(&body)?;
    let done = sqlx::query(
        "UPDATE contenu.actualites
            SET titre = $2, chapeau = $3, contenu_md = $4, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1",
    )
    .bind(id)
    .bind(&title)
    .bind(&summary)
    .bind(&content)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(not_found("news item", id, uri.path()));
    }
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/news/{slug}/publish`: keeps a date already set (a
/// scheduled announcement stays scheduled).
pub async fn publish(
    state: State<AppState>,
    admin: AdminMember,
    id: Path<String>,
    uri: OriginalUri,
) -> Result<Response, ApiError> {
    set_date(
        state,
        admin,
        id,
        uri,
        "coalesce(published_at, date_trunc('second', now() AT TIME ZONE 'UTC'))",
    )
    .await
}

/// `POST /v1/admin/news/{slug}/unpublish`.
pub async fn unpublish(
    state: State<AppState>,
    admin: AdminMember,
    id: Path<String>,
    uri: OriginalUri,
) -> Result<Response, ApiError> {
    set_date(state, admin, id, uri, "NULL").await
}

async fn set_date(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    value: &'static str,
) -> Result<Response, ApiError> {
    let id = super::resolve(
        &state.db,
        "contenu.actualites",
        "news item",
        &slug,
        uri.path(),
    )
    .await?;
    let done = sqlx::query(sql!(
        "UPDATE contenu.actualites SET published_at = {value}, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1"
    ))
    .bind(id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(not_found("news item", id, uri.path()));
    }
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `DELETE /v1/admin/news/{slug}`.
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(
        &state.db,
        "contenu.actualites",
        "news item",
        &slug,
        uri.path(),
    )
    .await?;
    let done = sqlx::query("DELETE FROM contenu.actualites WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found("news item", id, uri.path()));
    }
    Ok(empty())
}
