//! `/v1/admin/reports` and `/v1/admin/suggestions` (Laravel's
//! `GestionSignalementController` and `GestionSuggestionController`):
//! what readers report and what members suggest. Each is read, then
//! filed; nothing is deleted, so the history says what was already looked
//! at. Filing one back as new clears who handled it, and when.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Fields, Listing, listing, not_found, offset, ok, page};
use crate::auth::member::AdminMember;
use crate::config::SITE_URL;
use crate::error::ApiError;
use crate::extract::Json;
use crate::links;
use crate::routes::courses::timestamp;
use crate::routes::params::Params;
use crate::state::AppState;

/// `App\Enums\StatutSignalement`.
pub const REPORT_STATUSES: &[&str] = &["nouveau", "retenu", "corrige", "refuse"];
/// `App\Enums\TypeSignalement`.
pub const REPORT_TYPES: &[&str] = &["faute", "amelioration", "spam", "abus", "hors_sujet"];
/// `App\Enums\StatutSuggestion`.
pub const SUGGESTION_STATUSES: &[&str] = &["nouvelle", "retenue", "realisee", "ecartee"];

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatusBody {
    status: Option<String>,
    /// Kept as it was when absent.
    reply: Option<String>,
}

// --- Reports ----------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct ReportRow {
    id: i64,
    #[sqlx(rename = "type")]
    kind: String,
    statut: String,
    message: String,
    emplacement: Option<String>,
    reponse: Option<String>,
    author: Option<String>,
    signalable_type: String,
    signalable_id: i64,
    target_label: Option<String>,
    slug_a: Option<String>,
    slug_b: Option<String>,
    handled_by: Option<String>,
    traite_le: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub id: i64,
    /// `faute`, `amelioration`, `spam`, `abus` or `hors_sujet`.
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub message: String,
    /// Where in the content (a lesson of the course…), as the reader gave it.
    pub location: Option<String>,
    pub reply: Option<String>,
    pub author: Option<String>,
    pub target: Target,
    pub handled_by: Option<String>,
    pub handled_at: Option<String>,
    pub created_at: Option<String>,
}

/// What a report is about.
#[derive(Debug, Serialize)]
pub struct Target {
    /// `course`, `lesson`, `exercise`, `exam`, `question` or `answer`.
    #[serde(rename = "type")]
    pub kind: String,
    pub id: i64,
    /// False once the content is deleted.
    pub exists: bool,
    /// Its title; for an answer, the pseudo of its author (`?` without one).
    pub label: Option<String>,
    /// Its page on the site.
    pub web_url: Option<String>,
    /// What builds that page, those of its type only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lesson_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exercise_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exam_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ReportCounts {
    pub all: i64,
    pub nouveau: i64,
    pub retenu: i64,
    pub corrige: i64,
    pub refuse: i64,
}

const REPORT_SELECT: &str = "SELECT s.id, s.type, s.statut, s.message, s.emplacement, s.reponse,
        a.pseudo AS author, s.signalable_type, s.signalable_id,
        CASE s.signalable_type
            WHEN 'course' THEN (SELECT c.titre FROM contenu.courses c WHERE c.id = s.signalable_id)
            WHEN 'lesson' THEN (SELECT l.titre FROM contenu.lessons l WHERE l.id = s.signalable_id)
            WHEN 'exercise' THEN (SELECT e.titre FROM contenu.exercises e WHERE e.id = s.signalable_id)
            WHEN 'exam' THEN (SELECT x.titre FROM contenu.exams x WHERE x.id = s.signalable_id)
            WHEN 'question' THEN (SELECT q.titre FROM site.questions q
                                   WHERE q.id = s.signalable_id AND q.deleted_at IS NULL)
            WHEN 'answer' THEN (SELECT coalesce(u.pseudo, '?') FROM site.answers r
                                  LEFT JOIN site.users u ON u.id = r.user_id
                                 WHERE r.id = s.signalable_id AND r.deleted_at IS NULL)
        END AS target_label,
        CASE s.signalable_type
            WHEN 'course' THEN (SELECT c.slug FROM contenu.courses c WHERE c.id = s.signalable_id)
            WHEN 'lesson' THEN (SELECT c.slug FROM contenu.lessons l
                                  JOIN contenu.chapters ch ON ch.id = l.chapter_id
                                  JOIN contenu.courses c ON c.id = ch.course_id
                                 WHERE l.id = s.signalable_id)
            WHEN 'exercise' THEN (SELECT c.slug FROM contenu.exercises e
                                    JOIN contenu.chapters ch ON ch.id = e.chapter_id
                                    JOIN contenu.courses c ON c.id = ch.course_id
                                   WHERE e.id = s.signalable_id)
            WHEN 'exam' THEN (SELECT x.slug FROM contenu.exams x WHERE x.id = s.signalable_id)
            WHEN 'question' THEN (SELECT q.slug FROM site.questions q
                                   WHERE q.id = s.signalable_id AND q.deleted_at IS NULL)
            WHEN 'answer' THEN (SELECT q.slug FROM site.answers r
                                  JOIN site.questions q ON q.id = r.question_id
                                 WHERE r.id = s.signalable_id AND r.deleted_at IS NULL)
        END AS slug_a,
        CASE WHEN s.signalable_type = 'lesson'
             THEN (SELECT l.slug FROM contenu.lessons l WHERE l.id = s.signalable_id)
        END AS slug_b,
        t.pseudo AS handled_by, s.traite_le AT TIME ZONE 'UTC' AS traite_le,
        s.created_at AT TIME ZONE 'UTC' AS created_at
   FROM site.signalements s
   LEFT JOIN site.users a ON a.id = s.user_id
   LEFT JOIN site.users t ON t.id = s.traite_par";

