//! What readers send the team (`openapi.yaml`, tag `feedback`): reports of
//! mistakes with their captcha, suggestions, proposed resources; and the
//! public side of the donations goal. Laravel's `SignalementController`,
//! `CaptchaArithmetique`, `SuggestionController`,
//! `DemandeRessourceController` and `ObjectifDons`. The review of what was
//! sent is the admin's (step E).

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use fred::prelude::{ClientLike as _, KeysInterface as _};
use fred::types::Expiration;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::auth::{Caller, empty, limit, new_token, no_store};
use super::site::{self, cadence, created, ok};
use super::upload;
use crate::auth::member::{MaybeMember, Verified};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::rate_limit::WINDOW_SECS;
use crate::state::AppState;

fn invalid(errors: Vec<ParameterError>) -> Result<(), ApiError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::InvalidFields(errors))
    }
}

fn length(
    errors: &mut Vec<ParameterError>,
    field: &str,
    value: Option<&str>,
    required: bool,
    min: usize,
    max: usize,
) {
    match value {
        None if required => errors.push(ParameterError::field(field, "required", "is required")),
        None => {}
        Some(v) => {
            let n = v.chars().count();
            if n < min {
                errors.push(ParameterError::field(
                    field,
                    "too_short",
                    format!("must be at least {min} characters"),
                ));
            } else if n > max {
                errors.push(ParameterError::field(
                    field,
                    "too_long",
                    format!("must be at most {max} characters"),
                ));
            }
        }
    }
}

fn clean(value: Option<&str>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

// --- Captcha -------------------------------------------------------------------

/// A token answers within 10 minutes; once solved it is spent, and stays
/// known as spent for 15.
const CAPTCHA_SECS: i64 = 600;
const CAPTCHA_SPENT_SECS: i64 = 900;
/// `throttle:30,1` and `throttle:20,1`.
const CAPTCHA_LIMIT: u32 = 30;
const REPORT_LIMIT: u32 = 20;

fn captcha_key(token: &str) -> String {
    format!("pulsars-api:captcha:{token}")
}

#[derive(Debug, Serialize)]
pub struct Captcha {
    pub token: String,
    /// `Combien font a + b ?`, in French; `a` and `b` for another language.
    pub question: String,
    pub a: u8,
    pub b: u8,
}

fn digit() -> Result<u8, ApiError> {
    let mut byte = [0u8; 1];
    getrandom::fill(&mut byte)
        .map_err(|e| ApiError::Internal(format!("no randomness for a captcha: {e}")))?;
    Ok(1 + byte[0] % 9)
}

/// `GET /v1/captcha`: a sum of two digits, trivial for a person. The answer
/// stays in Redis, under a random token: nothing to decode on the client.
pub async fn captcha(State(state): State<AppState>, caller: Caller) -> Result<Response, ApiError> {
    let ip = caller.ip.unwrap_or_else(|| "unknown".to_owned());
    limit(&state, "captcha", &ip, CAPTCHA_LIMIT, WINDOW_SECS).await?;
    let (a, b) = (digit()?, digit()?);
    let token = new_token()?;
    if !state.redis.is_connected() {
        return Err(ApiError::Unavailable("redis: not connected".to_owned()));
    }
    state
        .redis
        .set::<(), _, _>(
            captcha_key(&token),
            i64::from(a + b),
            Some(Expiration::EX(CAPTCHA_SECS)),
            None,
            false,
        )
        .await
        .map_err(|e| ApiError::Unavailable(format!("redis: {e}")))?;
    Ok(ok(Captcha {
        question: format!("Combien font {a} + {b} ?"),
        token,
        a,
        b,
    }))
}

/// Checks a captcha answer and spends its token: right, not expired, never
/// used. A wrong answer leaves the token usable.
async fn solve(
    state: &AppState,
    token: Option<&str>,
    answer: Option<&str>,
) -> Result<(), ApiError> {
    let wrong = || {
        ApiError::InvalidFields(vec![ParameterError::field(
            "captcha_answer",
            "incorrect",
            "is not the right answer",
        )])
    };
    let (Some(token), Some(answer)) = (token, answer.map(str::trim)) else {
        return Err(wrong());
    };
    if !super::auth::is_token(token)
        || answer.is_empty()
        || !answer.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(wrong());
    }
    let key = captcha_key(token);
    let stored: Option<String> = state
        .redis
        .get(&key)
        .await
        .map_err(|e| ApiError::Unavailable(format!("redis: {e}")))?;
    let spent = || {
        ApiError::InvalidFields(vec![ParameterError::field(
            "captcha_answer",
            "used",
            "this check was already used: ask for a new question",
        )])
    };
    match stored.as_deref() {
        None => Err(wrong()),
        Some("used") => Err(spent()),
        Some(expected) if expected.parse::<u64>().ok() != answer.parse::<u64>().ok() => {
            Err(wrong())
        }
        Some(expected) => {
            // Atomic: of two requests with the same right answer, one wins.
            let previous: Option<String> = state
                .redis
                .getset(&key, "used")
                .await
                .map_err(|e| ApiError::Unavailable(format!("redis: {e}")))?;
            let _: Result<(), _> = state.redis.expire(&key, CAPTCHA_SPENT_SECS, None).await;
            if previous.as_deref() == Some(expected) {
                Ok(())
            } else {
                Err(spent())
            }
        }
    }
}

