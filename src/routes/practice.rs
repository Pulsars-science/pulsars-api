//! Exercises as members do them (`openapi.yaml`, tag `practice`), as
//! Laravel's `ExerciseController` and `CorrecteurExercice`:
//!
//! - `GET /v1/exercises/catalog`: the catalogue of `/exercices`, with each
//!   exercise's state for the member (`solved`) and the `state` filter;
//! - `GET /v1/exercises/random`: one exercise drawn among the same filters
//!   (`/exercices/aleatoire`);
//! - `GET /v1/exercises/facets`: the subjects of the catalogue;
//! - `GET /v1/me/exercises/{id}`: the member's attempts, and the solution
//!   once they tried;
//! - `POST /v1/exercises/{id}/attempts`: an answer, graded here. The
//!   expected answers never leave the API; the solution comes with the
//!   result.
//!
//! The statement itself is `GET /v1/exercises/{id}`, public and cached.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Serialize;
use serde_json::{Value, json};

use super::courses::timestamp;
use super::exercises::{Exercise, parse_id, to_exercise};
use super::params::{DIFFICULTIES, EXERCISE_TYPES, Lang, MAX_PAGE, Params};
use super::public::{Page, served, translated};
use super::site::{self, Rewards, not_found, ok, rewarded};
use crate::auth::member::{MaybeMember, Member, Verified};
use crate::db::exercises;
use crate::db::practice::{self, AnswerKeyRow, CardRow, CatalogFilters};
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::links;
use crate::state::AppState;

/// XP of an exercise solved, once per exercise
/// (`Experience::BAREME['exercice']`).
pub const EXERCISE_XP: i32 = 8;

/// Exercises per page of the catalogue, as Laravel's.
const PER_PAGE: u32 = 24;

const STATES: &[&str] = &["todo", "new", "solved"];

// --- Grading (`CorrecteurExercice`) ------------------------------------------

/// PHP's `strval` on a JSON value of the answer key.
fn strval(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(true) => "1".to_owned(),
        _ => String::new(),
    }
}

/// PHP's `(array)` then `strval` on each item: a list as is, a scalar as a
/// list of one, nothing as none.
fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(strval).collect(),
        Some(Value::Object(map)) => map.values().map(strval).collect(),
        Some(other) => vec![strval(other)],
    }
}

/// A QCM is right when the chosen ids are exactly the right ones, in any
/// order, duplicates ignored.
pub fn grade_qcm(correct: &[String], chosen: &[String]) -> bool {
    let mut correct = correct.to_vec();
    correct.sort();
    let mut chosen = chosen.to_vec();
    chosen.sort();
    chosen.dedup();
    correct == chosen
}

/// A short answer is right when, normalized, it is one of the accepted
/// ones, normalized the same way. An empty answer never is.
pub fn grade_short(accepted: &[String], answer: &str) -> bool {
    let answer = normalize(answer);
    !answer.is_empty() && accepted.iter().any(|a| normalize(a) == answer)
}

/// Cyrillic (lower case) to Latin, for the comparison only.
fn cyrillic(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' | 'ё' => "e",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "j",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "h",
        'ц' => "c",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// What `iconv('UTF-8', 'ASCII//TRANSLIT//IGNORE')` makes of the
/// characters an answer may hold; any other non-ASCII character is dropped.
fn ascii(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'ß' => "ss",
        'þ' => "th",
        'ð' => "d",
        '×' => "x",
        '−' | '‐' | '‑' | '‒' | '–' | '—' => "-",
        '‘' | '’' | '‚' | '′' => "'",
        '²' => "^2",
        '³' => "^3",
        '¹' => "^1",
        '½' => " 1/2",
        '¼' => " 1/4",
        '¾' => " 3/4",
        '\u{a0}' | '\u{202f}' => " ",
        _ => return None,
    })
}

/// The tolerant comparison form of an answer: case, accents, Cyrillic,
/// spaces and final punctuation do not count; `3,5` is `3.5`; mathematical
/// signs stay (`3/4` is not `34`).
pub fn normalize(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_ascii() {
            out.push(c);
        } else if let Some(latin) = cyrillic(c).or_else(|| ascii(c)) {
            out.push_str(latin);
        }
    }
    let kept: String = out
        .replace(',', ".")
        .chars()
        .filter(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(
                    c,
                    '-' | '\'' | '.' | '/' | '+' | '*' | '=' | '^' | '(' | ')' | '%'
                )
        })
        .collect();
    kept.trim_matches(|c| ".!?;:,".contains(c)).to_owned()
}

