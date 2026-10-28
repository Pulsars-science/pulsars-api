//! A member's progress on the courses (`site.enrollments`,
//! `site.lesson_progress`), as Laravel's `App\Services\Progression`.
//!
//! Progress counts lessons only: exercises count in XP, not in progress, so
//! that a learner stuck on an exercise can still finish a course. Only
//! published courses are looked up (`published_courses!()`); a member's
//! rows on a course that is no longer published stay, unseen.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};

use super::brains;

/// A published course, by slug.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CourseRow {
    pub id: i64,
    pub slug: String,
}

pub async fn course(pool: &PgPool, slug: &str) -> Result<Option<CourseRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.id, c.slug FROM ",
        published_courses!(),
        " c WHERE c.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// A lesson of a published course, in reading order, with its title in
/// `tr` when translated.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LessonRow {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub tr_title: Option<String>,
}

/// The lessons of course `course_id`, in reading order (chapters, then
/// lessons, by `ordre`).
pub async fn lessons(
    pool: &PgPool,
    course_id: i64,
    tr: Option<&str>,
) -> Result<Vec<LessonRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT l.id, l.slug, l.title, lt.title AS tr_title
           FROM ",
        published_lessons!(),
        " l LEFT JOIN ",
        lesson_translations!(),
        " lt ON lt.lesson_id = l.id AND lt.lang = $2
          WHERE l.course_id = $1
          ORDER BY l.chapter_position, l.position, l.id"
    ))
    .bind(course_id)
    .bind(tr)
    .fetch_all(pool)
    .await
}

/// The member's enrollment on a course.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EnrollmentRow {
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub async fn enrollment(
    pool: &PgPool,
    user_id: i64,
    course_id: i64,
) -> Result<Option<EnrollmentRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT started_at AT TIME ZONE 'UTC' AS started_at,
                completed_at AT TIME ZONE 'UTC' AS completed_at
           FROM site.enrollments WHERE user_id = $1 AND course_id = $2",
    )
    .bind(user_id)
    .bind(course_id)
    .fetch_optional(pool)
    .await
}

/// Ids of the lessons of course `course_id` the member completed.
pub async fn done_lessons(
    pool: &PgPool,
    user_id: i64,
    course_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT lp.lesson_id
           FROM site.lesson_progress lp
           JOIN contenu.lessons l ON l.id = lp.lesson_id
           JOIN contenu.chapters ch ON ch.id = l.chapter_id
          WHERE lp.user_id = $1 AND ch.course_id = $2
          ORDER BY lp.lesson_id",
    )
    .bind(user_id)
    .bind(course_id)
    .fetch_all(pool)
    .await
}

