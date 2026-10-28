//! A member's progress on the courses (`openapi.yaml`, tag `progress`), as
//! Laravel's `BibliothequeController` and `LessonController` showed it and
//! `Progression` computed it.
//!
//! The course and lesson pages read their content from the public routes
//! (`/v1/courses/{slug}`, `…/lessons/{lesson}`), cached for everyone; what
//! changes with the member comes from here, never cached:
//!
//! - `GET /v1/me/progress`: the gauge of every course the member started,
//!   for the library cards and the sidebar;
//! - `GET /v1/me/courses/{slug}/progress`: the course and lesson pages
//!   (completed lessons, percentage, the lesson to resume at);
//! - `PUT /v1/me/lessons/{id}/completion`: Laravel's toggle
//!   (`POST cours/{course}/lecon/{slug}/termine`), with the state wanted in
//!   the body, so that a retried request does not undo itself. Lessons are
//!   named by id: a slug is unique per chapter only.
//!
//! Enrolling has no route of its own, as in Laravel: completing a lesson or
//! answering an exercise enrolls.

use std::collections::BTreeMap;

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::exercises::parse_id;
use super::public::{served, translated};
use super::site::{self, Rewards, not_found, ok, rewarded};
use crate::auth::member::{Member, Verified};
use crate::db::learning::{self, CourseRow, percent};
use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

/// XP of a completed lesson (`Experience::BAREME['lecon']`).
pub const LESSON_XP: i32 = 10;

#[derive(Debug, Serialize)]
pub struct Gauge {
    pub done: i64,
    pub total: i64,
    pub percent: i64,
}

impl Gauge {
    fn new(done: i64, total: i64) -> Self {
        Self {
            done,
            total,
            percent: percent(done, total),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Progress {
    pub enrolled: bool,
    /// Ids of the completed lessons, as `/v1/courses/{slug}` numbers them.
    pub completed_lesson_ids: Vec<i64>,
    pub done: i64,
    pub total: i64,
    pub percent: i64,
    pub completed: bool,
    /// The first lesson not completed yet, else the first of the course
    /// (`Progression::lessonDeReprise`); `null` for a course without lessons.
    pub resume_lesson: Option<LessonRef>,
}

#[derive(Debug, Serialize)]
pub struct LessonRef {
    pub slug: String,
    pub title: String,
    pub lang: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Completion {
    pub completed: bool,
    pub progress: Gauge,
    pub course_completed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionBody {
    completed: bool,
}

/// `GET /v1/me/progress`: `{slug: {done, total, percent}}`.
pub async fn started(State(state): State<AppState>, member: Member) -> Result<Response, ApiError> {
    let rows = learning::started(&state.db, member.id()).await?;
    Ok(ok(rows
        .into_iter()
        .map(|(slug, done, total)| (slug, Gauge::new(done, total)))
        .collect::<BTreeMap<_, _>>()))
}

/// `GET /v1/me/courses/{slug}/progress`.
pub async fn progress(
    State(state): State<AppState>,
    member: Member,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let lang = site::lang(query.as_deref())?;
    let course = learning::course(&state.db, &slug).await?.ok_or_else(|| {
        not_found(
            format!("No published course has the slug '{slug}'."),
            uri.path(),
        )
    })?;
    Ok(ok(summary(&state, member.id(), &course, lang).await?))
}

/// `PUT /v1/me/lessons/{id}/completion`.
pub async fn complete(
    State(state): State<AppState>,
    Verified(member): Verified,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<CompletionBody>,
) -> Result<Response, ApiError> {
    let missing = || {
        not_found(
            format!("No published lesson has the id '{id}'."),
            uri.path(),
        )
    };
    let lesson_id = parse_id(&id).ok_or_else(missing)?;
    let course = learning::lesson_course(&state.db, lesson_id)
        .await?
        .ok_or_else(missing)?;
    let rewards = if body.completed {
        let (newly, brains) =
            learning::complete(&state.db, member.id(), course.id, lesson_id).await?;
        Rewards::new(if newly { LESSON_XP } else { 0 }, brains)
    } else {
        learning::undo(&state.db, member.id(), course.id, lesson_id).await?;
        Rewards::default()
    };
    let done = learning::done_lessons(&state.db, member.id(), course.id).await?;
    let total = learning::lessons(&state.db, course.id, None).await?.len();
    let enrollment = learning::enrollment(&state.db, member.id(), course.id).await?;
    Ok(rewarded(
        StatusCode::OK,
        Completion {
            completed: body.completed,
            progress: Gauge::new(
                i64::try_from(done.len()).unwrap_or(0),
                i64::try_from(total).unwrap_or(0),
            ),
            course_completed: enrollment.is_some_and(|e| e.completed_at.is_some()),
        },
        rewards,
    ))
}

async fn summary(
    state: &AppState,
    user_id: i64,
    course: &CourseRow,
    lang: super::params::Lang,
) -> Result<Progress, ApiError> {
    let lessons = learning::lessons(&state.db, course.id, lang.translation()).await?;
    let completed_lesson_ids = learning::done_lessons(&state.db, user_id, course.id).await?;
    let enrollment = learning::enrollment(&state.db, user_id, course.id).await?;
    let resume_lesson = lessons
        .iter()
        .find(|l| !completed_lesson_ids.contains(&l.id))
        .or_else(|| lessons.first())
        .map(|l| {
            let (title, is_tr) = translated(l.title.clone(), l.tr_title.clone());
            LessonRef {
                slug: l.slug.clone(),
                title,
                lang: served(lang, is_tr),
            }
        });
    let total = i64::try_from(lessons.len()).unwrap_or(i64::MAX);
    let done = i64::try_from(completed_lesson_ids.len()).unwrap_or(0);
    Ok(Progress {
        enrolled: enrollment.is_some(),
        completed: enrollment.is_some_and(|e| e.completed_at.is_some()),
        completed_lesson_ids,
        done,
        total,
        percent: percent(done, total),
        resume_lesson,
    })
}
