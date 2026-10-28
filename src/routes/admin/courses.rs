//! `/v1/admin/courses` (Laravel's `GestionCoursController`): a course is
//! created as a draft from a short form (its chapters and lessons come
//! later), then published. Nothing reaches the library before an admin
//! publishes it, and a course without lessons cannot be.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    Fields, Listing, conflict, created, not_found, offset, ok, page, refused, unique_slug,
};
use crate::auth::member::AdminMember;
use crate::error::ApiError;
use crate::extract::Json;
use crate::links;
use crate::routes::auth::{empty, no_store};
use crate::routes::courses::timestamp;
use crate::routes::params::{Params, like_pattern};
use crate::state::AppState;

const PER_PAGE: u32 = 25;
const STATUSES: &[&str] = &["brouillon", "publie", "archive"];
/// `App\Enums\Difficulte`.
pub const DIFFICULTIES: &[&str] = &["debutant", "intermediaire", "avance"];

#[derive(Debug, sqlx::FromRow)]
struct Row {
    id: i64,
    titre: String,
    slug: String,
    theme: String,
    statut: String,
    difficulte: String,
    author: Option<String>,
    chapters: i64,
    lessons: i64,
    exercises: i64,
    enrollments: i64,
    published_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminCourse {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub theme: String,
    /// `brouillon`, `publie` or `archive`.
    pub status: String,
    pub difficulty: String,
    pub author: Option<String>,
    pub chapter_count: i64,
    pub lesson_count: i64,
    pub exercise_count: i64,
    pub enrollment_count: i64,
    /// A course without lessons cannot be published.
    pub publishable: bool,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub all: i64,
    pub brouillon: i64,
    pub publie: i64,
    pub archive: i64,
}

#[derive(Debug, Serialize)]
struct CourseListing {
    #[serde(flatten)]
    listing: Listing<AdminCourse, Counts>,
    difficulties: &'static [&'static str],
    /// Every theme in use, for the creation form.
    themes: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CourseBody {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    difficulty: Option<String>,
    duration_minutes: Option<i64>,
}

const SELECT: &str = "SELECT c.id, c.titre, c.slug, c.theme, c.statut, c.difficulte,
        u.pseudo AS author,
        (SELECT count(*) FROM contenu.chapters ch WHERE ch.course_id = c.id) AS chapters,
        (SELECT count(*) FROM contenu.lessons l JOIN contenu.chapters ch ON ch.id = l.chapter_id
          WHERE ch.course_id = c.id) AS lessons,
        (SELECT count(*) FROM contenu.exercises e JOIN contenu.chapters ch ON ch.id = e.chapter_id
          WHERE ch.course_id = c.id) AS exercises,
        (SELECT count(*) FROM site.enrollments en WHERE en.course_id = c.id) AS enrollments,
        c.published_at AT TIME ZONE 'UTC' AS published_at,
        c.updated_at AT TIME ZONE 'UTC' AS updated_at
   FROM contenu.courses c
   LEFT JOIN site.users u ON u.id = c.auteur_id";

fn to_course(row: Row) -> AdminCourse {
    AdminCourse {
        publishable: row.lessons > 0,
        web_url: links::course(&row.slug),
        published_at: row.published_at.map(timestamp),
        updated_at: row.updated_at.map(timestamp),
        id: row.id,
        title: row.titre,
        slug: row.slug,
        theme: row.theme,
        status: row.statut,
        difficulty: row.difficulte,
        author: row.author,
        chapter_count: row.chapters,
        lesson_count: row.lessons,
        exercise_count: row.exercises,
        enrollment_count: row.enrollments,
    }
}

async fn find(state: &AppState, id: i64, path: &str) -> Result<AdminCourse, ApiError> {
    sqlx::query_as::<_, Row>(sql!("{SELECT} WHERE c.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .map(to_course)
        .ok_or_else(|| not_found("course", id, path))
}

/// `GET /v1/admin/courses?status=&q=&page=`: every course, last changed first.
pub async fn list(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let status = params.one_of("status", STATUSES);
    let q = params.text("q", 100);
    let page = page(&mut params);
    params.finish()?;
    let pattern = q.as_deref().map(like_pattern);

    let rows: Vec<Row> = sqlx::query_as(sql!(
        "{SELECT} WHERE ($1::text IS NULL OR c.statut = $1) AND ($2::text IS NULL OR c.titre ILIKE $2)
          ORDER BY c.updated_at DESC NULLS LAST, c.id DESC LIMIT $3 OFFSET $4"
    ))
    .bind(status)
    .bind(&pattern)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let (total, all, brouillon, publie, archive): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE ($1::text IS NULL OR statut = $1)
                                   AND ($2::text IS NULL OR titre ILIKE $2)),
                count(*),
                count(*) FILTER (WHERE statut = 'brouillon'),
                count(*) FILTER (WHERE statut = 'publie'),
                count(*) FILTER (WHERE statut = 'archive')
           FROM contenu.courses",
    )
    .bind(status)
    .bind(&pattern)
    .fetch_one(&state.db)
    .await?;
    let themes: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT theme FROM contenu.courses ORDER BY theme")
            .fetch_all(&state.db)
            .await?;
    Ok(no_store(
        axum::http::StatusCode::OK,
        &CourseListing {
            listing: Listing {
                data: rows.into_iter().map(to_course).collect(),
                page,
                per_page: PER_PAGE,
                total,
                counters: Counts {
                    all,
                    brouillon,
                    publie,
                    archive,
                },
            },
            difficulties: DIFFICULTIES,
            themes,
        },
    ))
}

/// `POST /v1/admin/courses`: always a draft, never published empty.
pub async fn create(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<CourseBody>,
) -> Result<Response, ApiError> {
    let mut fields = Fields::default();
    let title = fields.text("title", body.title.as_deref(), 5, 150);
    let description = fields.text("description", body.description.as_deref(), 20, 500);
    let theme = fields.text("theme", body.theme.as_deref(), 1, 100);
    let difficulty = fields.one_of("difficulty", body.difficulty.as_deref(), DIFFICULTIES);
    if let Some(minutes) = body.duration_minutes
        && !(5..=600).contains(&minutes)
    {
        fields.push(
            "duration_minutes",
            if minutes < 5 {
                "too_small"
            } else {
                "too_large"
            },
            "must be between 5 and 600",
        );
    }
    fields.finish()?;

    let slug = unique_slug(
        &state.db,
        "contenu.courses",
        &title,
        Some(60),
        "cours",
        None,
    )
    .await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.courses (titre, slug, description, theme, difficulte, duree_estimee,
                                      auteur_id, statut, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6::smallint, $7, 'brouillon',
                 date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC'))
         RETURNING id",
    )
    .bind(&title)
    .bind(&slug)
    .bind(&description)
    .bind(&theme)
    .bind(&difficulty)
    .bind(body.duration_minutes.map(|m| m as i32))
    .bind(admin.id())
    .fetch_one(&state.db)
    .await?;
    Ok(created(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/courses/{slug}/publish`: 422 `no_lessons` for a course
/// without lessons. The first publication date is kept.
pub async fn publish(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.courses", "course", &slug, uri.path()).await?;
    let course = find(&state, id, uri.path()).await?;
    if course.lesson_count == 0 {
        return Err(refused(
            "no_lessons",
            "This course has no lesson: it cannot be published.",
        ));
    }
    set(
        &state,
        id,
        "statut = 'publie', published_at = coalesce(published_at, date_trunc('second', now() AT TIME ZONE 'UTC'))",
    )
    .await?;
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/courses/{slug}/unpublish`: back to draft.
pub async fn unpublish(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.courses", "course", &slug, uri.path()).await?;
    if !set(&state, id, "statut = 'brouillon'").await? {
        return Err(not_found("course", id, uri.path()));
    }
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/courses/{slug}/archive`: out of the library, the members'
/// progress kept.
pub async fn archive(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.courses", "course", &slug, uri.path()).await?;
    if !set(&state, id, "statut = 'archive'").await? {
        return Err(not_found("course", id, uri.path()));
    }
    Ok(ok(find(&state, id, uri.path()).await?))
}

async fn set(state: &AppState, id: i64, assignments: &'static str) -> Result<bool, ApiError> {
    let done = sqlx::query(sql!(
        "UPDATE contenu.courses SET {assignments}, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1"
    ))
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// `DELETE /v1/admin/courses/{slug}`: 409 `has_enrollments` when members
/// follow it (deleting would take their progress: archive it instead).
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.courses", "course", &slug, uri.path()).await?;
    let course = find(&state, id, uri.path()).await?;
    if course.enrollment_count > 0 {
        return Err(conflict(
            "has_enrollments",
            "Members follow this course: archive it rather than delete it.",
        ));
    }
    sqlx::query("DELETE FROM contenu.courses WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(empty())
}