/// Enrolls the member on the course unless they are already.
pub async fn enroll(
    conn: &mut PgConnection,
    user_id: i64,
    course_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.enrollments (user_id, course_id, started_at, created_at, updated_at)
         VALUES ($1, $2, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_id, course_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(course_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Lessons done and in all, on the connection of a transaction.
async fn counts(
    conn: &mut PgConnection,
    user_id: i64,
    course_id: i64,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM site.lesson_progress lp
                   JOIN contenu.lessons l ON l.id = lp.lesson_id
                   JOIN contenu.chapters ch ON ch.id = l.chapter_id
                  WHERE lp.user_id = $1 AND ch.course_id = $2),
                (SELECT count(*) FROM contenu.lessons l
                   JOIN contenu.chapters ch ON ch.id = l.chapter_id
                  WHERE ch.course_id = $2)",
    )
    .bind(user_id)
    .bind(course_id)
    .fetch_one(conn)
    .await
}

/// `round(done / total × 100)`, 0 for a course without lessons: Laravel's
/// `(int) round(...)`, which rounds half away from zero.
pub fn percent(done: i64, total: i64) -> i64 {
    if total <= 0 {
        return 0;
    }
    ((done as f64 / total as f64) * 100.0).round() as i64
}

/// Marks a lesson completed, as `Progression::terminerLesson`, in one
/// transaction: the progress row (idempotent), the lesson's brains the
/// first time only, the enrollment, its last lesson, and its completion
/// at 100 %. Returns whether the lesson was not completed yet, and the
/// brains credited.
pub async fn complete(
    pool: &PgPool,
    user_id: i64,
    course_id: i64,
    lesson_id: i64,
) -> Result<(bool, i32), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let created: Option<DateTime<Utc>> = sqlx::query_scalar(
        "INSERT INTO site.lesson_progress (user_id, lesson_id, completed_at)
         VALUES ($1, $2, now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_id, lesson_id) DO NOTHING
         RETURNING completed_at AT TIME ZONE 'UTC'",
    )
    .bind(user_id)
    .bind(lesson_id)
    .fetch_optional(&mut *tx)
    .await?;
    let brains = match created {
        Some(at) => brains::credit_lesson(&mut tx, user_id, lesson_id, at).await?,
        None => 0,
    };
    let newly = created.is_some();
    enroll(&mut tx, user_id, course_id).await?;
    let (done, total) = counts(&mut tx, user_id, course_id).await?;
    sqlx::query(
        "UPDATE site.enrollments
            SET derniere_lesson_id = $3,
                completed_at = CASE WHEN $4 THEN coalesce(completed_at, now() AT TIME ZONE 'UTC') END,
                updated_at = now() AT TIME ZONE 'UTC'
          WHERE user_id = $1 AND course_id = $2",
    )
    .bind(user_id)
    .bind(course_id)
    .bind(lesson_id)
    .bind(percent(done, total) == 100)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((newly, brains))
}

/// Takes a completed lesson back, as `Progression::reprendreLesson`: the
/// course is no longer completed. Its brains stay.
pub async fn undo(
    pool: &PgPool,
    user_id: i64,
    course_id: i64,
    lesson_id: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM site.lesson_progress WHERE user_id = $1 AND lesson_id = $2")
        .bind(user_id)
        .bind(lesson_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE site.enrollments SET completed_at = NULL, updated_at = now() AT TIME ZONE 'UTC'
          WHERE user_id = $1 AND course_id = $2",
    )
    .bind(user_id)
    .bind(course_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// A lesson of a published course, by id: its course.
pub async fn lesson_course(
    pool: &PgPool,
    lesson_id: i64,
) -> Result<Option<CourseRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.id, c.slug FROM ",
        published_lessons!(),
        " l JOIN ",
        published_courses!(),
        " c ON c.id = l.course_id WHERE l.id = $1"
    ))
    .bind(lesson_id)
    .fetch_optional(pool)
    .await
}

/// Lessons done and in all on every published course the member completed
/// a lesson of (Laravel's `progressionsPour`: no gauge for a course never
/// started).
pub async fn started(pool: &PgPool, user_id: i64) -> Result<Vec<(String, i64, i64)>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.slug, count(*)::bigint AS done, max(c.lesson_count)::bigint AS total
           FROM site.lesson_progress lp
           JOIN ",
        published_lessons!(),
        " l ON l.id = lp.lesson_id
           JOIN ",
        published_courses!(),
        " c ON c.id = l.course_id
          WHERE lp.user_id = $1
          GROUP BY c.slug
          ORDER BY c.slug"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// The latest published courses the member is not enrolled on: title,
/// slug, description and difficulty, untranslated as on Laravel's
/// dashboard.
pub async fn suggestions(
    pool: &PgPool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<(String, String, String, String)>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT c.title, c.slug, c.description, c.difficulty FROM ",
        published_courses!(),
        " c WHERE NOT EXISTS (SELECT 1 FROM site.enrollments en
                                WHERE en.user_id = $1 AND en.course_id = c.id)
          ORDER BY c.published_at DESC, c.id DESC LIMIT $2"
    ))
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_round_like_php() {
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(2, 3), 67);
        assert_eq!(percent(1, 8), 13);
        assert_eq!(percent(5, 5), 100);
    }
}
