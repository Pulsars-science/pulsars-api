//! `/v1/admin/exams` (Laravel's `Admin\ExamenController` and
//! `ImporteurReponseExamen`): exam papers turned into structured content.
//!
//! 1. `POST /v1/admin/exams` (multipart): the admin uploads the PDF; the
//!    exam waits (`attente_import`) with a prompt to copy.
//! 2. The admin sends the prompt and the PDF to an AI of their choice,
//!    outside the site. Nothing here calls an AI.
//! 3. `POST /v1/admin/exams/{slug}/import`: the admin pastes the AI's JSON
//!    answer; the exam becomes a draft (`brouillon`) with its parts.
//! 4. `POST /v1/admin/exams/{slug}/publish` once proofread.

use axum::extract::{OriginalUri, Path, State};
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    DISK, Fields, Form, created, multipart_error, not_found, ok, pdf_response, random_name,
    refused, store_file, unique_slug,
};
use crate::auth::member::AdminMember;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::links;
use crate::routes::auth::{empty, no_store};
use crate::routes::courses::timestamp;
use crate::state::AppState;

/// `max:20480` (kilobytes) on the PDF.
pub const PDF_MAX_BYTES: usize = 20 * 1024 * 1024;
/// The upload route's body limit: the PDF and the three text fields.
pub const UPLOAD_BODY_LIMIT: usize = PDF_MAX_BYTES + 1024 * 1024;
/// `max:2000000` on the pasted answer, in characters.
pub const ANSWER_MAX_CHARS: usize = 2_000_000;
/// The import route's body limit: the answer JSON-escaped in its body.
pub const IMPORT_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// `App\Enums\MatiereExamen`, with the label the prompt names.
pub const SUBJECTS: &[(&str, &str)] = &[
    ("mathematiques", "Mathématiques"),
    ("physique", "Physique-Chimie"),
    ("chimie", "Chimie"),
    ("informatique", "Informatique"),
    ("sciences-industrielles", "Sciences industrielles"),
    ("langues", "Langues vivantes"),
    ("redaction", "Rédaction"),
];

/// `config/pays.php`: the school system a paper comes from.
pub const COUNTRIES: &[&str] = &[
    "FR", "BE", "CH", "CA", "LU", "MA", "TN", "DZ", "SN", "CI", "GB", "US",
];

const TYPES: &[&str] = &["brevet", "bac", "blanc"];