// --- The member's attempts -----------------------------------------------------

#[derive(Debug, Serialize)]
pub struct LastAttempt {
    pub answer: Value,
    pub correct: Option<bool>,
    pub at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExerciseState {
    pub attempt_count: i64,
    pub solved: bool,
    pub last_attempt: Option<LastAttempt>,
    /// `null` until the member tried: the solution would otherwise be read
    /// before answering.
    pub solution_md: Option<String>,
    pub lang: &'static str,
}

#[derive(Debug, Serialize)]
pub struct AttemptResult {
    /// `null` for a free answer, never graded: the learner compares it with
    /// the solution.
    pub correct: Option<bool>,
    pub attempt_count: i64,
    pub solution_md: Option<String>,
    pub lang: &'static str,
}

/// The stored answer (Laravel's keys) as the API shows it.
fn shown_answer(stored: Value) -> Value {
    match stored.get("texte") {
        Some(text) => json!({"text": text}),
        None => json!({"options": stored.get("options").cloned().unwrap_or(json!([]))}),
    }
}

fn solution(key: &AnswerKeyRow, lang: Lang) -> (Option<String>, &'static str) {
    match (&key.tr_solution_md, &key.solution_md) {
        (Some(tr), _) => (Some(tr.clone()), served(lang, true)),
        (None, original) => (original.clone(), served(lang, false)),
    }
}

async fn answer_key(
    state: &AppState,
    id: &str,
    lang: Lang,
    path: &str,
) -> Result<AnswerKeyRow, ApiError> {
    let missing = || not_found(format!("No published exercise has the id '{id}'."), path);
    let id = parse_id(id).ok_or_else(missing)?;
    practice::answer_key(&state.db, id, lang.translation())
        .await?
        .ok_or_else(missing)
}

/// `GET /v1/me/exercises/{id}`.
pub async fn mine(
    State(state): State<AppState>,
    member: Member,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let lang = site::lang(query.as_deref())?;
    let key = answer_key(&state, &id, lang, uri.path()).await?;
    let attempts = practice::attempts(&state.db, member.id(), key.id).await?;
    let (solution_md, served_lang) = solution(&key, lang);
    let tried = attempts.count > 0;
    Ok(ok(ExerciseState {
        attempt_count: attempts.count,
        solved: attempts.solved,
        last_attempt: tried.then(|| LastAttempt {
            answer: shown_answer(attempts.last_answer.map(|a| a.0).unwrap_or(Value::Null)),
            correct: attempts.last_correct,
            at: attempts.last_at.map(timestamp),
        }),
        solution_md: if tried { solution_md } else { None },
        lang: served_lang,
    }))
}

/// `POST /v1/exercises/{id}/attempts`: `{"options": [...]}` for a QCM,
/// `{"text": "..."}` otherwise.
pub async fn attempt(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<serde_json::Map<String, Value>>,
) -> Result<Response, ApiError> {
    let lang = site::lang(query.as_deref())?;
    let key = answer_key(&state, &id, lang, uri.path()).await?;
    let (stored, correct) = match key.kind.as_str() {
        "qcm" => {
            let options = options(body.get("options"))?;
            let right = grade_qcm(&strings(key.correct.as_ref().map(|j| &j.0)), &options);
            (json!({"options": options}), Some(right))
        }
        kind => {
            let max = if kind == "reponse_courte" { 500 } else { 5000 };
            let text = text(body.get("text"), max)?;
            let correct = (kind == "reponse_courte").then(|| {
                let mut accepted = strings(key.accepted.as_ref().map(|j| &j.0));
                for tr in &key.tr_accepted {
                    accepted.extend(strings(Some(&tr.0)));
                }
                grade_short(&accepted, &text)
            });
            (json!({"texte": text}), correct)
        }
    };
    let solved_before = practice::record(
        &state.db,
        member.id(),
        key.id,
        key.course_id,
        &stored,
        correct,
    )
    .await?;
    let attempts = practice::attempts(&state.db, member.id(), key.id).await?;
    let (solution_md, served_lang) = solution(&key, lang);
    let xp = if correct == Some(true) && !solved_before {
        EXERCISE_XP
    } else {
        0
    };
    Ok(rewarded(
        StatusCode::CREATED,
        AttemptResult {
            correct,
            attempt_count: attempts.count,
            solution_md,
            lang: served_lang,
        },
        Rewards::new(xp, 0),
    ))
}

fn field(name: &str, code: &'static str, detail: &str) -> ApiError {
    ApiError::InvalidFields(vec![ParameterError::field(name, code, detail)])
}

/// `options`: at least one, each a string of at most 50 characters
/// (trimmed, as Laravel's `TrimStrings`).
fn options(value: Option<&Value>) -> Result<Vec<String>, ApiError> {
    let items = match value {
        None | Some(Value::Null) => return Err(field("options", "required", "is required")),
        Some(Value::Array(items)) if items.is_empty() => {
            return Err(field("options", "required", "is required"));
        }
        Some(Value::Array(items)) => items,
        Some(_) => return Err(field("options", "invalid", "must be an array")),
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let pointer = format!("options/{i}");
            match item {
                Value::String(s) if s.trim().is_empty() => {
                    Err(field(&pointer, "required", "is required"))
                }
                Value::String(s) if s.trim().chars().count() > 50 => {
                    Err(field(&pointer, "too_long", "must be at most 50 characters"))
                }
                Value::String(s) => Ok(s.trim().to_owned()),
                _ => Err(field(&pointer, "invalid", "must be a string")),
            }
        })
        .collect()
}

