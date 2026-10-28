//! Private conversations (`site.direct_conversations`, `direct_messages`):
//! the rules of Laravel's `DiscussionController` and
//! `MessagePriveController`. One conversation per pair, smallest id first.
//!
//! Bodies are stored sealed (`crate::crypto`); this module never sees them
//! in clear.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::friends::pair;
use super::rooms::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct Conversation {
    pub id: i64,
    pub user_one_id: i64,
    pub user_two_id: i64,
}

impl Conversation {
    pub fn contains(&self, user_id: i64) -> bool {
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

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<Conversation>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, user_one_id, user_two_id FROM site.direct_conversations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// The conversation of `a` and `b`, created if needed; whether it was.
pub async fn open(pool: &PgPool, a: i64, b: i64) -> Result<(i64, bool), sqlx::Error> {
    let (one, two) = pair(a, b);
    let created: Option<i64> = sqlx::query_scalar(
        "INSERT INTO site.direct_conversations (user_one_id, user_two_id, created_at, updated_at)
         VALUES ($1, $2, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (user_one_id, user_two_id) DO NOTHING
         RETURNING id",
    )
    .bind(one)
    .bind(two)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = created {
        return Ok((id, true));
    }
    let id = sqlx::query_scalar(
        "SELECT id FROM site.direct_conversations WHERE user_one_id = $1 AND user_two_id = $2",
    )
    .bind(one)
    .bind(two)
    .fetch_one(pool)
    .await?;
    Ok((id, false))
}

/// A conversation in the list, with the other member and its last message.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Listed {
    pub id: i64,
    pub pseudo: String,
    pub avatar_path: Option<String>,
    pub last_body: Option<String>,
    pub last_key_version: Option<i16>,
    pub last_sender_id: Option<i64>,
    pub last_at: Option<DateTime<Utc>>,
    /// Messages of the other member not read yet.
    pub unread: i64,
}

/// The conversations of `user_id`, most recent message first (Laravel's
/// `orderByDesc('last_message_at')->orderByDesc('updated_at')`).
pub async fn of(pool: &PgPool, user_id: i64) -> Result<Vec<Listed>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.id, m.pseudo, m.avatar_path,
                last.contenu AS last_body, last.key_version AS last_key_version,
                last.sender_id AS last_sender_id,
                last.created_at AT TIME ZONE 'UTC' AS last_at,
                (SELECT count(*) FROM site.direct_messages d
                  WHERE d.direct_conversation_id = c.id AND d.read_at IS NULL
                    AND d.sender_id IS DISTINCT FROM $1) AS unread
           FROM site.direct_conversations c
           JOIN site.members m
             ON m.id = CASE WHEN c.user_one_id = $1 THEN c.user_two_id ELSE c.user_one_id END
           LEFT JOIN LATERAL (
                SELECT d.contenu, d.key_version, d.sender_id, d.created_at
                  FROM site.direct_messages d
                 WHERE d.direct_conversation_id = c.id
                 ORDER BY d.id DESC LIMIT 1) last ON true
          WHERE c.user_one_id = $1 OR c.user_two_id = $1
          ORDER BY c.last_message_at DESC, c.updated_at DESC, c.id DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Marks as read what the other member sent to `reader`.
pub async fn mark_read(
    pool: &PgPool,
    conversation_id: i64,
    reader: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE site.direct_messages
            SET read_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
          WHERE direct_conversation_id = $1 AND read_at IS NULL
            AND sender_id IS DISTINCT FROM $2",
    )
    .bind(conversation_id)
    .bind(reader)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    /// Sealed.
    pub body: String,
    pub key_version: i16,
    pub sender_id: Option<i64>,
    pub sender_pseudo: Option<String>,
    pub read: bool,
    pub created_at: Option<DateTime<Utc>>,
}

const MESSAGE_COLUMNS: &str = "d.id, d.contenu AS body, d.key_version, d.sender_id,
    u.pseudo AS sender_pseudo, d.read_at IS NOT NULL AS read,
    d.created_at AT TIME ZONE 'UTC' AS created_at";

/// Up to `limit` messages of the conversation, oldest first.
pub async fn messages(
    pool: &PgPool,
    conversation_id: i64,
    window: Window,
    limit: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let (condition, order, bound) = match window {
        Window::Latest => ("d.id < $3", "DESC", i64::MAX),
        Window::Before(id) => ("d.id < $3", "DESC", id),
        Window::After(id) => ("d.id > $3", "ASC", id),
    };
    let mut rows: Vec<Message> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS}
           FROM site.direct_messages d
           LEFT JOIN site.users u ON u.id = d.sender_id
          WHERE d.direct_conversation_id = $1 AND {condition}
          ORDER BY d.id {order}
          LIMIT $2"
    )))
    .bind(conversation_id)
    .bind(limit)
    .bind(bound)
    .fetch_all(pool)
    .await?;
    if order == "DESC" {
        rows.reverse();
    }
    Ok(rows)
}

/// Stores a sealed message and moves the conversation up the list.
pub async fn post(
    pool: &PgPool,
    conversation_id: i64,
    sender: i64,
    sealed: &str,
    key_version: i16,
) -> Result<Message, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO site.direct_messages
                (direct_conversation_id, sender_id, contenu, key_version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(conversation_id)
    .bind(sender)
    .bind(sealed)
    .bind(key_version)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE site.direct_conversations
            SET last_message_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1",
    )
    .bind(conversation_id)
    .execute(&mut *tx)
    .await?;
    let message = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS}
           FROM site.direct_messages d
           LEFT JOIN site.users u ON u.id = d.sender_id
          WHERE d.id = $1"
    )))
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(message)
}

/// The friends of `user_id` one can write to: pseudo and avatar.
pub async fn friends(
    pool: &PgPool,
    user_id: i64,
) -> Result<Vec<(String, Option<String>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT m.pseudo, m.avatar_path
           FROM site.friendships f
           JOIN site.members m
             ON m.id = CASE WHEN f.user_one_id = $1 THEN f.user_two_id ELSE f.user_one_id END
          WHERE (f.user_one_id = $1 OR f.user_two_id = $1) AND f.accepted_at IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