#[derive(Debug, sqlx::FromRow)]
struct Row {
    id: i64,
    titre: String,
    slug: String,
    matiere: String,
    pays: String,
    statut: String,
    parts: i64,
    fichier_pdf: Option<String>,
    nom_fichier_original: Option<String>,
    prompt_ia: Option<String>,
    erreur_ia: Option<String>,
    analyse_at: Option<DateTime<Utc>>,
    published_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AdminExam {
    pub id: i64,
    pub title: String,
    pub slug: String,
    /// `App\Enums\MatiereExamen` value, and its French label.
    pub subject: String,
    pub subject_label: String,
    pub country: String,
    /// `attente_import`, `brouillon` or `publie`.
    pub status: String,
    pub part_count: i64,
    pub original_filename: Option<String>,
    /// The admin's own instructions, added to the prompt.
    pub ai_prompt: Option<String>,
    /// The whole prompt to send to the AI with the PDF.
    pub import_prompt: String,
    pub ai_error: Option<String>,
    pub analyzed_at: Option<String>,
    pub published_at: Option<String>,
    pub created_at: Option<String>,
    /// The page of the exam on the site (a draft is shown to admins only).
    pub web_url: String,
    /// `GET /v1/admin/exams/{slug}/pdf`, null without a PDF.
    pub pdf_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExamListing {
    data: Vec<AdminExam>,
    subjects: Vec<Choice>,
    countries: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct Choice {
    value: &'static str,
    label: &'static str,
}

const SELECT: &str = "SELECT x.id, x.titre, x.slug, x.matiere, x.pays, x.statut,
        (SELECT count(*) FROM contenu.exam_parties p WHERE p.exam_id = x.id) AS parts,
        x.fichier_pdf, x.nom_fichier_original, x.prompt_ia, x.erreur_ia,
        x.analyse_at AT TIME ZONE 'UTC' AS analyse_at,
        x.published_at AT TIME ZONE 'UTC' AS published_at,
        x.created_at AT TIME ZONE 'UTC' AS created_at
   FROM contenu.exams x";

fn subject_label(subject: &str) -> &str {
    SUBJECTS
        .iter()
        .find(|(value, _)| *value == subject)
        .map_or(subject, |(_, label)| label)
}

fn to_exam(row: Row) -> AdminExam {
    AdminExam {
        import_prompt: prompt(subject_label(&row.matiere), row.prompt_ia.as_deref()),
        subject_label: subject_label(&row.matiere).to_owned(),
        web_url: links::exam(&row.slug),
        pdf_url: row
            .fichier_pdf
            .as_ref()
            .map(|_| format!("/v1/admin/exams/{}/pdf", links::encode(&row.slug, false))),
        id: row.id,
        title: row.titre,
        slug: row.slug,
        subject: row.matiere,
        country: row.pays,
        status: row.statut,
        part_count: row.parts,
        original_filename: row.nom_fichier_original,
        ai_prompt: row.prompt_ia,
        ai_error: row.erreur_ia,
        analyzed_at: row.analyse_at.map(timestamp),
        published_at: row.published_at.map(timestamp),
        created_at: row.created_at.map(timestamp),
    }
}

async fn find_row(state: &AppState, id: i64, path: &str) -> Result<Row, ApiError> {
    sqlx::query_as::<_, Row>(sql!("{SELECT} WHERE x.id = $1"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| not_found("exam", id, path))
}

async fn find(state: &AppState, id: i64, path: &str) -> Result<AdminExam, ApiError> {
    find_row(state, id, path).await.map(to_exam)
}

/// The prompt of `ImporteurReponseExamen::prompt()`, word for word.
pub fn prompt(subject: &str, extra: Option<&str>) -> String {
    let extra = extra.map(str::trim).unwrap_or_default();
    let instructions = if extra.is_empty() {
        String::new()
    } else {
        format!("\nInstructions complémentaires :\n{extra}\n")
    };
    format!(
        "Je joins à ce message un sujet d’examen au format PDF.

Analyse ce PDF et transforme-le en contenu pédagogique structuré pour Pulsars.
Matière attendue : {subject}.
{instructions}
Règles impératives :
- retranscris fidèlement toutes les questions visibles, sans en inventer ;
- conserve les données, unités, notations, barèmes et sous-questions ;
- rédige pour chaque exercice un corrigé détaillé, juste et compréhensible ;
- indique clairement dans le corrigé toute donnée illisible ou manquante ;
- utilise du Markdown simple dans les champs enonce_md et corrige_md, sans HTML ;
- écris en français correct avec les accents ;
- pour le champ type, choisis exactement une valeur parmi brevet, bac ou blanc ;
- réponds uniquement avec un objet JSON valide, sans bloc ```json, sans préambule et sans commentaire après le JSON.

Respecte exactement cette structure :
{{
  \"titre\": \"Titre complet du sujet\",
  \"type\": \"bac\",
  \"niveau\": \"3e ou Terminale\",
  \"session\": \"année ou session, ou null\",
  \"duree_minutes\": 240,
  \"points_total\": 20,
  \"description\": \"Courte description du sujet, ou null\",
  \"parties\": [
    {{
      \"titre\": \"Exercice 1 — titre descriptif\",
      \"points\": 5,
      \"enonce_md\": \"Énoncé complet avec toutes les sous-questions\",
      \"corrige_md\": \"Correction détaillée, étape par étape\"
    }}
  ]
}}

Utilise null lorsque la session, la durée, le total de points ou le barème d’un exercice ne figure pas dans le PDF. Le tableau parties doit contenir tous les exercices du sujet dans leur ordre d’origine."
    )
}

/// `GET /v1/admin/exams`: every exam, newest first (not paginated, as in
/// Laravel), with the subjects and countries of the upload form.
pub async fn list(
    State(state): State<AppState>,
    _admin: AdminMember,
) -> Result<Response, ApiError> {
    let rows: Vec<Row> = sqlx::query_as(sql!(
        "{SELECT} ORDER BY x.created_at DESC NULLS LAST, x.id DESC"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(no_store(
        axum::http::StatusCode::OK,
        &ExamListing {
            data: rows.into_iter().map(to_exam).collect(),
            subjects: SUBJECTS
                .iter()
                .map(|(value, label)| Choice { value, label })
                .collect(),
            countries: COUNTRIES,
        },
    ))
}

/// `Str::headline()`: words split on spaces, `-`, `_` and case changes,
/// each capitalised.
pub fn headline(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for chunk in name.split(|c: char| c.is_whitespace() || c == '-' || c == '_') {
        let mut word = String::new();
        let mut previous_lower = false;
        for c in chunk.chars() {
            if c.is_uppercase() && previous_lower && !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            previous_lower = c.is_lowercase() || c.is_numeric();
            word.push(c);
        }
        if !word.is_empty() {
            words.push(word);
        }
    }
    words
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `POST /v1/admin/exams` (`multipart/form-data`): `pdf` (at most 20 MiB),
/// `subject`, `country` and an optional `ai_prompt`.
pub async fn create(
    State(state): State<AppState>,
    AdminMember(admin): AdminMember,
    OriginalUri(uri): OriginalUri,
    Form(mut form): Form,
) -> Result<Response, ApiError> {
    let mut pdf: Option<(String, axum::body::Bytes)> = None;
    let mut subject = None;
    let mut country = None;
    let mut extra = None;
    while let Some(field) = form.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "pdf" => {
                let file_name = field.file_name().unwrap_or_default().to_owned();
                let bytes = field.bytes().await.map_err(multipart_error)?;
                pdf = Some((file_name, bytes));
            }
            "subject" => subject = Some(field.text().await.map_err(multipart_error)?),
            "country" => country = Some(field.text().await.map_err(multipart_error)?),
            "ai_prompt" => extra = Some(field.text().await.map_err(multipart_error)?),
            _ => {
                return Err(ApiError::InvalidBody(vec![ParameterError {
                    parameter: name,
                    detail: "unknown field".to_owned(),
                    code: None,
                }]));
            }
        }
    }

    let mut fields = Fields::default();
    match &pdf {
        None => fields.push("pdf", "required", "is required"),
        Some((_, bytes)) if bytes.is_empty() => fields.push("pdf", "required", "is required"),
        Some((_, bytes)) if bytes.len() > PDF_MAX_BYTES => {
            fields.push("pdf", "too_large", "must be at most 20 MiB");
        }
        // `mimes:pdf` reads the content, not the name.
        Some((_, bytes)) if !bytes.starts_with(b"%PDF-") => {
            fields.push("pdf", "not_pdf", "must be a PDF file");
        }
        Some(_) => {}
    }
    let subject_values: Vec<&str> = SUBJECTS.iter().map(|(v, _)| *v).collect();
    let subject = fields.one_of("subject", subject.as_deref(), &subject_values);
    let country = fields.one_of("country", country.as_deref(), COUNTRIES);
    let extra = fields.optional("ai_prompt", extra.as_deref(), 5000);
    fields.finish()?;
    let Some((file_name, bytes)) = pdf else {
        return Err(ApiError::Internal("checked above".to_owned()));
    };

    let stem = match file_name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file_name.as_str(),
    };
    let title = headline(stem);
    let title = if title.is_empty() {
        "Sujet".to_owned()
    } else {
        title.chars().take(255).collect()
    };
    let path = format!("examens/{}.pdf", random_name(40)?);
    let mut tx = state.db.begin().await?;
    store_file(&mut *tx, &path, &bytes, "application/pdf").await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contenu.exams (titre, slug, type, matiere, pays, niveau, statut, fichier_pdf,
                                    nom_fichier_original, prompt_ia, auteur_id, created_at, updated_at)
         VALUES ($1, $2, 'blanc', $3, $4, 'À déterminer', 'attente_import', $5, $6, $7, $8,
                 date_trunc('second', now() AT TIME ZONE 'UTC'), date_trunc('second', now() AT TIME ZONE 'UTC'))
         RETURNING id",
    )
    .bind(&title)
    .bind(format!("analyse-{}", uuid::Uuid::new_v4()))
    .bind(&subject)
    .bind(&country)
    .bind(&path)
    .bind(file_name.chars().take(255).collect::<String>())
    .bind(&extra)
    .bind(admin.id())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(created(find(&state, id, uri.path()).await?))
}

/// `GET /v1/admin/exams/{slug}/pdf`: the statement PDF, drafts included.
pub async fn pdf(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.exams", "exam", &slug, uri.path()).await?;
    let row = find_row(&state, id, uri.path()).await?;
    let missing = || ApiError::NotFound {
        detail: format!("Exam {id} has no stored PDF."),
        instance: Some(uri.path().to_owned()),
    };
    let path = row.fichier_pdf.as_deref().ok_or_else(missing)?;
    let name = row
        .nom_fichier_original
        .clone()
        .unwrap_or_else(|| format!("{}.pdf", row.slug));
    pdf_response(&state.db, path, &name)
        .await?
        .ok_or_else(missing)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportBody {
    ai_response: Option<String>,
}

/// One part of the AI's answer, checked.
#[derive(Debug, PartialEq)]
pub struct Part {
    pub title: String,
    pub points: Option<f64>,
    pub statement_md: String,
    pub correction_md: String,
}

/// The AI's answer, checked (`ImporteurReponseExamen::importer()`).
#[derive(Debug, PartialEq)]
pub struct Answer {
    pub title: String,
    pub kind: String,
    pub level: String,
    pub session: Option<String>,
    pub duration_minutes: Option<i64>,
    pub total_points: Option<i64>,
    pub description: Option<String>,
    pub parts: Vec<Part>,
}

/// Reads the pasted answer: a JSON object, maybe in a ```` ```json ````
/// fence, with the keys the prompt asks for. Errors name the field under
/// `ai_response` (`ai_response.parties.0.titre`).
pub fn read_answer(raw: &str) -> Result<Answer, ApiError> {
    let mut text = raw.trim();
    if text.len() >= 6 && text.starts_with("```") && text.ends_with("```") {
        let mut inner = &text[3..text.len() - 3];
        if inner.len() >= 4 && inner[..4].eq_ignore_ascii_case("json") {
            inner = &inner[4..];
        }
        text = inner.trim();
    }
    let value: Value = serde_json::from_str(text).map_err(|_| {
        ApiError::InvalidFields(vec![ParameterError::field(
            "ai_response",
            "invalid_json",
            "is not valid JSON",
        )])
    })?;

    let mut fields = Fields::default();
    let empty = serde_json::Map::new();
    let object = value.as_object().unwrap_or(&empty);
    let name = |key: &str| format!("ai_response.{key}");
    let title = required_string(&mut fields, &name("titre"), object.get("titre"), 180);
    let kind = required_string(&mut fields, &name("type"), object.get("type"), usize::MAX);
    if !kind.is_empty() && !TYPES.contains(&kind.as_str()) {
        fields.push(
            &name("type"),
            "invalid",
            "must be one of: brevet, bac, blanc",
        );
    }
    let level = required_string(&mut fields, &name("niveau"), object.get("niveau"), 50);
    let session = nullable_string(&mut fields, &name("session"), object.get("session"), 80);
    let duration = nullable_integer(
        &mut fields,
        &name("duree_minutes"),
        object.get("duree_minutes"),
        1,
        1440,
    );
    let total = nullable_integer(
        &mut fields,
        &name("points_total"),
        object.get("points_total"),
        1,
        1000,
    );
    let description = nullable_string(
        &mut fields,
        &name("description"),
        object.get("description"),
        1000,
    );

    let mut parts = Vec::new();
    match object.get("parties") {
        None | Some(Value::Null) => fields.push(
            &name("parties"),
            "required",
            "the answer must list the exercises",
        ),
        Some(Value::Array(items)) if items.is_empty() => fields.push(
            &name("parties"),
            "too_short",
            "the answer must contain at least one exercise",
        ),
        Some(Value::Array(items)) if items.len() > 100 => {
            fields.push(&name("parties"), "too_long", "must have at most 100 items");
        }
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                let part = item.as_object().unwrap_or(&empty);
                let key = |k: &str| format!("ai_response.parties.{i}.{k}");
                let title = required_string(&mut fields, &key("titre"), part.get("titre"), 180);
                let points = match part.get("points") {
                    None | Some(Value::Null) => None,
                    Some(v) => match number(v) {
                        Some(n) if (0.0..=1000.0).contains(&n) => Some(n),
                        Some(_) => {
                            fields.push(&key("points"), "invalid", "must be between 0 and 1000");
                            None
                        }
                        None => {
                            fields.push(&key("points"), "invalid", "must be a number");
                            None
                        }
                    },
                };
                let statement = required_string(
                    &mut fields,
                    &key("enonce_md"),
                    part.get("enonce_md"),
                    usize::MAX,
                );
                let correction = required_string(
                    &mut fields,
                    &key("corrige_md"),
                    part.get("corrige_md"),
                    usize::MAX,
                );
                parts.push(Part {
                    title,
                    points,
                    statement_md: statement.trim().to_owned(),
                    correction_md: correction.trim().to_owned(),
                });
            }
        }
        Some(_) => fields.push(&name("parties"), "invalid", "must be an array"),
    }
    fields.finish()?;
    Ok(Answer {
        title,
        kind,
        level,
        session,
        duration_minutes: duration,
        total_points: total,
        description,
        parts,
    })
}

/// Laravel's `required|string|max`: not null, a string, not blank.
fn required_string(fields: &mut Fields, name: &str, value: Option<&Value>, max: usize) -> String {
    match value {
        None | Some(Value::Null) => {
            fields.push(name, "required", "is required");
            String::new()
        }
        Some(Value::String(s)) if s.trim().is_empty() => {
            fields.push(name, "required", "is required");
            String::new()
        }
        Some(Value::String(s)) => {
            if s.chars().count() > max {
                fields.push(
                    name,
                    "too_long",
                    format!("must be at most {max} characters"),
                );
            }
            s.clone()
        }
        Some(_) => {
            fields.push(name, "invalid", "must be a string");
            String::new()
        }
    }
}

fn nullable_string(
    fields: &mut Fields,
    name: &str,
    value: Option<&Value>,
    max: usize,
) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if s.chars().count() > max {
                fields.push(
                    name,
                    "too_long",
                    format!("must be at most {max} characters"),
                );
            }
            Some(s.clone())
        }
        Some(_) => {
            fields.push(name, "invalid", "must be a string");
            None
        }
    }
}

/// `nullable|integer|min|max`: an integer, or a string of one.
fn nullable_integer(
    fields: &mut Fields,
    name: &str,
    value: Option<&Value>,
    min: i64,
    max: i64,
) -> Option<i64> {
    let n = match value {
        None | Some(Value::Null) => return None,
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        Some(_) => None,
    };
    match n {
        Some(n) if (min..=max).contains(&n) => Some(n),
        Some(_) => {
            fields.push(name, "invalid", format!("must be between {min} and {max}"));
            None
        }
        None => {
            fields.push(name, "invalid", "must be an integer");
            None
        }
    }
}

/// `numeric`: a number, or a string of one.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|f| f.is_finite()),
        _ => None,
    }
}