/// `https://pulsars.science/forum/q/{slug}`.
fn question_url(slug: &str) -> String {
    format!("{SITE_URL}/forum/q/{}", links::encode(slug, false))
}

fn target(row: &ReportRow) -> Target {
    let id = row.signalable_id;
    let a = row.slug_a.as_deref();
    let web_url = match (row.signalable_type.as_str(), a, row.slug_b.as_deref()) {
        ("course", Some(course), _) => Some(links::course(course)),
        ("lesson", Some(course), Some(lesson)) => Some(links::lesson(course, lesson)),
        ("exercise", Some(course), _) => Some(links::exercise(course, id)),
        ("exam", Some(exam), _) => Some(links::exam(exam)),
        ("question", Some(question), _) => Some(question_url(question)),
        ("answer", Some(question), _) => Some(format!("{}#reponse-{id}", question_url(question))),
        _ => None,
    };
    let kind = row.signalable_type.as_str();
    let slug = |of: &[&str]| row.slug_a.clone().filter(|_| of.contains(&kind));
    let present = row.target_label.is_some();
    Target {
        kind: row.signalable_type.clone(),
        id,
        exists: present,
        label: row.target_label.clone(),
        web_url,
        course_slug: slug(&["course", "lesson", "exercise"]),
        lesson_slug: row.slug_b.clone().filter(|_| kind == "lesson"),
        exercise_id: (kind == "exercise" && present).then_some(id),
        exam_slug: slug(&["exam"]),
        question_slug: slug(&["question", "answer"]),
        answer_id: (kind == "answer" && present).then_some(id),
    }
}

fn to_report(row: ReportRow) -> Report {
    Report {
        target: target(&row),
        id: row.id,
        kind: row.kind,
        status: row.statut,
        message: row.message,
        location: row.emplacement,
        reply: row.reponse,
        author: row.author,
        handled_by: row.handled_by,
        handled_at: row.traite_le.map(timestamp),
        created_at: row.created_at.map(timestamp),
    }
}

/// `GET /v1/admin/reports?status=&type=&page=`: newest first.
pub async fn reports(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    const PER_PAGE: u32 = 25;
    let mut params = Params::parse(query.as_deref());
    let status = params.one_of("status", REPORT_STATUSES);
    let kind = params.one_of("type", REPORT_TYPES);
    let page = page(&mut params);
    params.finish()?;

    let rows: Vec<ReportRow> = sqlx::query_as(sql!(
        "{REPORT_SELECT} WHERE ($1::text IS NULL OR s.statut = $1) AND ($2::text IS NULL OR s.type = $2)
          ORDER BY s.created_at DESC NULLS LAST, s.id DESC LIMIT $3 OFFSET $4"
    ))
    .bind(status)
    .bind(kind)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let (total, all, nouveau, retenu, corrige, refuse): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT count(*) FILTER (WHERE ($1::text IS NULL OR statut = $1)
                                       AND ($2::text IS NULL OR type = $2)),
                    count(*),
                    count(*) FILTER (WHERE statut = 'nouveau'),
                    count(*) FILTER (WHERE statut = 'retenu'),
                    count(*) FILTER (WHERE statut = 'corrige'),
                    count(*) FILTER (WHERE statut = 'refuse')
               FROM site.signalements",
        )
        .bind(status)
        .bind(kind)
        .fetch_one(&state.db)
        .await?;
    Ok(listing(Listing {
        data: rows.into_iter().map(to_report).collect(),
        page,
        per_page: PER_PAGE,
        total,
        counters: ReportCounts {
            all,
            nouveau,
            retenu,
            corrige,
            refuse,
        },
    }))
}

