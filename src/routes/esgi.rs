//! Private ESGI courses: subject catalogue, JSON-backed quizzes and
//! flashcards, PDF handouts, and imports by the isolated `admin_esgi` role.

use std::collections::{BTreeMap, HashSet};

use axum::extract::{OriginalUri, Path, Request, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::types::Json as SqlJson;

use super::admin::{self, pdf_response};
use super::auth::no_store;
use super::site::{created, ok};
use super::upload;
use crate::auth::member::{EsgiAdminMember, EsgiMember};
use crate::error::{ApiError, ParameterError};
use crate::state::AppState;

pub const PDF_MAX_BYTES: usize = 20 * 1024 * 1024;
pub const JSON_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuizQuestion {
    pub id: String,
    pub question: String,
    pub options: BTreeMap<String, String>,
    pub answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flashcard {
    pub id: String,
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportCourse {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub quiz: Vec<QuizQuestion>,
    pub flashcards: Vec<Flashcard>,
}

#[derive(Debug, sqlx::FromRow)]
struct SubjectRow {
    slug: String,
    code: String,
    title: String,
    position: i16,
    course_count: i64,
}

#[derive(Debug, Serialize)]
pub struct Subject {
    pub slug: String,
    pub code: String,
    pub title: String,
    pub position: i16,
    pub course_count: i64,
}

impl From<SubjectRow> for Subject {
    fn from(row: SubjectRow) -> Self {
        Self {
            slug: row.slug,
            code: row.code,
            title: row.title,
            position: row.position,
            course_count: row.course_count,
        }
    }
}

const SUBJECT_SELECT: &str = "SELECT m.slug, m.code, m.titre AS title, m.ordre AS position,
        (SELECT count(*) FROM contenu.esgi_cours c WHERE c.matiere_slug = m.slug) AS course_count
    FROM contenu.esgi_matieres m";

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct CourseSummary {
    pub id: i64,
    pub subject_slug: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub question_count: i64,
    pub flashcard_count: i64,
    pub has_pdf: bool,
}

const COURSE_SUMMARY_SELECT: &str = "SELECT c.id, c.matiere_slug AS subject_slug, c.slug,
        c.titre AS title, c.description,
        jsonb_array_length(c.quiz) AS question_count,
        jsonb_array_length(c.flashcards) AS flashcard_count,
        c.fichier_pdf IS NOT NULL AS has_pdf
    FROM contenu.esgi_cours c";

#[derive(Debug, sqlx::FromRow)]
struct CourseRow {
    id: i64,
    subject_slug: String,
    subject_title: String,
    slug: String,
    title: String,
    description: Option<String>,
    quiz: SqlJson<Vec<QuizQuestion>>,
    flashcards: SqlJson<Vec<Flashcard>>,
    has_pdf: bool,
}

#[derive(Debug, Serialize)]
pub struct Course {
    pub id: i64,
    pub subject_slug: String,
    pub subject_title: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub quiz: Vec<QuizQuestion>,
    pub flashcards: Vec<Flashcard>,
    pub pdf_url: Option<String>,
}

impl From<CourseRow> for Course {
    fn from(row: CourseRow) -> Self {
        let pdf_url = row
            .has_pdf
            .then(|| format!("/v1/esgi/courses/{}/pdf", row.slug));
        Self {
            id: row.id,
            subject_slug: row.subject_slug,
            subject_title: row.subject_title,
            slug: row.slug,
            title: row.title,
            description: row.description,
            quiz: row.quiz.0,
            flashcards: row.flashcards.0,
            pdf_url,
        }
    }
}

#[derive(Debug, Serialize)]
struct SubjectCourses {
    subject: Subject,
    courses: Vec<CourseSummary>,
}

#[derive(Debug, Serialize)]
struct AdminCourses {
    subjects: Vec<Subject>,
    courses: Vec<CourseSummary>,
}

async fn subjects(state: &AppState) -> Result<Vec<Subject>, ApiError> {
    let rows: Vec<SubjectRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SUBJECT_SELECT} ORDER BY m.ordre, m.slug"
    )))
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(Subject::from).collect())
}

async fn find_subject(state: &AppState, slug: &str, path: &str) -> Result<Subject, ApiError> {
    sqlx::query_as::<_, SubjectRow>(sqlx::AssertSqlSafe(format!(
        "{SUBJECT_SELECT} WHERE m.slug = $1"
    )))
    .bind(slug)
    .fetch_optional(&state.db)
    .await?
    .map(Subject::from)
    .ok_or_else(|| ApiError::NotFound {
        detail: format!("No ESGI subject has the slug '{slug}'."),
        instance: Some(path.to_owned()),
    })
}