// --- Reports -----------------------------------------------------------------

/// What a report may point at (a closed list), and its table.
const TARGETS: &[(&str, &str)] = &[
    ("course", "contenu.courses"),
    ("lesson", "contenu.lessons"),
    ("exercise", "contenu.exercises"),
    ("exam", "contenu.exams"),
];
/// `TypeSignalement`.
const REPORT_TYPES: &[&str] = &["faute", "amelioration", "spam", "abus", "hors_sujet"];

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReportBody {
    target_type: Option<String>,
    target_id: Option<Value>,
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<String>,
    location: Option<String>,
    captcha_token: Option<String>,
    captcha_answer: Option<String>,
}

/// `POST /v1/reports`: a mistake or an idea about a course, a lesson, an
/// exercise or an exam. Open to visitors, who solve the captcha. 204, a
/// double send included (one report).
pub async fn report(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Json(body): Json<ReportBody>,
) -> Result<Response, ApiError> {
    let ip = caller.ip.clone().unwrap_or_else(|| "unknown".to_owned());
    let who = member
        .as_ref()
        .map_or_else(|| ip.clone(), |m| format!("member:{}", m.id()));
    limit(&state, "reports", &who, REPORT_LIMIT, WINDOW_SECS).await?;

    let guest = member.is_none();
    let message = clean(body.message.as_deref());
    let location = clean(body.location.as_deref());
    let mut errors = Vec::new();
    let target = body
        .target_type
        .as_deref()
        .and_then(|t| TARGETS.iter().find(|(name, _)| *name == t).copied());
    if target.is_none() {
        let code = if body.target_type.is_none() {
            "required"
        } else {
            "invalid"
        };
        errors.push(ParameterError::field(
            "target_type",
            code,
            "must be course, lesson, exercise or exam",
        ));
    }
    let target_id = body.target_id.as_ref().and_then(Value::as_i64);
    if target_id.is_none() {
        let code = if body.target_id.is_none() {
            "required"
        } else {
            "invalid"
        };
        errors.push(ParameterError::field(
            "target_id",
            code,
            "must be an integer",
        ));
    }
    match body.kind.as_deref() {
        Some(kind) if REPORT_TYPES.contains(&kind) => {}
        other => errors.push(ParameterError::field(
            "type",
            if other.is_none() {
                "required"
            } else {
                "invalid"
            },
            format!("must be one of {}", REPORT_TYPES.join(", ")),
        )),
    }
    length(&mut errors, "message", message.as_deref(), true, 10, 2000);
    length(&mut errors, "location", location.as_deref(), false, 0, 300);
    if guest {
        if clean(body.captcha_token.as_deref()).is_none() {
            errors.push(ParameterError::field(
                "captcha_token",
                "required",
                "is required",
            ));
        }
        if clean(body.captcha_answer.as_deref()).is_none() {
            errors.push(ParameterError::field(
                "captcha_answer",
                "required",
                "is required",
            ));
        }
    }
    invalid(errors)?;
    let (Some((kind_name, table)), Some(target_id), Some(message)) = (target, target_id, message)
    else {
        return Err(ApiError::InvalidFields(Vec::new()));
    };
    if guest {
        solve(
            &state,
            body.captcha_token.as_deref().map(str::trim),
            body.captcha_answer.as_deref(),
        )
        .await?;
    }
    let exists: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1)"
    )))
    .bind(target_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "message",
            "target_missing",
            "this content no longer exists",
        )]));
    }
    // A second send is almost always a double click: no new report, and no
    // limit message for it.
    let user_id = member.as_ref().map(|m| m.id());
    let double: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.signalements
                         WHERE signalable_type = $1 AND signalable_id = $2 AND message = $3
                           AND ($4::bigint IS NULL OR user_id = $4))",
    )
    .bind(kind_name)
    .bind(target_id)
    .bind(&message)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if double {
        return Ok(empty());
    }
    let (who, max) = match user_id {
        Some(id) => (format!("member:{id}"), 30),
        None => (
            site::visitor_hash(&state, caller.ip.as_deref(), None).await?,
            5,
        ),
    };
    cadence(
        &state,
        "reports",
        &who,
        max,
        "message",
        "too many reports lately: come back a little later, they will all be read",
    )
    .await?;
    sqlx::query(
        "INSERT INTO site.signalements
                (signalable_type, signalable_id, user_id, type, message, emplacement,
                 created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')",
    )
    .bind(kind_name)
    .bind(target_id)
    .bind(user_id)
    .bind(&body.kind)
    .bind(&message)
    .bind(&location)
    .execute(&state.db)
    .await?;
    Ok(empty())
}

