//! `GET /v1/languages`, `/v1/languages/{code}` and
//! `/v1/languages/{code}/levels/{level}` (`openapi.yaml`, tag `languages`):
//! the languages taught on pulsars.science/langues, their levels and the
//! words of a level. Public content, like the courses.
//!
//! And their statistics (`docs/languages.md` §Statistics):
//! `POST /v1/languages/events`, what learners do, sent anonymously by the
//! front end; `GET /v1/languages/admin/stats`, the sums, for the admin
//! dashboard.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::params::{Lang, Params};
use super::public::{self, Data, PublicError, PublicResult, served, translated};
use crate::auth::Admin;
use crate::db::language_stats;
use crate::db::languages::{self, Event, LanguageRow, LevelRow};
use crate::error::{ApiError, ParameterError};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub lang: &'static str,
    pub native_name: String,
    pub script: String,
    pub reading_system: Option<String>,
    /// ISO 3166-1 alpha-2 codes of the countries where it is spoken.
    pub countries: Vec<String>,
    pub level_count: i64,
    pub word_count: i64,
}

impl Language {
    fn new(row: LanguageRow, lang: Lang) -> Self {
        let (name, is_tr) = translated(row.name, row.tr_name);
        Self {
            code: row.code,
            name,
            lang: served(lang, is_tr),
            native_name: row.native_name,
            script: row.script,
            reading_system: row.reading_system,
            countries: row.countries,
            level_count: row.level_count,
            word_count: row.word_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LanguageDetail {
    #[serde(flatten)]
    pub language: Language,
    pub levels: Vec<Level>,
}

#[derive(Debug, Serialize)]
pub struct Level {
    pub slug: String,
    pub framework: String,
    pub position: i32,
    pub title: String,
    pub description: String,
    pub lang: &'static str,
    pub word_count: i64,
}

impl Level {
    fn new(row: LevelRow, lang: Lang) -> Self {
        let (title, title_tr) = translated(row.title, row.tr_title);
        let (description, description_tr) = translated(row.description, row.tr_description);
        Self {
            slug: row.slug,
            framework: row.framework,
            position: row.position,
            title,
            description,
            lang: served(lang, title_tr && description_tr),
            word_count: row.word_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LevelDetail {
    pub language: LanguageRef,
    #[serde(flatten)]
    pub level: Level,
    pub previous: Option<LevelRef>,
    pub next: Option<LevelRef>,
    pub words: Vec<Word>,
}

#[derive(Debug, Serialize)]
pub struct LanguageRef {
    pub code: String,
    pub name: String,
    pub native_name: String,
    pub script: String,
    pub reading_system: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LevelRef {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct Word {
    pub id: i64,
    pub text: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub lang: &'static str,
}

pub async fn list(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;

    let rows = languages::list(&state.db, lang.translation()).await?;
    let body = Data {
        data: rows
            .into_iter()
            .map(|row| Language::new(row, lang))
            .collect::<Vec<_>>(),
    };
    public::json(&headers, lang, &body)
}

pub async fn show(
    State(state): State<AppState>,
    Path(code): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;
    let tr = lang.translation();

    let language = languages::find(&state.db, tr, &code)
        .await?
        .ok_or_else(|| language_not_found(&code, uri.path()))?;
    let levels = languages::levels(&state.db, tr, &code).await?;
    let body = Data {
        data: LanguageDetail {
            language: Language::new(language, lang),
            levels: levels
                .into_iter()
                .map(|row| Level::new(row, lang))
                .collect(),
        },
    };
    public::json(&headers, lang, &body)
}

pub async fn level(
    State(state): State<AppState>,
    Path((code, slug)): Path<(String, String)>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;
    let tr = lang.translation();

    let language = languages::find(&state.db, tr, &code)
        .await?
        .ok_or_else(|| language_not_found(&code, uri.path()))?;
    let levels = languages::levels(&state.db, tr, &code).await?;
    let index = levels.iter().position(|l| l.slug == slug).ok_or_else(|| {
        public::not_found(
            format!("No level `{slug}` exists in language `{code}`."),
            uri.path(),
        )
    })?;
    let words = languages::words(&state.db, tr, levels[index].id).await?;

    let reference = |row: &LevelRow| LevelRef {
        slug: row.slug.clone(),
        title: translated(row.title.clone(), row.tr_title.clone()).0,
    };
    let previous = index
        .checked_sub(1)
        .and_then(|i| levels.get(i))
        .map(reference);
    let next = levels.get(index + 1).map(reference);
    let level = levels
        .into_iter()
        .nth(index)
        .map(|row| Level::new(row, lang));
    let Some(level) = level else {
        return Err(language_not_found(&code, uri.path()));
    };

    let body = Data {
        data: LevelDetail {
            language: LanguageRef {
                code: language.code,
                name: translated(language.name, language.tr_name).0,
                native_name: language.native_name,
                script: language.script,
                reading_system: language.reading_system,
            },
            level,
            previous,
            next,
            words: words
                .into_iter()
                .map(|w| {
                    let (meaning, is_tr) = translated(w.meaning, w.tr_meaning);
                    Word {
                        id: w.id,
                        text: w.text,
                        reading: w.reading,
                        meaning,
                        lang: served(lang, is_tr),
                    }
                })
                .collect(),
        },
    };
    public::json(&headers, lang, &body)
}

fn language_not_found(code: &str, path: &str) -> PublicError {
    public::not_found(format!("No language `{code}` is published."), path)
}

// --- Statistics -------------------------------------------------------------

/// The body of `POST /v1/languages/events`. Every field is read leniently,
/// then checked by [`EventBody::validate`], so that all the mistakes come
/// back in one problem.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EventBody {
    kind: Option<String>,
    learner: Option<String>,
    language: Option<String>,
    level: Option<String>,
    score: Option<i64>,
    correct: Option<i64>,
    questions: Option<i64>,
    duration_ms: Option<i64>,
    missed_words: Option<Vec<i64>>,
    character: Option<String>,
    mode: Option<String>,
    mistakes: Option<i64>,
}

/// A checked event: who, where, what.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedEvent {
    pub learner: uuid::Uuid,
    pub language: String,
    pub level: String,
    pub event: Event,
}

/// Longest list of missed words accepted (a round has at most 100 questions).
const MAX_MISSED: usize = 100;

impl EventBody {
    pub fn validate(self) -> Result<CheckedEvent, Vec<ParameterError>> {
        let mut errors = Vec::new();
        let mut fail = |parameter: &str, detail: &str| {
            errors.push(ParameterError {
                parameter: parameter.to_owned(),
                detail: detail.to_owned(),
                code: None,
            });
        };
        let learner = match self.learner.as_deref().map(uuid::Uuid::parse_str) {
            Some(Ok(id)) => Some(id),
            _ => {
                fail("learner", "must be a UUID");
                None
            }
        };
        let text = |value: Option<String>, name: &str, fail: &mut dyn FnMut(&str, &str)| match value
        {
            Some(v) if !v.is_empty() && v.len() <= 32 => Some(v),
            _ => {
                fail(name, "must be a short non-empty string");
                None
            }
        };
        let language = text(self.language, "language", &mut fail);
        let level = text(self.level, "level", &mut fail);
        let within = |value: Option<i64>,
                      name: &str,
                      min: i64,
                      max: i64,
                      fail: &mut dyn FnMut(&str, &str)| {
            match value {
                Some(v) if (min..=max).contains(&v) => Some(v),
                _ => {
                    fail(name, &format!("must be an integer between {min} and {max}"));
                    None
                }
            }
        };

        let event = match self.kind.as_deref() {
            Some("lesson.completed") => {
                let score = within(self.score, "score", 0, 10_000, &mut fail);
                let questions = within(self.questions, "questions", 1, 100, &mut fail);
                let correct = within(self.correct, "correct", 0, 100, &mut fail);
                let duration_ms = within(self.duration_ms, "duration_ms", 0, 86_400_000, &mut fail);
                let missed = match self.missed_words {
                    Some(ids) if ids.len() <= MAX_MISSED => Some(ids),
                    _ => {
                        fail("missed_words", "must be a list of at most 100 word ids");
                        None
                    }
                };
                if let (Some(c), Some(q)) = (correct, questions)
                    && c > q
                {
                    fail("correct", "must not exceed questions");
                }
                match (score, correct, questions, duration_ms, missed) {
                    (
                        Some(score),
                        Some(correct),
                        Some(questions),
                        Some(duration_ms),
                        Some(missed),
                    ) if correct <= questions => Some(Event::LessonCompleted {
                        score: i32::try_from(score).unwrap_or_default(),
                        correct: i16::try_from(correct).unwrap_or_default(),
                        questions: i16::try_from(questions).unwrap_or_default(),
                        duration_ms: i32::try_from(duration_ms).unwrap_or_default(),
                        missed_words: missed,
                    }),
                    _ => None,
                }
            }
            Some("writing.completed") => {
                let mut chars = self.character.as_deref().unwrap_or_default().chars();
                let character = match (chars.next(), chars.next()) {
                    (Some(c), None) if !c.is_ascii() && !c.is_whitespace() && !c.is_control() => {
                        Some(c.to_string())
                    }
                    _ => {
                        fail("character", "must be a single non-ASCII character");
                        None
                    }
                };
                let mode = match self.mode.as_deref() {
                    Some("trace") => Some("trace"),
                    Some("memory") => Some("memory"),
                    _ => {
                        fail("mode", "must be trace or memory");
                        None
                    }
                };
                let mistakes = within(self.mistakes, "mistakes", 0, 1000, &mut fail);
                match (character, mode, mistakes) {
                    (Some(character), Some(mode), Some(mistakes)) => {
                        Some(Event::WritingCompleted {
                            character,
                            mode,
                            mistakes: i16::try_from(mistakes).unwrap_or_default(),
                        })
                    }
                    _ => None,
                }
            }
            _ => {
                fail("kind", "must be lesson.completed or writing.completed");
                None
            }
        };

        match (learner, language, level, event) {
            (Some(learner), Some(language), Some(level), Some(event)) if errors.is_empty() => {
                Ok(CheckedEvent {
                    learner,
                    language,
                    level,
                    event,
                })
            }
            _ => Err(errors),
        }
    }
}

/// `POST /v1/languages/events`: `202 Accepted`, no body.
pub async fn record_event(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Result<Response, ApiError> {
    let parsed: EventBody = serde_json::from_slice(&body).map_err(|_| {
        ApiError::InvalidBody(vec![ParameterError {
            parameter: "body".to_owned(),
            detail: "must be a JSON object".to_owned(),
            code: None,
        }])
    })?;
    let checked = parsed.validate().map_err(ApiError::InvalidBody)?;
    let recorded = languages::record_event(
        &state.db,
        checked.learner,
        &checked.language,
        &checked.level,
        &checked.event,
    )
    .await?;
    if !recorded {
        return Err(ApiError::NotFound {
            detail: format!(
                "No level `{}` exists in a published language `{}`.",
                checked.level, checked.language
            ),
            instance: Some(uri.path().to_owned()),
        });
    }
    let mut response = StatusCode::ACCEPTED.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// `GET /v1/languages/admin/stats?days=30`: learners, lessons, writing, per
/// day and per level, and what learners struggle with, for a game token
/// with the admin role. The site's dashboard reads the same numbers with a
/// session: `GET /v1/admin/languages/stats`.
pub async fn admin_stats(
    State(state): State<AppState>,
    _admin: Admin,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let days = params.integer(
        "days",
        1,
        language_stats::MAX_DAYS,
        language_stats::DEFAULT_DAYS,
    );
    params.finish()?;

    let body = language_stats::collect(&state.db, chrono::Utc::now(), days).await?;
    let mut response = Json(body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.append(header::VARY, HeaderValue::from_static("Authorization"));
    Ok(response)
}