async fn course_summaries(
    state: &AppState,
    subject: Option<&str>,
) -> Result<Vec<CourseSummary>, ApiError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{COURSE_SUMMARY_SELECT}
         WHERE ($1::text IS NULL OR c.matiere_slug = $1)
         ORDER BY c.created_at DESC, c.id DESC"
    )))
    .bind(subject)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

async fn find_course(state: &AppState, slug: &str, path: &str) -> Result<Course, ApiError> {
    let row: Option<CourseRow> = sqlx::query_as(
        "SELECT c.id, c.matiere_slug AS subject_slug, m.titre AS subject_title,
                c.slug, c.titre AS title, c.description, c.quiz, c.flashcards,
                c.fichier_pdf IS NOT NULL AS has_pdf
           FROM contenu.esgi_cours c
           JOIN contenu.esgi_matieres m ON m.slug = c.matiere_slug
          WHERE c.slug = $1",
    )
    .bind(slug)
    .fetch_optional(&state.db)
    .await?;
    row.map(Course::from).ok_or_else(|| ApiError::NotFound {
        detail: format!("No ESGI course has the slug '{slug}'."),
        instance: Some(path.to_owned()),
    })
}

pub async fn list_subjects(
    State(state): State<AppState>,
    EsgiMember(_member): EsgiMember,
) -> Result<Response, ApiError> {
    Ok(ok(subjects(&state).await?))
}

pub async fn show_subject(
    State(state): State<AppState>,
    EsgiMember(_member): EsgiMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let subject = find_subject(&state, &slug, uri.path()).await?;
    let courses = course_summaries(&state, Some(&slug)).await?;
    Ok(ok(SubjectCourses { subject, courses }))
}

pub async fn show_course(
    State(state): State<AppState>,
    EsgiMember(_member): EsgiMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    Ok(ok(find_course(&state, &slug, uri.path()).await?))
}

pub async fn course_pdf(
    State(state): State<AppState>,
    EsgiMember(_member): EsgiMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT fichier_pdf, nom_fichier_original FROM contenu.esgi_cours
          WHERE slug = $1 AND fichier_pdf IS NOT NULL",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?;
    let (path, original) = row.ok_or_else(|| ApiError::NotFound {
        detail: format!("ESGI course '{slug}' has no PDF."),
        instance: Some(uri.path().to_owned()),
    })?;
    pdf_response(
        &state.db,
        &path,
        original.as_deref().unwrap_or("cours-esgi.pdf"),
    )
    .await?
    .ok_or_else(|| ApiError::NotFound {
        detail: "The ESGI PDF is missing from storage.".to_owned(),
        instance: Some(uri.path().to_owned()),
    })
}

pub async fn admin_list(
    State(state): State<AppState>,
    EsgiAdminMember(_admin): EsgiAdminMember,
) -> Result<Response, ApiError> {
    Ok(no_store(
        axum::http::StatusCode::OK,
        &AdminCourses {
            subjects: subjects(&state).await?,
            courses: course_summaries(&state, None).await?,
        },
    ))
}

