//! `GET /v1/exercises` and `/v1/exercises/{id}` (`openapi.yaml`, tag
//! `exercises`). Never the answer: the views do not even have it.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::Value;

use super::cursor;
use super::params::{DIFFICULTIES, EXERCISE_TYPES, Lang, Params};
use super::public::{self, Data, Pagination, PublicResult, served, translated};
use crate::db::exercises::{self, ExerciseFilters, ExerciseRow};
use crate::links;
use crate::state::AppState;

/// Longest slug accepted in a filter (`varchar(255)` in Laravel).
const SLUG_MAX_CHARS: usize = 255;

#[derive(Debug, Serialize)]
pub struct Exercise {
    pub id: i64,
    pub lang: &'static str,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub question_md: String,
    pub options: Option<Vec<QcmOption>>,
    pub multiple: Option<bool>,
    pub position: i32,
    pub chapter: ChapterRef,
    pub course: CourseRef,
    pub web_url: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct QcmOption {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ChapterRef {
    pub position: i32,
    pub title: String,
    pub lang: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CourseRef {
    pub slug: String,
    pub title: String,
    pub lang: &'static str,
    pub difficulty: String,
}

pub async fn list(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    let limit = params.limit();
    let cursor = params.cursor();
    let course = params.text("course", SLUG_MAX_CHARS);
    let subject = params.text("subject", SLUG_MAX_CHARS);
    let topic = params.text("topic", SLUG_MAX_CHARS);
    params.require("topic", topic.is_some(), "subject", subject.is_some());
    let kind = params.one_of("type", EXERCISE_TYPES);
    let difficulty = params.one_of("difficulty", DIFFICULTIES);
    let filters = params.finish()?;

    let pagination = Pagination::new("exercises", lang, &filters, limit);
    let after = match cursor {
        Some(c) => Some(cursor::id_key(&cursor::decode(
            &c,
            &pagination.fingerprint,
        )?)?),
        None => None,
    };
    let rows = exercises::list(
        &state.db,
        lang.translation(),
        &ExerciseFilters {
            course: course.as_deref(),
            subject: subject.as_deref(),
            topic: topic.as_deref(),
            kind,
            difficulty,
        },
        after,
        pagination.fetch(),
    )
    .await?;
    let page = pagination.page(
        rows,
        |row| vec![Value::from(row.id)],
        |row| to_exercise(row, lang),
    );
    public::json(&headers, lang, &page)
}

pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;

    let missing = || {
        public::not_found(
            format!("No published exercise has the id '{id}'."),
            uri.path(),
        )
    };
    // The spec lists no 400 here: an id that is not a positive integer names
    // no exercise.
    let numeric = parse_id(&id).ok_or_else(missing)?;
    let row = exercises::find(&state.db, lang.translation(), numeric)
        .await?
        .ok_or_else(missing)?;
    public::json(
        &headers,
        lang,
        &Data {
            data: to_exercise(row, lang),
        },
    )
}

/// A positive decimal id without sign or leading zero.
pub(crate) fn parse_id(raw: &str) -> Option<i64> {
    let canonical =
        !raw.is_empty() && !raw.starts_with('0') && raw.bytes().all(|b| b.is_ascii_digit());
    if canonical { raw.parse().ok() } else { None }
}

pub(crate) fn to_exercise(row: ExerciseRow, lang: Lang) -> Exercise {
    let (title, is_tr) = translated(row.title, row.tr_title);
    let (chapter_title, chapter_tr) = translated(row.chapter_title, row.tr_chapter_title);
    let (course_title, course_tr) = translated(row.course_title, row.tr_course_title);
    let is_qcm = row.kind == "qcm";
    Exercise {
        id: row.id,
        lang: served(lang, is_tr),
        title,
        question_md: translated(row.question_md, row.tr_question_md).0,
        options: if is_qcm {
            Some(row.options.map(|o| options(&o.0)).unwrap_or_default())
        } else {
            None
        },
        multiple: if is_qcm {
            Some(row.multiple.unwrap_or(false))
        } else {
            None
        },
        kind: row.kind,
        position: row.position,
        chapter: ChapterRef {
            position: row.chapter_position,
            title: chapter_title,
            lang: served(lang, chapter_tr),
        },
        web_url: links::exercise(&row.course_slug, row.id),
        course: CourseRef {
            slug: row.course_slug,
            title: course_title,
            lang: served(lang, course_tr),
            difficulty: row.course_difficulty,
        },
    }
}

/// Keeps exactly `id` and `text` of each option, whatever else the JSON
/// holds: a key added to the view by mistake cannot leak through here.
fn options(value: &Value) -> Vec<QcmOption> {
    let text = |item: &Value, key: &str| match item.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| QcmOption {
                    id: text(item, "id"),
                    text: text(item, "text"),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn options_keep_only_id_and_text() {
        let parsed = options(&json!([
            {"id": "a", "text": "31", "correct": true},
            {"id": "b", "text": 7},
            {"text": "no id"},
        ]));
        assert_eq!(
            parsed,
            vec![
                QcmOption {
                    id: "a".into(),
                    text: "31".into()
                },
                QcmOption {
                    id: "b".into(),
                    text: "7".into()
                },
                QcmOption {
                    id: String::new(),
                    text: "no id".into()
                },
            ]
        );
        assert!(options(&json!({"correctes": ["a"]})).is_empty());
    }

    #[test]
    fn only_canonical_ids_are_looked_up() {
        assert_eq!(parse_id("12"), Some(12));
        for bad in ["", "0", "012", "-1", "1e3", "abc", "99999999999999999999"] {
            assert_eq!(parse_id(bad), None, "{bad}");
        }
    }
}