// --- Suggestions ---------------------------------------------------------------

/// A suggestion's image: at most 4 MiB, 6000 px a side.
const IMAGE_MAX_BYTES: usize = 4096 * 1024;
const IMAGE_MAX_SIDE: u32 = 6000;
/// `throttle:20,1` on both forms.
const FORM_LIMIT: u32 = 20;

/// `POST /v1/suggestions` (`multipart/form-data`: `title`, `message`, an
/// optional `image`): 201 `{"data": {"id"}}`.
pub async fn suggestion(
    State(state): State<AppState>,
    Verified(member): Verified,
    request: Request,
) -> Result<Response, ApiError> {
    limit(
        &state,
        "suggestions",
        &member.id().to_string(),
        FORM_LIMIT,
        WINDOW_SECS,
    )
    .await?;
    let form = upload::read(request, &state, IMAGE_MAX_BYTES).await?;
    let title = form.text("title");
    let message = form.text("message");
    let mut errors = Vec::new();
    length(&mut errors, "title", title.as_deref(), false, 0, 150);
    length(&mut errors, "message", message.as_deref(), true, 10, 4000);
    let mut image = None;
    if form.too_large.iter().any(|f| f == "image") {
        errors.push(ParameterError::field(
            "image",
            "too_large",
            "must be at most 4 MiB",
        ));
    } else if let Some(file) = form.files.get("image") {
        match upload::image(&file.bytes) {
            None => errors.push(ParameterError::field(
                "image",
                "invalid_image",
                "must be a JPEG, PNG, WebP or GIF image",
            )),
            Some(found) if found.width > IMAGE_MAX_SIDE || found.height > IMAGE_MAX_SIDE => errors
                .push(ParameterError::field(
                    "image",
                    "too_large_dimensions",
                    "must be at most 6000 pixels a side",
                )),
            Some(found) => image = Some((found, file.bytes.clone())),
        }
    }
    invalid(errors)?;
    cadence(
        &state,
        "suggestions",
        &member.id().to_string(),
        10,
        "message",
        "too many suggestions lately: come back a little later",
    )
    .await?;
    let mut tx = state.db.begin().await?;
    let path = match image {
        Some((found, bytes)) => {
            let path = format!(
                "suggestions/{}.{}",
                upload::random_name(40)?,
                found.extension
            );
            upload::store(&mut *tx, "public", &path, &bytes, found.mime).await?;
            Some(path)
        }
        None => None,
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.suggestions (user_id, titre, message, image, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(member.id())
    .bind(&title)
    .bind(&message)
    .bind(&path)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(created(json!({ "id": id })))
}

// --- Proposed resources ------------------------------------------------------------

/// A proposed resource: a PDF of at most 20 MiB.
const RESOURCE_MAX_BYTES: usize = 20480 * 1024;

/// `POST /v1/resource-requests` (`multipart/form-data`): a document to
/// publish, credited to the name of the member's choice, under a licence.
/// Pending until an admin decides. 201 `{"data": {"id", "status"}}`.
pub async fn resource_request(
    State(state): State<AppState>,
    Verified(member): Verified,
    request: Request,
) -> Result<Response, ApiError> {
    limit(
        &state,
        "resource-requests",
        &member.id().to_string(),
        FORM_LIMIT,
        WINDOW_SECS,
    )
    .await?;
    let form = upload::read(request, &state, RESOURCE_MAX_BYTES).await?;
    let title = form.text("title");
    let author = form.text("author_name");
    let license = form.text("license");
    let custom = form.text("license_custom");
    let description = form.text("description");
    let category = form.text("category");
    let lang = form.text("lang");
    let mut errors = Vec::new();
    length(&mut errors, "title", title.as_deref(), true, 0, 200);
    length(&mut errors, "author_name", author.as_deref(), true, 0, 200);
    let licenses: Vec<String> = sqlx::query_scalar("SELECT cle FROM contenu.licences")
        .fetch_all(&state.db)
        .await?;
    match license.as_deref() {
        None => errors.push(ParameterError::field(
            "license",
            "required",
            "choose a licence",
        )),
        Some(key) if !licenses.iter().any(|l| l == key) => errors.push(ParameterError::field(
            "license",
            "invalid",
            "must be a key of GET /v1/licenses",
        )),
        Some("autre") => length(
            &mut errors,
            "license_custom",
            custom.as_deref(),
            true,
            0,
            200,
        ),
        Some(_) => length(
            &mut errors,
            "license_custom",
            custom.as_deref(),
            false,
            0,
            200,
        ),
    }
    length(
        &mut errors,
        "description",
        description.as_deref(),
        false,
        0,
        4000,
    );
    length(&mut errors, "category", category.as_deref(), false, 0, 120);
    if let Some(lang) = &lang
        && !super::profile::LOCALES.contains(&lang.as_str())
    {
        errors.push(ParameterError::field(
            "lang",
            "invalid",
            "must be fr, en or ru",
        ));
    }
    let file = form.files.get("file");
    if form.too_large.iter().any(|f| f == "file") {
        errors.push(ParameterError::field(
            "file",
            "too_large",
            "must be at most 20 MiB",
        ));
    } else {
        match file {
            None => errors.push(ParameterError::field("file", "required", "attach the PDF")),
            Some(file) if !upload::is_pdf(&file.bytes) => errors.push(ParameterError::field(
                "file",
                "not_pdf",
                "must be a PDF file",
            )),
            Some(_) => {}
        }
    }
    invalid(errors)?;
    let Some(file) = file else {
        return Err(ApiError::InvalidFields(Vec::new()));
    };
    cadence(
        &state,
        "resource-requests",
        &member.id().to_string(),
        8,
        "file",
        "too many resources proposed lately: come back a little later",
    )
    .await?;
    let path = format!("demandes-ressources/{}.pdf", upload::random_name(40)?);
    let original: String = file
        .file_name
        .clone()
        .unwrap_or_else(|| "document.pdf".to_owned())
        .chars()
        .take(255)
        .collect();
    let mut tx = state.db.begin().await?;
    upload::store(&mut *tx, "local", &path, &file.bytes, "application/pdf").await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.demandes_ressources
                (user_id, titre, auteur_nom, licence, licence_personnalisee, description,
                 categorie, langue, fichier, nom_fichier_original, taille, created_at, updated_at)
         VALUES ($1, $2, $3, $4, CASE WHEN $4 = 'autre' THEN $5 END, $6, $7, $8, $9, $10, $11,
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(member.id())
    .bind(&title)
    .bind(&author)
    .bind(&license)
    .bind(&custom)
    .bind(&description)
    .bind(&category)
    .bind(&lang)
    .bind(&path)
    .bind(&original)
    .bind(i64::try_from(file.bytes.len()).unwrap_or(i64::MAX))
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(created(json!({ "id": id, "status": "en_attente" })))
}

#[derive(Debug, Serialize)]
pub struct License {
    pub key: String,
    pub name: String,
    pub url: Option<String>,
    /// `autre`: the member writes the licence's name (`license_custom`).
    pub free: bool,
}

/// `GET /v1/licenses`.
pub async fn licenses(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT cle, nom, url FROM contenu.licences ORDER BY cle = 'autre', cle")
            .fetch_all(&state.db)
            .await?;
    Ok(ok(rows
        .into_iter()
        .map(|(key, name, url)| License {
            free: key == "autre",
            key,
            name,
            url,
        })
        .collect::<Vec<_>>()))
}

// --- Donations goal ------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DonationGoal {
    /// `floor(raised / goal × 100)`, at most 100.
    pub percent: i64,
    pub raised: f64,
    pub goal: f64,
    pub currency: &'static str,
}

/// `GET /v1/settings/donation-goal`: what the site's header shows, `null`
/// until an admin sets a goal (`site.reglages`, `objectif_dons`). Amounts
/// are numbers: the site formats them in the reader's language.
pub async fn donation_goal(State(state): State<AppState>) -> Result<Response, ApiError> {
    // The amounts as the admin typed them (`PUT /v1/admin/settings/donation-goal`).
    let stored = crate::db::admin_stats::donation_goal(&state.db).await?;
    let (goal, raised) = (stored.goal, stored.raised.max(0.0));
    let data = (goal.is_finite() && goal > 0.0 && raised.is_finite()).then(|| DonationGoal {
        percent: ((raised / goal * 100.0).floor() as i64).min(100),
        raised,
        goal,
        currency: "EUR",
    });
    Ok(no_store(StatusCode::OK, &json!({ "data": data })))
}