/// `text`: required, at most `max` characters once trimmed.
fn text(value: Option<&Value>, max: usize) -> Result<String, ApiError> {
    match value {
        Some(Value::String(s)) if !s.trim().is_empty() => {
            let s = s.trim();
            if s.chars().count() > max {
                Err(field(
                    "text",
                    "too_long",
                    &format!("must be at most {max} characters"),
                ))
            } else {
                Ok(s.to_owned())
            }
        }
        None | Some(Value::Null) | Some(Value::String(_)) => {
            Err(field("text", "required", "is required"))
        }
        Some(_) => Err(field("text", "invalid", "must be a string")),
    }
}

// --- Catalogue -----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Card {
    pub id: i64,
    pub title: String,
    pub lang: &'static str,
    #[serde(rename = "type")]
    pub kind: String,
    pub auto_corrected: bool,
    pub course: CourseRef,
    /// Not translated: a filing label.
    pub theme: String,
    pub difficulty: String,
    /// What it earns while not solved yet.
    pub xp: i32,
    /// `null`: never tried (or a visitor); else whether one attempt was right.
    pub solved: Option<bool>,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct CourseRef {
    pub slug: String,
    pub title: String,
    pub lang: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Subject {
    pub slug: String,
    pub name: String,
    pub exercise_count: i64,
}

#[derive(Debug, Serialize)]
pub struct Facets {
    pub subjects: Vec<Subject>,
}

struct Query {
    lang: Lang,
    search: Option<String>,
    subject: Option<String>,
    kind: Option<&'static str>,
    difficulty: Option<&'static str>,
    state: Option<&'static str>,
    page: u32,
}

impl Query {
    fn parse(raw: Option<&str>) -> Result<Self, ApiError> {
        let mut params = Params::parse(raw);
        let lang = params.lang();
        let search = params.text("q", 100);
        let subject = params.text("subject", 255);
        let kind = params.one_of("type", EXERCISE_TYPES);
        let difficulty = params.one_of("difficulty", DIFFICULTIES);
        let state = params.one_of("state", STATES);
        let page = params.integer("page", 1, MAX_PAGE, 1);
        params.finish()?;
        Ok(Self {
            lang,
            search,
            subject,
            kind,
            difficulty,
            state,
            page,
        })
    }

    /// The state filter means nothing without knowing who looks: ignored
    /// for a visitor, as in Laravel.
    fn filters(&self, member: bool) -> CatalogFilters<'_> {
        CatalogFilters {
            search: self.search.as_deref(),
            subject: self.subject.as_deref(),
            kind: self.kind,
            difficulty: self.difficulty,
            state: if member { self.state } else { None },
        }
    }
}

