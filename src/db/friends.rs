//! Follows (`site.abonnements`), friendships (`site.friendships`) and the
//! friend QR token (`site.users.friend_token`): the rules of Laravel's
//! `Abonnements` service, `AmitieController` and `AmiQrController`.
//!
//! A friendship is stored once per pair, smallest id first
//! (`Friendship::paire`). It is pending until `accepted_at` is set.
//! `origine` says how it was made: `demande` (a request, or a QR code) or
//! `abonnement` (two members following each other), which unfollowing undoes.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};

/// `(smallest, largest)`.
pub fn pair(a: i64, b: i64) -> (i64, i64) {
    if a < b { (a, b) } else { (b, a) }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Friendship {
    pub id: i64,
    pub user_one_id: i64,
    pub user_two_id: i64,
    pub requested_by_id: i64,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

impl Friendship {
    pub fn is_accepted(&self) -> bool {
        self.accepted_at.is_some()
    }

    pub fn concerns(&self, user_id: i64) -> bool {
        self.user_one_id == user_id || self.user_two_id == user_id
    }

    pub fn other_than(&self, user_id: i64) -> i64 {
        if self.user_one_id == user_id {
            self.user_two_id
        } else {
            self.user_one_id
        }
    }
}

const COLUMNS: &str = "id, user_one_id, user_two_id, requested_by_id,
    accepted_at AT TIME ZONE 'UTC' AS accepted_at, created_at AT TIME ZONE 'UTC' AS created_at";

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<Friendship>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM site.friendships WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn between(pool: &PgPool, a: i64, b: i64) -> Result<Option<Friendship>, sqlx::Error> {
    let (one, two) = pair(a, b);
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM site.friendships WHERE user_one_id = $1 AND user_two_id = $2"
    )))
    .bind(one)
    .bind(two)
    .fetch_optional(pool)
    .await
}

/// Whether `a` and `b` are friends (an accepted friendship).
pub async fn are_friends(pool: &PgPool, a: i64, b: i64) -> Result<bool, sqlx::Error> {
    Ok(between(pool, a, b).await?.is_some_and(|f| f.is_accepted()))
}

/// `requester` asks `other` to be friends. The friendship of the pair, and
/// whether this request created it (Laravel's `firstOrCreate`).
pub async fn request(
    pool: &PgPool,
    requester: i64,
    other: i64,
) -> Result<(Friendship, bool), sqlx::Error> {
    let (one, two) = pair(requester, other);
    let created: Option<Friendship> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO site.friendships (user_one_id, user_two_id, requested_by_id, created_at, updated_at)
         VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_one_id, user_two_id) DO NOTHING
         RETURNING {COLUMNS}"
    )))
    .bind(one)
    .bind(two)
    .bind(requester)
    .fetch_optional(pool)
    .await?;
    match created {
        Some(friendship) => Ok((friendship, true)),
        None => {
            let existing = between(pool, one, two)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            Ok((existing, false))
        }
    }
}

/// Accepts friendship `id`; an accepted one stays as it is.
pub async fn accept(pool: &PgPool, id: i64) -> Result<Option<Friendship>, sqlx::Error> {
    sqlx::query(
        "UPDATE site.friendships
            SET accepted_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1 AND accepted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    find(pool, id).await
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM site.friendships WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `scanner` scanned the QR code of `owner`: they are friends at once, a
/// pending request of either side included. The friendship, and whether it
/// was accepted already.
pub async fn befriend(
    pool: &PgPool,
    scanner: i64,
    owner: i64,
) -> Result<(Friendship, bool), sqlx::Error> {
    let (one, two) = pair(scanner, owner);
    let changed: Option<i64> = sqlx::query_scalar(
        "INSERT INTO site.friendships
                (user_one_id, user_two_id, requested_by_id, accepted_at, created_at, updated_at)
         VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_one_id, user_two_id) DO UPDATE
            SET accepted_at = EXCLUDED.accepted_at, updated_at = EXCLUDED.updated_at
          WHERE site.friendships.accepted_at IS NULL
         RETURNING id",
    )
    .bind(one)
    .bind(two)
    .bind(scanner)
    .fetch_optional(pool)
    .await?;
    let friendship = between(pool, one, two)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    Ok((friendship, changed.is_none()))
}

/// A friendship as the friends page lists it, with the other member.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Listed {
    pub id: i64,
    pub requested_by_id: i64,
    pub accepted: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub other_id: i64,
    pub pseudo: String,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
}

