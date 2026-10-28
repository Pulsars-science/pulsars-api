//! Rooms (`site.rooms`, `room_members`, `room_messages`, `room_invites`):
//! the rules of Laravel's `Salons` service and `Room`/`RoomInvite` models.
//!
//! `type` is `public` (listed, joined freely) or `prive` (unlisted, entered
//! by an invitation link only). A member's `role` in a room is
//! `proprietaire` (its creator) or `membre`.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub const PUBLIC: &str = "public";
pub const PRIVATE: &str = "prive";
pub const OWNER: &str = "proprietaire";
pub const MEMBER: &str = "membre";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Room {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub kind: String,
}

impl Room {
    pub fn is_public(&self) -> bool {
        self.kind == PUBLIC
    }
}

pub async fn find(pool: &PgPool, slug: &str) -> Result<Option<Room>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, nom AS name, slug, description, type AS kind FROM site.rooms WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// The role of `user_id` in `room_id`, if a member.
pub async fn role(
    pool: &PgPool,
    room_id: i64,
    user_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT role FROM site.room_members WHERE room_id = $1 AND user_id = $2")
        .bind(room_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

pub async fn member_count(pool: &PgPool, room_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM site.room_members WHERE room_id = $1")
        .bind(room_id)
        .fetch_one(pool)
        .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Listed {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub kind: String,
    pub member_count: i64,
    pub message_count: i64,
    pub joined: bool,
}

/// The rooms `user_id` sees in the list (`Room::visiblesPar`): every public
/// one, and the private ones they belong to. Busiest first.
pub async fn visible_to(pool: &PgPool, user_id: i64) -> Result<Vec<Listed>, sqlx::Error> {
    sqlx::query_as(
        "SELECT r.nom AS name, r.slug, r.description, r.type AS kind,
                (SELECT count(*) FROM site.room_members m WHERE m.room_id = r.id) AS member_count,
                (SELECT count(*) FROM site.room_messages x WHERE x.room_id = r.id) AS message_count,
                EXISTS (SELECT 1 FROM site.room_members m
                         WHERE m.room_id = r.id AND m.user_id = $1) AS joined
           FROM site.rooms r
          WHERE r.type = 'public'
             OR EXISTS (SELECT 1 FROM site.room_members m WHERE m.room_id = r.id AND m.user_id = $1)
          ORDER BY message_count DESC, r.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// `contenu.slugify(name)`, or `salon` when nothing is left of it.
pub async fn base_slug(pool: &PgPool, name: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT coalesce(nullif(contenu.slugify($1), ''), 'salon')")
        .bind(name)
        .fetch_one(pool)
        .await
}

/// Creates a room with `slug` and makes `creator` its owner. `None` when
/// the slug is taken.
pub async fn create(
    pool: &PgPool,
    creator: i64,
    name: &str,
    slug: &str,
    description: Option<&str>,
    kind: &str,
) -> Result<Option<Room>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let room: Option<Room> = sqlx::query_as(
        "INSERT INTO site.rooms (nom, slug, description, type, created_by, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (slug) DO NOTHING
         RETURNING id, nom AS name, slug, description, type AS kind",
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(kind)
    .bind(creator)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(room) = room else {
        return Ok(None);
    };
    sqlx::query(
        "INSERT INTO site.room_members (room_id, user_id, role, joined_at)
         VALUES ($1, $2, 'proprietaire', now() AT TIME ZONE 'UTC')",
    )
    .bind(room.id)
    .bind(creator)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(room))
}

/// Adds `user_id` to the room as a member; nothing if already in it.
/// Whether they joined now.
pub async fn join(pool: &PgPool, room_id: i64, user_id: i64) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO site.room_members (room_id, user_id, role, joined_at)
         VALUES ($1, $2, 'membre', now() AT TIME ZONE 'UTC')
         ON CONFLICT (room_id, user_id) DO NOTHING",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn leave(pool: &PgPool, room_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM site.room_members WHERE room_id = $1 AND user_id = $2")
        .bind(room_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Deletes the room; its members, messages and invitations go with it.
pub async fn delete(pool: &PgPool, room_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM site.rooms WHERE id = $1")
        .bind(room_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Clears the unread count of `user_id` in the room (`Salons::marquerLu`).
pub async fn mark_read(pool: &PgPool, room_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE site.room_members SET last_read_at = now() AT TIME ZONE 'UTC'
          WHERE room_id = $1 AND user_id = $2",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

// --- Messages ----------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub body: String,
    /// `None` once the author's account is deleted.
    pub author_id: Option<i64>,
    pub author_pseudo: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Which messages of a thread to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// The latest ones.
    Latest,
    /// The latest ones before this id: older history.
    Before(i64),
    /// The ones after this id, oldest first: what is new since a poll.
    After(i64),
}

/// Up to `limit` messages of the room, oldest first.
pub async fn messages(
    pool: &PgPool,
    room_id: i64,
    window: Window,
    limit: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let (condition, order, bound) = match window {
        Window::Latest => ("x.id < $3", "DESC", i64::MAX),
        Window::Before(id) => ("x.id < $3", "DESC", id),
        Window::After(id) => ("x.id > $3", "ASC", id),
    };
    let mut rows: Vec<Message> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT x.id, x.contenu AS body, x.user_id AS author_id, u.pseudo AS author_pseudo,
                x.created_at AT TIME ZONE 'UTC' AS created_at
           FROM site.room_messages x
           LEFT JOIN site.users u ON u.id = x.user_id
          WHERE x.room_id = $1 AND {condition}
          ORDER BY x.id {order}
          LIMIT $2"
    )))
    .bind(room_id)
    .bind(limit)
    .bind(bound)
    .fetch_all(pool)
    .await?;
    if order == "DESC" {
        rows.reverse();
    }
    Ok(rows)
}

pub async fn post(
    pool: &PgPool,
    room_id: i64,
    user_id: i64,
    body: &str,
) -> Result<Message, sqlx::Error> {
    sqlx::query_as(
        "WITH x AS (
            INSERT INTO site.room_messages (room_id, user_id, contenu, created_at)
            VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC')
            RETURNING id, contenu, user_id, created_at)
         SELECT x.id, x.contenu AS body, x.user_id AS author_id, u.pseudo AS author_pseudo,
                x.created_at AT TIME ZONE 'UTC' AS created_at
           FROM x JOIN site.users u ON u.id = x.user_id",
    )
    .bind(room_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(pool)
    .await
}

// --- Invitations -------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Invite {
    pub id: i64,
    pub room_id: i64,
    pub token: String,
    pub created_by: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<i32>,
    pub uses: i32,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Why an invitation no longer works (`RoomInvite::raisonInvalidite`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    Revoked,
    Expired,
    Exhausted,
}

impl Invalid {
    pub fn code(self) -> &'static str {
        match self {
            Self::Revoked => "revoked",
            Self::Expired => "expired",
            Self::Exhausted => "exhausted",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Revoked => "This invitation link was revoked.",
            Self::Expired => "This invitation link has expired.",
            Self::Exhausted => "This invitation link has reached its maximum number of uses.",
        }
    }
}

impl Invite {
    /// `None` while the link works.
    pub fn invalid(&self, now: DateTime<Utc>) -> Option<Invalid> {
        if self.revoked_at.is_some() {
            Some(Invalid::Revoked)
        } else if self.expires_at.is_some_and(|at| at <= now) {
            Some(Invalid::Expired)
        } else if self.max_uses.is_some_and(|max| self.uses >= max) {
            Some(Invalid::Exhausted)
        } else {
            None
        }
    }
}

const INVITE_COLUMNS: &str = "i.id, i.room_id, i.token, u.pseudo AS created_by,
    i.expires_at AT TIME ZONE 'UTC' AS expires_at, i.max_usages AS max_uses, i.usages AS uses,
    i.revoked_at AT TIME ZONE 'UTC' AS revoked_at";

/// The invitations of a room, newest first.
pub async fn invites(pool: &PgPool, room_id: i64) -> Result<Vec<Invite>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {INVITE_COLUMNS} FROM site.room_invites i
           LEFT JOIN site.users u ON u.id = i.created_by
          WHERE i.room_id = $1 ORDER BY i.id DESC"
    )))
    .bind(room_id)
    .fetch_all(pool)
    .await
}

