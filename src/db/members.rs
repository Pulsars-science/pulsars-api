//! Member profiles, from `site.members`, `site.member_roles`,
//! `site.member_xp` and `site.member_brains`
//! (`docs/contract/database.md` §4, `docs/contract/brains.md` §4).
//!
//! Read by id for `/v1/me`, by pseudo for the public profiles, and by a
//! piece of pseudo for the search of `/v1/members`: never listed whole.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Member {
    pub id: i64,
    pub pseudo: String,
    /// Relative to Laravel's `public` disk.
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub member_since: Option<DateTime<Utc>>,
    pub xp_total: i32,
    pub xp_level: i32,
    /// Brains balance, shown to the member themselves only.
    pub brains: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MemberRole {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
}

/// The profile, XP and brains of member `id`, or `None` if the account no
/// longer exists.
pub async fn find(pool: &PgPool, id: i64) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as(
        "SELECT m.id, m.pseudo, m.avatar_path, m.bio, m.member_since,
                x.total AS xp_total, x.level AS xp_level,
                coalesce(b.balance, 0) AS brains
           FROM site.members m
           JOIN site.member_xp x ON x.user_id = m.id
           LEFT JOIN site.member_brains b ON b.user_id = m.id
          WHERE m.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Roles of member `id`: permission roles first, highest level first, then
/// in display order (the `Me` schema's order).
pub async fn roles(pool: &PgPool, id: i64) -> Result<Vec<MemberRole>, sqlx::Error> {
    sqlx::query_as(
        "SELECT slug, name, category, color
           FROM site.member_roles
          WHERE user_id = $1
          ORDER BY permission_level DESC, display_order, slug",
    )
    .bind(id)
    .fetch_all(pool)
    .await
}

/// The brains balance of `user_id` (`brains.md` §4): 0 for an account
/// without movements, or one that no longer exists.
pub async fn brains(pool: &PgPool, user_id: i64) -> Result<i32, sqlx::Error> {
    let balance: Option<i32> =
        sqlx::query_scalar("SELECT balance FROM site.member_brains WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(balance.unwrap_or(0))
}

/// What the public profile of a member shows (`ProfilPublicController`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Profile {
    pub id: i64,
    pub pseudo: String,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub member_since: Option<DateTime<Utc>>,
    pub school_level: Option<String>,
    pub class: Option<String>,
    /// Whether the member shows their brains balance (`cerveaux_publics`).
    pub brains_public: bool,
    pub brains: i32,
    /// Forum posts not deleted (Eloquent's soft deletes).
    pub question_count: i64,
    pub answer_count: i64,
}

/// The profile of the member named `pseudo` (exactly, as Laravel's route
/// binding), or `None`.
pub async fn find_by_pseudo(pool: &PgPool, pseudo: &str) -> Result<Option<Profile>, sqlx::Error> {
    sqlx::query_as(
        "SELECT u.id, u.pseudo, u.avatar AS avatar_path, u.bio,
                u.created_at AT TIME ZONE 'UTC' AS member_since,
                u.niveau_scolaire AS school_level, u.classe AS class,
                u.cerveaux_publics AS brains_public,
                coalesce(b.balance, 0) AS brains,
                (SELECT count(*) FROM site.questions q
                  WHERE q.user_id = u.id AND q.deleted_at IS NULL) AS question_count,
                (SELECT count(*) FROM site.answers a
                  WHERE a.user_id = u.id AND a.deleted_at IS NULL) AS answer_count
           FROM site.users u
           LEFT JOIN site.member_brains b ON b.user_id = u.id
          WHERE u.pseudo = $1",
    )
    .bind(pseudo)
    .fetch_optional(pool)
    .await
}

/// Pseudo and avatar path of member `id`.
pub async fn find_by_id_brief(
    pool: &PgPool,
    id: i64,
) -> Result<Option<(String, Option<String>)>, sqlx::Error> {
    sqlx::query_as("SELECT pseudo, avatar_path FROM site.members WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// The id of the member named `pseudo`.
pub async fn id_by_pseudo(pool: &PgPool, pseudo: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM site.users WHERE pseudo = $1")
        .bind(pseudo)
        .fetch_optional(pool)
        .await
}

/// What a member's experience is made of: the counts of
/// `Experience::comptes` and `Experience::jeu`, every forum post included.
#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct XpCounts {
    pub lessons: i64,
    pub exercises: i64,
    pub courses: i64,
    pub questions: i64,
    pub answers: i64,
    pub accepted_answers: i64,
    pub games: i64,
    /// XP of the games, capped per day (UTC), as `site.member_xp`.
    pub game_xp: i64,
}

pub async fn xp_counts(pool: &PgPool, user_id: i64) -> Result<XpCounts, sqlx::Error> {
    sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM site.lesson_progress
              WHERE user_id = $1 AND completed_at IS NOT NULL) AS lessons,
            (SELECT count(DISTINCT exercise_id) FROM site.exercise_attempts
              WHERE user_id = $1 AND correct = true) AS exercises,
            (SELECT count(*) FROM site.enrollments
              WHERE user_id = $1 AND completed_at IS NOT NULL) AS courses,
            (SELECT count(*) FROM site.questions WHERE user_id = $1) AS questions,
            (SELECT count(*) FROM site.answers WHERE user_id = $1) AS answers,
            (SELECT count(*) FROM site.answers a
               JOIN site.questions q ON q.accepted_answer_id = a.id
              WHERE a.user_id = $1) AS accepted_answers,
            (SELECT count(*) FROM site.game_results WHERE user_id = $1) AS games,
            (SELECT coalesce(sum(least(30, j.games * 3 + j.wins * 5)), 0)::bigint
               FROM (SELECT count(*) AS games,
                            count(*) FILTER (WHERE outcome = 'win') AS wins
                       FROM site.game_results WHERE user_id = $1
                      GROUP BY ended_at::date) j) AS game_xp",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// A line of the member search.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Found {
    pub id: i64,
    pub pseudo: String,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    /// Members matching, over every page.
    pub total: i64,
}

/// Members whose pseudo matches `pattern` (an `ILIKE` pattern, as Laravel's
/// `whereLike` on PostgreSQL), by pseudo.
pub async fn search(
    pool: &PgPool,
    pattern: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Found>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, pseudo, avatar_path, bio, count(*) OVER () AS total
           FROM site.members
          WHERE pseudo ILIKE $1
          ORDER BY pseudo, id
          LIMIT $2 OFFSET $3",
    )
    .bind(pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_matching(pool: &PgPool, pattern: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM site.members WHERE pseudo ILIKE $1")
        .bind(pattern)
        .fetch_one(pool)
        .await
}

/// A role as the profiles show it (`Role::pourAffichage`), with its owner.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ShownRole {
    pub user_id: i64,
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
    pub level: i16,
}

/// The roles of `user_ids`, highest permission first (Laravel's
/// `sortByDesc('niveau')`), then in display order.
pub async fn roles_of(pool: &PgPool, user_ids: &[i64]) -> Result<Vec<ShownRole>, sqlx::Error> {
    sqlx::query_as(
        "SELECT user_id, slug, name, category, color, permission_level AS level
           FROM site.member_roles
          WHERE user_id = ANY($1)
          ORDER BY user_id, permission_level DESC, display_order, slug",
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await
}