fn card(row: CardRow, lang: Lang) -> Card {
    let (title, is_tr) = translated(row.title, row.tr_title);
    let (course_title, course_tr) = translated(row.course_title, row.tr_course_title);
    Card {
        id: row.id,
        lang: served(lang, is_tr),
        title,
        auto_corrected: row.kind != "reponse_libre",
        kind: row.kind,
        web_url: links::exercise(&row.course_slug, row.id),
        course: CourseRef {
            slug: row.course_slug,
            title: course_title,
            lang: served(lang, course_tr),
        },
        theme: row.theme,
        difficulty: row.difficulty,
        xp: EXERCISE_XP,
        solved: row.solved,
    }
}

/// `GET /v1/exercises/catalog`: `{data, next_cursor: null, total}`.
pub async fn catalog(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let query = Query::parse(query.as_deref())?;
    let user_id = member.as_ref().map(Member::id);
    let limit = i64::from(PER_PAGE);
    let (rows, total) = practice::catalog(
        &state.db,
        query.lang.translation(),
        user_id,
        &query.filters(user_id.is_some()),
        limit,
        i64::from(query.page - 1) * limit,
    )
    .await?;
    let lang = query.lang;
    Ok(super::auth::no_store(
        StatusCode::OK,
        &Page {
            data: rows.into_iter().map(|r| card(r, lang)).collect::<Vec<_>>(),
            next_cursor: None,
            total: Some(total),
        },
    ))
}

/// `GET /v1/exercises/random`: the exercise, as `GET /v1/exercises/{id}`.
pub async fn random(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let query = Query::parse(query.as_deref())?;
    let user_id = member.as_ref().map(Member::id);
    let picked = practice::random(&state.db, user_id, &query.filters(user_id.is_some()))
        .await?
        .ok_or_else(|| not_found("No exercise matches these filters.", uri.path()))?;
    let row = exercises::find(&state.db, query.lang.translation(), picked.0)
        .await?
        .ok_or_else(|| not_found("No exercise matches these filters.", uri.path()))?;
    let exercise: Exercise = to_exercise(row, query.lang);
    Ok(ok(exercise))
}

/// `GET /v1/exercises/facets`.
pub async fn facets(State(state): State<AppState>) -> Result<Response, ApiError> {
    let subjects = practice::subjects(&state.db).await?;
    Ok(ok(Facets {
        subjects: subjects
            .into_iter()
            .map(|(slug, name, exercise_count)| Subject {
                slug,
                name,
                exercise_count,
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|&i| i.to_owned()).collect()
    }

    #[test]
    fn a_qcm_needs_all_the_right_options_and_only_them() {
        let correct = s(&["a", "c"]);
        assert!(grade_qcm(&correct, &s(&["a", "c"])));
        assert!(grade_qcm(&correct, &s(&["c", "a"])));
        assert!(grade_qcm(&correct, &s(&["c", "a", "a"])));
        assert!(!grade_qcm(&correct, &s(&["a"])));
        assert!(!grade_qcm(&correct, &s(&["a", "b", "c"])));
        assert_eq!(strings(Some(&json!([1, "b"]))), s(&["1", "b"]));
        assert_eq!(strings(Some(&json!("a"))), s(&["a"]));
        assert!(strings(None).is_empty());
    }

    #[test]
    fn short_answers_ignore_case_accents_and_spaces() {
        let accepted = s(&["main", "<main>"]);
        for answer in ["main", "MAIN", " Main ", "main."] {
            assert!(grade_short(&accepted, answer), "{answer}");
        }
        assert!(!grade_short(&accepted, "body"));
        assert!(!grade_short(&accepted, ""));
        assert!(grade_short(&s(&["élève"]), "ELEVE"));
        assert!(grade_short(&s(&["3,5"]), "3.5"));
        assert!(grade_short(&s(&["3a+3"]), "3a + 3"));
        assert!(!grade_short(&s(&["3/4"]), "34"));
        assert!(grade_short(&s(&["52 euros"]), "52euros!"));
    }

    #[test]
    fn russian_answers_compare_in_latin_letters() {
        assert_eq!(normalize("Ёлка"), "elka");
        assert!(grade_short(&s(&["сумма"]), "Сумма"));
        assert!(!grade_short(&s(&["сумма"]), "разность"));
    }
}