/// Every friendship of `user_id`, pending ones included, newest first
/// (Laravel's `latest()`).
pub async fn of(pool: &PgPool, user_id: i64) -> Result<Vec<Listed>, sqlx::Error> {
    sqlx::query_as(
        "SELECT f.id, f.requested_by_id, f.accepted_at IS NOT NULL AS accepted,
                f.created_at AT TIME ZONE 'UTC' AS created_at,
                m.id AS other_id, m.pseudo, m.avatar_path, m.bio
           FROM site.friendships f
           JOIN site.members m
             ON m.id = CASE WHEN f.user_one_id = $1 THEN f.user_two_id ELSE f.user_one_id END
          WHERE f.user_one_id = $1 OR f.user_two_id = $1
          ORDER BY f.created_at DESC NULLS LAST, f.id DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

// --- Follows ---------------------------------------------------------------

pub async fn follows(pool: &PgPool, follower: i64, followed: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.abonnements WHERE abonne_id = $1 AND suivi_id = $2)",
    )
    .bind(follower)
    .bind(followed)
    .fetch_one(pool)
    .await
}

/// `follower` follows `followed` (`Abonnements::suivre`). Following back
/// makes them friends; a pending request between them is accepted. `false`
/// when nothing changed (already following).
pub async fn follow(pool: &PgPool, follower: i64, followed: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO site.abonnements (abonne_id, suivi_id, created_at, updated_at)
         VALUES ($1, $2, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (abonne_id, suivi_id) DO NOTHING",
    )
    .bind(follower)
    .bind(followed)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if inserted && follows_in(&mut tx, followed, follower).await? {
        let (one, two) = pair(follower, followed);
        sqlx::query(
            "INSERT INTO site.friendships
                    (user_one_id, user_two_id, requested_by_id, accepted_at, origine,
                     created_at, updated_at)
             VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC', 'abonnement',
                     now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
             ON CONFLICT (user_one_id, user_two_id) DO UPDATE
                SET accepted_at = EXCLUDED.accepted_at, updated_at = EXCLUDED.updated_at
              WHERE site.friendships.accepted_at IS NULL",
        )
        .bind(one)
        .bind(two)
        .bind(follower)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(inserted)
}

/// `follower` stops following `followed` (`Abonnements::nePlusSuivre`): a
/// friendship born of the mutual follow goes too, one born of a request
/// stays. `false` when there was nothing to undo.
pub async fn unfollow(pool: &PgPool, follower: i64, followed: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let deleted =
        sqlx::query("DELETE FROM site.abonnements WHERE abonne_id = $1 AND suivi_id = $2")
            .bind(follower)
            .bind(followed)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            > 0;
    if deleted {
        let (one, two) = pair(follower, followed);
        sqlx::query(
            "DELETE FROM site.friendships
              WHERE user_one_id = $1 AND user_two_id = $2 AND origine = 'abonnement'",
        )
        .bind(one)
        .bind(two)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(deleted)
}

async fn follows_in(
    tx: &mut Transaction<'_, Postgres>,
    follower: i64,
    followed: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM site.abonnements WHERE abonne_id = $1 AND suivi_id = $2)",
    )
    .bind(follower)
    .bind(followed)
    .fetch_one(&mut **tx)
    .await
}

// --- QR token --------------------------------------------------------------

/// The friend token of `user_id`, set to `fresh` if it had none
/// (`User::jetonAmi`).
pub async fn token(pool: &PgPool, user_id: i64, fresh: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE site.users SET friend_token = coalesce(nullif(friend_token, ''), $2)
          WHERE id = $1 RETURNING friend_token",
    )
    .bind(user_id)
    .bind(fresh)
    .fetch_one(pool)
    .await
}

/// Replaces the friend token of `user_id` (`User::regenererJetonAmi`): the
/// previous QR code stops working.
pub async fn renew_token(pool: &PgPool, user_id: i64, fresh: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE site.users SET friend_token = $2 WHERE id = $1")
        .bind(user_id)
        .bind(fresh)
        .execute(pool)
        .await?;
    Ok(())
}

/// The member whose QR code carries `token`: id, pseudo and avatar path.
pub async fn owner_of_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<(i64, String, Option<String>)>, sqlx::Error> {
    sqlx::query_as("SELECT id, pseudo, avatar FROM site.users WHERE friend_token = $1")
        .bind(token)
        .fetch_optional(pool)
        .await
}
