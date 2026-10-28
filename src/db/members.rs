//! Member profiles, from `site.members`, `site.member_roles`,
//! `site.member_xp` and `site.member_brains`
//! (`docs/contract/database.md` §4, `docs/contract/brains.md` §4).
//!
//! Only ever read by id: the API has no member directory.

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