pub async fn import_course(
    State(state): State<AppState>,
    EsgiAdminMember(admin): EsgiAdminMember,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> Result<Response, ApiError> {
    let mut form = upload::read(request, &state, PDF_MAX_BYTES).await?;
    if !form.too_large.is_empty() {
        return Err(ApiError::InvalidFields(
            form.too_large
                .into_iter()
                .map(|field| ParameterError::field(&field, "too_large", "file is too large"))
                .collect(),
        ));
    }
    let subject = form
        .text("subject")
        .ok_or_else(|| field_error("subject", "required", "Choose the ESGI subject."))?;
    find_subject(&state, &subject, uri.path()).await?;

    let json = form
        .files
        .remove("json")
        .ok_or_else(|| field_error("json", "required", "Choose a JSON file."))?;
    if json.bytes.len() > JSON_MAX_BYTES {
        return Err(field_error(
            "json",
            "too_large",
            "JSON must be at most 2 MiB.",
        ));
    }
    let mut document: ImportCourse = serde_json::from_slice(&json.bytes)
        .map_err(|error| field_error("json", "invalid_json", format!("Invalid JSON: {error}")))?;
    validate_document(&mut document)?;

    let pdf = form.files.remove("pdf");
    if let Some(file) = &pdf
        && !upload::is_pdf(&file.bytes)
    {
        return Err(field_error("pdf", "invalid", "The file must be a PDF."));
    }

    let title = document.title.clone();
    let description = document.description.clone();
    let quiz = SqlJson(document.quiz);
    let flashcards = SqlJson(document.flashcards);
    let slug = admin::unique_slug(
        &state.db,
        "contenu.esgi_cours",
        &title,
        Some(120),
        "cours-esgi",
        None,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    let (pdf_path, original_name) = if let Some(file) = pdf {
        let path = format!("esgi/{}.pdf", upload::random_name(40)?);
        upload::store_file(&mut *tx, &path, &file.bytes, "application/pdf").await?;
        (
            Some(path),
            file.file_name
                .map(|name| name.chars().take(255).collect::<String>()),
        )
    } else {
        (None, None)
    };
    sqlx::query(
        "INSERT INTO contenu.esgi_cours
            (matiere_slug, slug, titre, description, quiz, flashcards,
             fichier_pdf, nom_fichier_original, auteur_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(subject)
    .bind(&slug)
    .bind(title)
    .bind(description)
    .bind(quiz)
    .bind(flashcards)
    .bind(pdf_path)
    .bind(original_name)
    .bind(admin.id())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(created(find_course(&state, &slug, uri.path()).await?))
}

pub async fn delete_course(
    State(state): State<AppState>,
    EsgiAdminMember(_admin): EsgiAdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let mut tx = state.db.begin().await?;
    let deleted: Option<(i64, Option<String>)> =
        sqlx::query_as("DELETE FROM contenu.esgi_cours WHERE slug = $1 RETURNING id, fichier_pdf")
            .bind(&slug)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((_id, path)) = deleted else {
        tx.rollback().await?;
        return Err(ApiError::NotFound {
            detail: format!("No ESGI course has the slug '{slug}'."),
            instance: Some(uri.path().to_owned()),
        });
    };
    if let Some(path) = path {
        sqlx::query("DELETE FROM contenu.fichiers WHERE disque = 'local' AND chemin = $1")
            .bind(path)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(no_store(axum::http::StatusCode::NO_CONTENT, &()))
}

fn field_error(
    field: impl Into<String>,
    code: &'static str,
    detail: impl Into<String>,
) -> ApiError {
    let field = field.into();
    ApiError::InvalidFields(vec![ParameterError::field(&field, code, detail)])
}

fn validate_document(document: &mut ImportCourse) -> Result<(), ApiError> {
    document.title = document.title.trim().to_owned();
    document.description = document
        .description
        .take()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let mut errors = Vec::new();
    check_text(&mut errors, "title", &document.title, 1, 180);
    if let Some(description) = &document.description {
        check_text(&mut errors, "description", description, 0, 1000);
    }
    if document.quiz.len() > 200 {
        errors.push(ParameterError::field(
            "quiz",
            "too_long",
            "at most 200 questions",
        ));
    }
    if document.flashcards.len() > 200 {
        errors.push(ParameterError::field(
            "flashcards",
            "too_long",
            "at most 200 flashcards",
        ));
    }
    let mut question_ids = HashSet::new();
    for (index, question) in document.quiz.iter().enumerate() {
        let prefix = format!("quiz.{index}");
        check_text(&mut errors, &format!("{prefix}.id"), &question.id, 1, 80);
        check_text(
            &mut errors,
            &format!("{prefix}.question"),
            &question.question,
            1,
            2000,
        );
        check_text(
            &mut errors,
            &format!("{prefix}.explanation"),
            &question.explanation,
            1,
            4000,
        );
        if !question_ids.insert(&question.id) {
            errors.push(ParameterError::field(
                &format!("{prefix}.id"),
                "duplicate",
                "question ids must be unique",
            ));
        }
        if !(2..=8).contains(&question.options.len()) {
            errors.push(ParameterError::field(
                &format!("{prefix}.options"),
                "invalid",
                "must contain between 2 and 8 options",
            ));
        }
        if !question.options.contains_key(&question.answer) {
            errors.push(ParameterError::field(
                &format!("{prefix}.answer"),
                "invalid",
                "must name one of the option keys",
            ));
        }
        for (key, value) in &question.options {
            check_text(
                &mut errors,
                &format!("{prefix}.options.{key}"),
                value,
                1,
                1000,
            );
        }
    }
    let mut card_ids = HashSet::new();
    for (index, card) in document.flashcards.iter().enumerate() {
        let prefix = format!("flashcards.{index}");
        check_text(&mut errors, &format!("{prefix}.id"), &card.id, 1, 80);
        check_text(
            &mut errors,
            &format!("{prefix}.front"),
            &card.front,
            1,
            2000,
        );
        check_text(&mut errors, &format!("{prefix}.back"), &card.back, 1, 4000);
        if !card_ids.insert(&card.id) {
            errors.push(ParameterError::field(
                &format!("{prefix}.id"),
                "duplicate",
                "flashcard ids must be unique",
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::InvalidFields(errors))
    }
}

fn check_text(errors: &mut Vec<ParameterError>, field: &str, value: &str, min: usize, max: usize) {
    let count = value.trim().chars().count();
    if count < min {
        errors.push(ParameterError::field(
            field,
            "required",
            "must not be empty",
        ));
    } else if count > max {
        errors.push(ParameterError::field(
            field,
            "too_long",
            format!("must be at most {max} characters"),
        ));
    }
}
