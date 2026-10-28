//! `GET /v1/catalog`, `/v1/courses`, `/v1/courses/{slug}` and
//! `/v1/courses/{slug}/lessons/{lesson}` (`openapi.yaml`, tags `catalog` and
//! `courses`).

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::HeaderMap;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;

use super::cursor;
use super::params::{DIFFICULTIES, LANGS, LEVELS, Lang, Params};

/// `sort` of `GET /v1/courses`.
const SORTS: &[&str] = &["title", "recent"];
use super::public::{self, Data, Pagination, PublicResult, served, translated};
use crate::db::courses::{self, After, CatalogRow, CourseFilters, CourseRow, Sort};
use crate::links;
use crate::state::AppState;

// --- Catalogue --------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub taxonomy: &'static str,
    pub lang: &'static str,
    pub subjects: Vec<CatalogSubject>,
}

#[derive(Debug, Serialize)]
pub struct CatalogSubject {
    pub slug: String,
    pub name: String,
    pub course_count: i64,
    pub topics: Vec<CatalogTopic>,
}

#[derive(Debug, Serialize)]
pub struct CatalogTopic {
    pub slug: String,
    pub name: String,
    pub course_count: i64,
}

pub async fn catalog(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;

    let rows = courses::catalog(&state.db, lang.translation()).await?;
    let body = Data {
        data: to_catalog(rows, lang),
    };
    public::json(&headers, lang, &body)
}

/// Groups the (subject, topic) rows, which arrive sorted. Rows of one
/// subject slug are merged even when its raw name was spelled differently.
fn to_catalog(rows: Vec<CatalogRow>, lang: Lang) -> Catalog {
    let mut subjects: Vec<CatalogSubject> = Vec::new();
    for row in rows {
        let index = match subjects.iter().position(|s| s.slug == row.subject_slug) {
            Some(index) => index,
            None => {
                subjects.push(CatalogSubject {
                    slug: row.subject_slug.clone(),
                    name: translated(row.subject_name.clone(), row.tr_subject_name.clone()).0,
                    course_count: 0,
                    topics: Vec::new(),
                });
                subjects.len() - 1
            }
        };
        let subject = &mut subjects[index];
        let count = i64::from(row.course_count);
        subject.course_count += count;
        // Courses without a topic count for the subject only.
        if let (Some(slug), Some(name)) = (row.topic_slug, row.topic_name) {
            match subject.topics.iter_mut().find(|t| t.slug == slug) {
                Some(topic) => topic.course_count += count,
                None => subject.topics.push(CatalogTopic {
                    slug,
                    name,
                    course_count: count,
                }),
            }
        }
    }
    Catalog {
        // The subject/topic tree is derived from the free `theme` string.
        taxonomy: "theme",
        lang: lang.code(),
        subjects,
    }
}

// --- Courses ----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SubjectRef {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TopicRef {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CourseSummary {
    pub id: i64,
    pub slug: String,
    pub lang: &'static str,
    pub title: String,
    pub description: String,
    pub theme: String,
    pub subject: SubjectRef,
    pub topic: Option<TopicRef>,
    pub difficulty: String,
    pub levels: Vec<String>,
    pub duration_minutes: Option<i32>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub chapter_count: i32,
    pub lesson_count: i32,
    pub exercise_count: i32,
    pub available_langs: Vec<&'static str>,
    pub published_at: String,
    pub updated_at: String,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct CourseDetail {
    #[serde(flatten)]
    pub summary: CourseSummary,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Serialize)]
pub struct Chapter {
    pub position: i32,
    pub title: String,
    pub lang: &'static str,
    pub lessons: Vec<LessonItem>,
    pub exercises: Vec<ExerciseItem>,
}