/// Sets the status of row `id` of `table`; `reopened` is the status that
/// puts it back on the pile.
async fn file(
    state: &AppState,
    table: &'static str,
    id: i64,
    admin: i64,
    body: &StatusBody,
    statuses: &[&str],
    reopened: &str,
) -> Result<bool, ApiError> {
    let mut fields = Fields::default();
    let status = fields.one_of("status", body.status.as_deref(), statuses);
    let reply = fields.optional("reply", body.reply.as_deref(), 1000);
    fields.finish()?;
    let reopen = status == reopened;
    let done = sqlx::query(sql!(
        "UPDATE {table}
            SET statut = $2, reponse = coalesce($3, reponse),
                traite_par = CASE WHEN $4 THEN NULL ELSE $5 END,
                traite_le = CASE WHEN $4 THEN NULL ELSE date_trunc('second', now() AT TIME ZONE 'UTC') END,
                updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1"
    ))
    .bind(id)
    .bind(&status)
    .bind(&reply)
    .bind(reopen)
    .bind(admin)
    .execute(&state.db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// `PATCH /v1/admin/reports/{id}`: `{"status": …, "reply": …}`.
pub async fn file_report(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Path(id): Path<i64>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<StatusBody>,
) -> Result<Response, ApiError> {
    if !file(
        &state,
        "site.signalements",
        id,
        admin.id(),
        &body,
        REPORT_STATUSES,
        "nouveau",
    )
    .await?
    {
        return Err(not_found("report", id, uri.path()));
    }
    let row: ReportRow = sqlx::query_as(sql!("{REPORT_SELECT} WHERE s.id = $1"))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(ok(to_report(row)))
}

// --- Suggestions ------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct SuggestionRow {
    id: i64,
    titre: Option<String>,
    message: String,
    image: Option<String>,
    statut: String,
    reponse: Option<String>,
    author: Option<String>,
    handled_by: Option<String>,
    traite_le: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub id: i64,
    pub title: Option<String>,
    pub message: String,
    /// The attached picture, on the public storage.
    pub image_url: Option<String>,
    pub status: String,
    pub reply: Option<String>,
    pub author: Option<String>,
    pub handled_by: Option<String>,
    pub handled_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SuggestionCounts {
    pub all: i64,
    pub nouvelle: i64,
    pub retenue: i64,
    pub realisee: i64,
    pub ecartee: i64,
}

const SUGGESTION_SELECT: &str = "SELECT s.id, s.titre, s.message, s.image, s.statut, s.reponse,
        a.pseudo AS author, t.pseudo AS handled_by,
        s.traite_le AT TIME ZONE 'UTC' AS traite_le, s.created_at AT TIME ZONE 'UTC' AS created_at
   FROM site.suggestions s
   LEFT JOIN site.users a ON a.id = s.user_id
   LEFT JOIN site.users t ON t.id = s.traite_par";

fn to_suggestion(row: SuggestionRow, storage: &str) -> Suggestion {
    Suggestion {
        image_url: row
            .image
            .as_deref()
            .map(|path| format!("{storage}/{}", links::encode(path, true))),
        id: row.id,
        title: row.titre,
        message: row.message,
        status: row.statut,
        reply: row.reponse,
        author: row.author,
        handled_by: row.handled_by,
        handled_at: row.traite_le.map(timestamp),
        created_at: row.created_at.map(timestamp),
    }
}

/// `GET /v1/admin/suggestions?status=&page=`: newest first.
pub async fn suggestions(
    State(state): State<AppState>,
    _admin: AdminMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    const PER_PAGE: u32 = 20;
    let mut params = Params::parse(query.as_deref());
    let status = params.one_of("status", SUGGESTION_STATUSES);
    let page = page(&mut params);
    params.finish()?;

    let rows: Vec<SuggestionRow> = sqlx::query_as(sql!(
        "{SUGGESTION_SELECT} WHERE ($1::text IS NULL OR s.statut = $1)
          ORDER BY s.created_at DESC NULLS LAST, s.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind(status)
    .bind(i64::from(PER_PAGE))
    .bind(offset(page, PER_PAGE))
    .fetch_all(&state.db)
    .await?;
    let (total, all, nouvelle, retenue, realisee, ecartee): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT count(*) FILTER (WHERE $1::text IS NULL OR statut = $1), count(*),
                    count(*) FILTER (WHERE statut = 'nouvelle'),
                    count(*) FILTER (WHERE statut = 'retenue'),
                    count(*) FILTER (WHERE statut = 'realisee'),
                    count(*) FILTER (WHERE statut = 'ecartee')
               FROM site.suggestions",
        )
        .bind(status)
        .fetch_one(&state.db)
        .await?;
    let storage = state.config.public_storage_url.as_str();
    Ok(listing(Listing {
        data: rows
            .into_iter()
            .map(|row| to_suggestion(row, storage))
            .collect(),
        page,
        per_page: PER_PAGE,
        total,
        counters: SuggestionCounts {
            all,
            nouvelle,
            retenue,
            realisee,
            ecartee,
        },
    }))
}

/// `PATCH /v1/admin/suggestions/{id}`: `{"status": …, "reply": …}`.
pub async fn file_suggestion(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    Path(id): Path<i64>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<StatusBody>,
) -> Result<Response, ApiError> {
    if !file(
        &state,
        "site.suggestions",
        id,
        admin.id(),
        &body,
        SUGGESTION_STATUSES,
        "nouvelle",
    )
    .await?
    {
        return Err(not_found("suggestion", id, uri.path()));
    }
    let row: SuggestionRow = sqlx::query_as(sql!("{SUGGESTION_SELECT} WHERE s.id = $1"))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(ok(to_suggestion(
        row,
        state.config.public_storage_url.as_str(),
    )))
}