pub async fn invite(pool: &PgPool, token: &str) -> Result<Option<Invite>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {INVITE_COLUMNS} FROM site.room_invites i
           LEFT JOIN site.users u ON u.id = i.created_by
          WHERE i.token = $1"
    )))
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn room_by_id(pool: &PgPool, room_id: i64) -> Result<Option<Room>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, nom AS name, slug, description, type AS kind FROM site.rooms WHERE id = $1",
    )
    .bind(room_id)
    .fetch_optional(pool)
    .await
}

/// A new invitation link; `None` if `token` is taken.
pub async fn create_invite(
    pool: &PgPool,
    room_id: i64,
    creator: i64,
    token: &str,
    days: Option<i32>,
    max_uses: Option<i32>,
) -> Result<Option<Invite>, sqlx::Error> {
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO site.room_invites
                (room_id, token, created_by, expires_at, max_usages, created_at, updated_at)
         VALUES ($1, $2, $3, now() AT TIME ZONE 'UTC' + make_interval(days => $4), $5,
                 now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (token) DO NOTHING
         RETURNING id",
    )
    .bind(room_id)
    .bind(token)
    .bind(creator)
    .bind(days)
    .bind(max_uses)
    .fetch_optional(pool)
    .await?;
    match id {
        Some(_) => invite(pool, token).await,
        None => Ok(None),
    }
}

pub async fn revoke_invite(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE site.room_invites
            SET revoked_at = coalesce(revoked_at, now() AT TIME ZONE 'UTC'),
                updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// What accepting an invitation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accepted {
    /// A new member: one use of the link counted.
    Joined,
    /// Already a member: no use counted (`Salons::accepterInvitation`).
    AlreadyMember,
    /// The link no longer works.
    Invalid(Invalid),
}

/// `user_id` accepts invitation `invite_id`. The link is locked while it is
/// checked and its use counted, in the transaction of the membership, so
/// two members racing for the last use cannot both get in.
pub async fn accept_invite(
    pool: &PgPool,
    invite_id: i64,
    user_id: i64,
) -> Result<Accepted, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let invite: Option<Invite> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {INVITE_COLUMNS} FROM site.room_invites i
           LEFT JOIN site.users u ON u.id = i.created_by
          WHERE i.id = $1 FOR UPDATE OF i"
    )))
    .bind(invite_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(check) = invite else {
        return Ok(Accepted::Invalid(Invalid::Revoked));
    };
    if let Some(reason) = check.invalid(Utc::now()) {
        return Ok(Accepted::Invalid(reason));
    }
    let joined = sqlx::query(
        "INSERT INTO site.room_members (room_id, user_id, role, joined_at)
         VALUES ($1, $2, 'membre', now() AT TIME ZONE 'UTC')
         ON CONFLICT (room_id, user_id) DO NOTHING",
    )
    .bind(check.room_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if joined {
        sqlx::query(
            "UPDATE site.room_invites SET usages = usages + 1, updated_at = now() AT TIME ZONE 'UTC'
              WHERE id = $1",
        )
        .bind(invite_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(if joined {
        Accepted::Joined
    } else {
        Accepted::AlreadyMember
    })
}