#[derive(Debug, Serialize)]
pub struct LessonItem {
    pub slug: String,
    pub position: i32,
    pub title: String,
    pub lang: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ExerciseItem {
    pub id: i64,
    pub position: i32,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub lang: &'static str,
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
    let subject = params.text("subject", 100);
    let topic = params.text("topic", 100);
    params.require("topic", topic.is_some(), "subject", subject.is_some());
    let difficulty = params.one_of("difficulty", DIFFICULTIES);
    let level = params.one_of("level", LEVELS);
    let tag = params.text("tag", 50);
    let theme = params.text("theme", 50);
    let available_in = params.one_of("available_in", LANGS);
    let search = params.search();
    let sort = match params.one_of("sort", SORTS) {
        Some("recent") => Sort::Recent,
        _ => Sort::Title,
    };
    let page = params.page(cursor.is_some());
    let filters = params.finish()?;

    let pagination = Pagination::new("courses", lang, &filters, limit);
    let after = match (page, &cursor) {
        (Some(page), _) => After::Offset(pagination.offset(page)),
        (None, Some(c)) => {
            let key = cursor::decode(c, &pagination.fingerprint)?;
            match sort {
                Sort::Title => {
                    let (title, id) = cursor::text_id_key(&key)?;
                    After::Title(title, id)
                }
                Sort::Recent => {
                    let (at, id) = cursor::time_id_key(&key)?;
                    After::Recent(at, id)
                }
            }
        }
        (None, None) => After::Start,
    };
    let course_filters = CourseFilters {
        subject: subject.as_deref(),
        topic: topic.as_deref(),
        difficulty,
        level,
        tag: tag.as_deref(),
        theme: theme.as_deref(),
        // French is the source language: every course has it.
        translated_in: available_in.filter(|l| *l != "fr"),
        search: search.as_deref(),
    };
    let tr = lang.translation();
    let rows = courses::list(
        &state.db,
        tr,
        &course_filters,
        sort,
        after,
        pagination.fetch(),
    )
    .await?;
    let body = match page {
        Some(_) => {
            let total = match rows.first() {
                Some(row) => row.total,
                // Past the last page: count from the first one.
                None => courses::list(&state.db, tr, &course_filters, sort, After::Start, 1)
                    .await?
                    .first()
                    .map_or(0, |row| row.total),
            };
            pagination.numbered(rows, total, |row| to_summary(row, lang))
        }
        None => pagination.page(
            rows,
            |row| match sort {
                Sort::Title => vec![Value::from(row.title.clone()), Value::from(row.id)],
                Sort::Recent => cursor::time_id(row.published_at, row.id),
            },
            |row| to_summary(row, lang),
        ),
    };
    public::json(&headers, lang, &body)
}

pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;
    let tr = lang.translation();

    let course = courses::find(&state.db, tr, &slug)
        .await?
        .ok_or_else(|| course_not_found(&slug, uri.path()))?;
    let (chapters, lessons, exercises) = tokio::try_join!(
        courses::chapters(&state.db, tr, course.id),
        courses::lessons(&state.db, tr, course.id),
        courses::exercises(&state.db, tr, course.id),
    )?;

    let chapters = chapters
        .into_iter()
        .map(|chapter| {
            let (title, is_tr) = translated(chapter.title, chapter.tr_title);
            Chapter {
                position: chapter.position,
                title,
                lang: served(lang, is_tr),
                lessons: lessons
                    .iter()
                    .filter(|l| l.chapter_id == chapter.id)
                    .map(|l| {
                        let (title, is_tr) = translated(l.title.clone(), l.tr_title.clone());
                        LessonItem {
                            slug: l.slug.clone(),
                            position: l.position,
                            title,
                            lang: served(lang, is_tr),
                        }
                    })
                    .collect(),
                exercises: exercises
                    .iter()
                    .filter(|e| e.chapter_id == chapter.id)
                    .map(|e| {
                        let (title, is_tr) = translated(e.title.clone(), e.tr_title.clone());
                        ExerciseItem {
                            id: e.id,
                            position: e.position,
                            kind: e.kind.clone(),
                            title,
                            lang: served(lang, is_tr),
                        }
                    })
                    .collect(),
            }
        })
        .collect();

    let body = Data {
        data: CourseDetail {
            summary: to_summary(course, lang),
            chapters,
        },
    };
    public::json(&headers, lang, &body)
}

fn course_not_found(slug: &str, path: &str) -> public::PublicError {
    public::not_found(format!("No published course has the slug '{slug}'."), path)
}

fn to_summary(row: CourseRow, lang: Lang) -> CourseSummary {
    let (title, is_tr) = translated(row.title, row.tr_title);
    let (description, _) = translated(row.description, row.tr_description);
    let mut available_langs = vec!["fr"];
    for code in ["en", "ru"] {
        if row.translation_langs.iter().any(|l| l == code) {
            available_langs.push(code);
        }
    }
    let tags = match row.tags.0 {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|t| t.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    CourseSummary {
        id: row.id,
        web_url: links::course(&row.slug),
        slug: row.slug,
        lang: served(lang, is_tr),
        title,
        description,
        theme: row.theme,
        subject: SubjectRef {
            slug: row.subject_slug,
            name: translated(row.subject_name, row.tr_subject_name).0,
        },
        topic: match (row.topic_slug, row.topic_name) {
            (Some(slug), Some(name)) => Some(TopicRef { slug, name }),
            _ => None,
        },
        difficulty: row.difficulty,
        levels: row.levels,
        duration_minutes: row.duration_minutes,
        tags,
        author: row.author_pseudo,
        chapter_count: row.chapter_count,
        lesson_count: row.lesson_count,
        exercise_count: row.exercise_count,
        available_langs,
        published_at: timestamp(row.published_at),
        // Laravel always sets it; the schema has no null here.
        updated_at: timestamp(row.updated_at.unwrap_or(row.published_at)),
    }
}

/// ISO 8601 in UTC with a `Z`, to the second.
pub fn timestamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

// --- Lessons ----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Lesson {
    pub slug: String,
    pub lang: &'static str,
    pub title: String,
    pub position: i32,
    pub content_md: String,
    pub chapter: ChapterRef,
    pub course: CourseRef,
    pub previous: Option<LessonLink>,
    pub next: Option<LessonLink>,
    pub updated_at: String,
    pub web_url: String,
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
}