/// `POST /v1/admin/exams/{slug}/import` (`{"ai_response": "…"}`): replaces
/// the parts and the description with the AI's answer; the exam becomes a
/// draft to proofread.
pub async fn import(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<ImportBody>,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.exams", "exam", &slug, uri.path()).await?;
    find_row(&state, id, uri.path()).await?;
    let mut fields = Fields::default();
    let raw = fields.text(
        "ai_response",
        body.ai_response.as_deref(),
        1,
        ANSWER_MAX_CHARS,
    );
    fields.finish()?;
    let answer = read_answer(&raw)?;

    let slug = unique_slug(
        &state.db,
        "contenu.exams",
        &answer.title,
        None,
        "sujet-examen",
        Some(id),
    )
    .await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM contenu.exam_parties WHERE exam_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (i, part) in answer.parts.iter().enumerate() {
        sqlx::query(
            "INSERT INTO contenu.exam_parties (exam_id, ordre, titre, points, enonce_md, corrige_md)
             VALUES ($1, $2, $3, $4::float8::numeric(5,2), $5, $6)",
        )
        .bind(id)
        .bind(i16::try_from(i + 1).unwrap_or(i16::MAX))
        .bind(&part.title)
        .bind(part.points)
        .bind(&part.statement_md)
        .bind(&part.correction_md)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "UPDATE contenu.exams
            SET titre = $2, slug = $3, type = $4, niveau = $5, session = $6,
                duree_minutes = $7::smallint, points_total = $8::smallint, description = $9,
                statut = 'brouillon', erreur_ia = NULL, analyse_at = date_trunc('second', now() AT TIME ZONE 'UTC'),
                published_at = NULL, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1",
    )
    .bind(id)
    .bind(&answer.title)
    .bind(&slug)
    .bind(&answer.kind)
    .bind(&answer.level)
    .bind(&answer.session)
    .bind(answer.duration_minutes.map(|n| n as i32))
    .bind(answer.total_points.map(|n| n as i32))
    .bind(&answer.description)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/exams/{slug}/publish`: 422 `no_parts` for an exam
/// without parts. Publication is dated now.
pub async fn publish(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.exams", "exam", &slug, uri.path()).await?;
    let row = find_row(&state, id, uri.path()).await?;
    if row.parts == 0 {
        return Err(refused("no_parts", "This exam has no part to publish."));
    }
    sqlx::query(
        "UPDATE contenu.exams
            SET statut = 'publie', published_at = date_trunc('second', now() AT TIME ZONE 'UTC'), erreur_ia = NULL,
                updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `POST /v1/admin/exams/{slug}/unpublish`: back to draft.
pub async fn unpublish(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.exams", "exam", &slug, uri.path()).await?;
    let done = sqlx::query(
        "UPDATE contenu.exams
            SET statut = 'brouillon', published_at = NULL, updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
          WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(not_found("exam", id, uri.path()));
    }
    Ok(ok(find(&state, id, uri.path()).await?))
}

/// `DELETE /v1/admin/exams/{slug}`: the exam, its parts and its PDF.
pub async fn delete(
    State(state): State<AppState>,
    _admin: AdminMember,
    Path(slug): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let id = super::resolve(&state.db, "contenu.exams", "exam", &slug, uri.path()).await?;
    let mut tx = state.db.begin().await?;
    let path: Option<Option<String>> =
        sqlx::query_scalar("DELETE FROM contenu.exams WHERE id = $1 RETURNING fichier_pdf")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(path) = path else {
        return Err(not_found("exam", id, uri.path()));
    };
    if let Some(path) = path {
        sqlx::query("DELETE FROM contenu.fichiers WHERE disque = $1 AND chemin = $2")
            .bind(DISK)
            .bind(&path)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headlines_follow_laravel() {
        assert_eq!(headline("bac-maths-2025"), "Bac Maths 2025");
        assert_eq!(headline("sujet_officielBac"), "Sujet Officiel Bac");
        assert_eq!(headline("  épreuve  écrite "), "Épreuve Écrite");
    }

    #[test]
    fn the_prompt_names_the_subject_and_the_extra_instructions() {
        let p = prompt("Mathématiques", Some("  Conserve le barème officiel. "));
        assert!(p.starts_with("Je joins à ce message un sujet d’examen au format PDF."));
        assert!(p.contains("Matière attendue : Mathématiques.\n\nInstructions complémentaires :\nConserve le barème officiel.\n\nRègles impératives :"));
        assert!(p.contains("\"parties\": [\n    {\n      \"titre\""));
        let bare = prompt("Chimie", None);
        assert!(bare.contains("Matière attendue : Chimie.\n\nRègles impératives :"));
    }

    fn codes(error: ApiError) -> Vec<(String, &'static str)> {
        match error {
            ApiError::InvalidFields(errors) => errors
                .into_iter()
                .map(|e| (e.parameter, e.code.unwrap_or_default()))
                .collect(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn answers_may_come_in_a_fence() {
        let answer = read_answer(
            "```json\n{\"titre\": \"Bac\", \"type\": \"bac\", \"niveau\": \"Terminale\",
              \"parties\": [{\"titre\": \"Suites\", \"points\": 2.5,
                             \"enonce_md\": \" Étudier. \", \"corrige_md\": \"Par récurrence.\"}]}\n```",
        )
        .unwrap();
        assert_eq!(answer.title, "Bac");
        assert_eq!(answer.session, None);
        assert_eq!(answer.parts[0].points, Some(2.5));
        assert_eq!(answer.parts[0].statement_md, "Étudier.");
    }

    #[test]
    fn answers_follow_the_importer_rules() {
        assert_eq!(
            codes(read_answer("Voici les réponses du sujet.").unwrap_err()),
            [("ai_response".to_owned(), "invalid_json")]
        );
        assert_eq!(
            codes(
                read_answer(
                    r#"{"titre": " ", "type": "concours", "niveau": "T", "duree_minutes": 0,
                        "parties": [{"titre": "A", "points": "x", "enonce_md": "e"}]}"#
                )
                .unwrap_err()
            ),
            [
                ("ai_response.titre".to_owned(), "required"),
                ("ai_response.type".to_owned(), "invalid"),
                ("ai_response.duree_minutes".to_owned(), "invalid"),
                ("ai_response.parties.0.points".to_owned(), "invalid"),
                ("ai_response.parties.0.corrige_md".to_owned(), "required"),
            ]
        );
        assert_eq!(
            codes(
                read_answer(r#"{"titre": "A", "type": "bac", "niveau": "T", "parties": []}"#)
                    .unwrap_err()
            ),
            [("ai_response.parties".to_owned(), "too_short")]
        );
    }
}