#[derive(Debug, Serialize)]
pub struct LessonLink {
    pub slug: String,
    pub title: String,
}

pub async fn lesson(
    State(state): State<AppState>,
    Path((slug, lesson_slug)): Path<(String, String)>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;
    let tr = lang.translation();

    let course = courses::find(&state.db, tr, &slug)
        .await?
        .ok_or_else(|| course_not_found(&slug, uri.path()))?;
    let (chapters, lessons) = tokio::try_join!(
        courses::chapters(&state.db, tr, course.id),
        courses::lessons(&state.db, tr, course.id),
    )?;

    // Lesson slugs are only unique per chapter: the first in reading order
    // wins (`openapi.yaml`, getLesson).
    let missing = || {
        public::not_found(
            format!("The course '{slug}' has no lesson with the slug '{lesson_slug}'."),
            uri.path(),
        )
    };
    let index = lessons
        .iter()
        .position(|l| l.slug == lesson_slug)
        .ok_or_else(missing)?;
    let current = &lessons[index];
    let chapter = chapters
        .iter()
        .find(|c| c.id == current.chapter_id)
        .ok_or_else(missing)?;
    let content = courses::lesson_content(&state.db, tr, current.id)
        .await?
        .ok_or_else(missing)?;

    let link = |i: usize| {
        lessons.get(i).map(|l| LessonLink {
            slug: l.slug.clone(),
            title: translated(l.title.clone(), l.tr_title.clone()).0,
        })
    };
    let (title, is_tr) = translated(current.title.clone(), current.tr_title.clone());
    let (chapter_title, chapter_tr) = translated(chapter.title.clone(), chapter.tr_title.clone());
    let (course_title, course_tr) = translated(course.title.clone(), course.tr_title.clone());

    let body = Data {
        data: Lesson {
            slug: current.slug.clone(),
            lang: served(lang, is_tr),
            title,
            position: current.position,
            content_md: translated(content.content_md, content.tr_content_md).0,
            chapter: ChapterRef {
                position: chapter.position,
                title: chapter_title,
                lang: served(lang, chapter_tr),
            },
            course: CourseRef {
                slug: course.slug.clone(),
                title: course_title,
                lang: served(lang, course_tr),
            },
            previous: index.checked_sub(1).and_then(link),
            next: link(index + 1),
            updated_at: timestamp(
                content
                    .updated_at
                    .or(course.updated_at)
                    .unwrap_or(course.published_at),
            ),
            web_url: links::lesson(&course.slug, &current.slug),
        },
    };
    public::json(&headers, lang, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(subject: &str, tr: Option<&str>, topic: Option<&str>, count: i32) -> CatalogRow {
        CatalogRow {
            subject_slug: subject.to_lowercase(),
            subject_name: subject.to_owned(),
            tr_subject_name: tr.map(str::to_owned),
            topic_slug: topic.map(str::to_lowercase),
            topic_name: topic.map(str::to_owned),
            course_count: count,
        }
    }

    #[test]
    fn catalog_counts_courses_without_topic_in_the_subject_only() {
        let catalog = to_catalog(
            vec![
                row("Chimie", Some("Chemistry"), Some("Transformations"), 7),
                row("Mathematiques", None, None, 4),
                row("Mathematiques", None, Some("Algebre"), 13),
                row("Mathematiques", None, Some("Analyse"), 31),
            ],
            Lang::En,
        );
        let json = serde_json::to_value(&catalog).unwrap_or_default();
        assert_eq!(
            json,
            serde_json::json!({
                "taxonomy": "theme",
                "lang": "en",
                "subjects": [
                    {"slug": "chimie", "name": "Chemistry", "course_count": 7, "topics": [
                        {"slug": "transformations", "name": "Transformations", "course_count": 7},
                    ]},
                    {"slug": "mathematiques", "name": "Mathematiques", "course_count": 48, "topics": [
                        {"slug": "algebre", "name": "Algebre", "course_count": 13},
                        {"slug": "analyse", "name": "Analyse", "course_count": 31},
                    ]},
                ],
            })
        );
    }

    #[test]
    fn timestamps_are_utc_seconds_with_z() {
        let at = DateTime::from_timestamp(1_786_613_524, 999_000_000).unwrap_or_default();
        assert_eq!(timestamp(at), "2026-08-13T09:32:04Z");
    }
}
