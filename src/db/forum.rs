//! The forum: themes, questions, answers, votes, reports, translations and
//! moderation, in Laravel's tables (`site.forum_themes`, `site.questions`,
//! `site.answers`, `site.forum_votes`, `site.signalements`,
//! `site.forum_translations`).
//!
//! Questions and answers are soft deleted (`deleted_at`): a deleted row is
//! never read here, except to restore it. Votes, reports and translations
//! name their target as Laravel's morph map does: `question` or `answer`.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Rows of `site.questions` that are not deleted.
const LIVE: &str = "q.deleted_at IS NULL";

/// What a vote, a report or a translation is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Question,
    Answer,
}

impl Target {
    /// The morph name Laravel stored.
    pub fn morph(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Answer => "answer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "question" => Some(Self::Question),
            "answer" => Some(Self::Answer),
            _ => None,
        }
    }
}

// --- Themes -----------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThemeRow {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub color: String,
    /// Questions not deleted, hidden ones included (Laravel's
    /// `withCount('questions')`).
    pub question_count: i64,
}

const THEME_COLUMNS: &str = "t.id, t.nom AS name, t.slug, t.description, t.couleur AS color,
    (SELECT count(*) FROM site.questions q
      WHERE q.forum_theme_id = t.id AND q.deleted_at IS NULL) AS question_count";

/// Every theme, in the forum's order.
pub async fn themes(pool: &PgPool) -> Result<Vec<ThemeRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {THEME_COLUMNS} FROM site.forum_themes t ORDER BY t.ordre, t.nom, t.id"
    )))
    .fetch_all(pool)
    .await
}

pub async fn theme(pool: &PgPool, slug: &str) -> Result<Option<ThemeRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {THEME_COLUMNS} FROM site.forum_themes t WHERE t.slug = $1"
    )))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

// --- Questions --------------------------------------------------------------

/// Order of the question list. Pinned questions always come first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Newest first (Laravel's `recent`).
    Recent,
    /// Most votes first, then newest (`populaire`).
    Popular,
    /// Only questions without an answer, newest first (`sans-reponse`).
    Unanswered,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuestionRow {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub user_id: Option<i64>,
    pub author_pseudo: Option<String>,
    pub theme_name: String,
    pub theme_slug: String,
    pub theme_color: String,
    pub votes: i64,
    pub answer_count: i64,
    pub views: i32,
    pub accepted_answer_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub hidden: bool,
    pub hidden_reason: Option<String>,
    pub locked: bool,
    pub pinned: bool,
    /// Whether the reader voted for it (false for a guest).
    pub voted: bool,
}

/// `body` is cut to `$body_chars` characters: a list only shows an excerpt.
fn question_select(body_chars: Option<u32>) -> String {
    let body = match body_chars {
        Some(chars) => format!("left(q.corps, {chars})"),
        None => "q.corps".to_owned(),
    };
    format!(
        "SELECT q.id, q.titre AS title, q.slug, {body} AS body, q.user_id,
                u.pseudo AS author_pseudo,
                t.nom AS theme_name, t.slug AS theme_slug, t.couleur AS theme_color,
                vq.n AS votes,
                (SELECT count(*) FROM site.answers a
                  WHERE a.question_id = q.id AND a.deleted_at IS NULL) AS answer_count,
                q.vues AS views, q.accepted_answer_id,
                q.created_at AT TIME ZONE 'UTC' AS created_at,
                q.updated_at AT TIME ZONE 'UTC' AS updated_at,
                q.masque_le IS NOT NULL AS hidden, q.masque_motif AS hidden_reason,
                q.verrouille_le IS NOT NULL AS locked, q.epingle_le IS NOT NULL AS pinned,
                EXISTS (SELECT 1 FROM site.forum_votes v
                         WHERE v.votable_type = 'question' AND v.votable_id = q.id
                           AND v.user_id = $1) AS voted
           FROM site.questions q
           JOIN site.forum_themes t ON t.id = q.forum_theme_id
           LEFT JOIN site.users u ON u.id = q.user_id
           LEFT JOIN LATERAL (SELECT count(*) AS n FROM site.forum_votes v
                               WHERE v.votable_type = 'question' AND v.votable_id = q.id) vq ON true"
    )
}

/// The filter of the list: parameter `$theme` is the theme id or null,
/// `$unanswered` whether to keep only questions without an answer.
fn list_filter(theme: u8, unanswered: u8) -> String {
    format!(
        "q.deleted_at IS NULL
         AND (${theme}::bigint IS NULL OR q.forum_theme_id = ${theme})
         AND (NOT ${unanswered} OR NOT EXISTS (SELECT 1 FROM site.answers a
                                    WHERE a.question_id = q.id AND a.deleted_at IS NULL))"
    )
}

/// One page of questions, for `viewer` (or a guest), with excerpts of
/// `body_chars` characters.
pub async fn questions(
    pool: &PgPool,
    viewer: Option<i64>,
    theme_id: Option<i64>,
    sort: Sort,
    limit: i64,
    offset: i64,
    body_chars: u32,
) -> Result<Vec<QuestionRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{} WHERE {}
          ORDER BY q.epingle_le IS NULL,
                   CASE WHEN $4 THEN vq.n END DESC NULLS LAST,
                   q.created_at DESC NULLS LAST, q.id DESC
          LIMIT $5 OFFSET $6",
        question_select(Some(body_chars)),
        list_filter(2, 3)
    )))
    .bind(viewer)
    .bind(theme_id)
    .bind(sort == Sort::Unanswered)
    .bind(sort == Sort::Popular)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// How many questions the list has, over every page.
pub async fn count_questions(
    pool: &PgPool,
    theme_id: Option<i64>,
    sort: Sort,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM site.questions q WHERE {}",
        list_filter(1, 2)
    )))
    .bind(theme_id)
    .bind(sort == Sort::Unanswered)
    .fetch_one(pool)
    .await
}

/// The question of `slug`, unless deleted.
pub async fn question(
    pool: &PgPool,
    viewer: Option<i64>,
    slug: &str,
) -> Result<Option<QuestionRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{} WHERE q.slug = $2 AND {LIVE}",
        question_select(None)
    )))
    .bind(viewer)
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// What the routes need of a question to act on it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuestionRef {
    pub id: i64,
    pub user_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub hidden: bool,
    pub locked: bool,
}

const REF_COLUMNS: &str = "q.id, q.user_id, q.titre AS title, q.corps AS body,
    q.masque_le IS NOT NULL AS hidden, q.verrouille_le IS NOT NULL AS locked";

pub async fn question_ref(pool: &PgPool, slug: &str) -> Result<Option<QuestionRef>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {REF_COLUMNS} FROM site.questions q WHERE q.slug = $1 AND {LIVE}"
    )))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn question_ref_by_id(
    pool: &PgPool,
    id: i64,
) -> Result<Option<QuestionRef>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {REF_COLUMNS} FROM site.questions q WHERE q.id = $1 AND {LIVE}"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Adds a question; `None` when the slug is already used (the caller draws
/// another one).
pub async fn create_question(
    pool: &PgPool,
    theme_id: i64,
    user_id: i64,
    title: &str,
    slug: &str,
    body: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO site.questions (forum_theme_id, user_id, titre, slug, corps, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (slug) DO NOTHING
         RETURNING id",
    )
    .bind(theme_id)
    .bind(user_id)
    .bind(title)
    .bind(slug)
    .bind(body)
    .fetch_optional(pool)
    .await
}

/// One more view.
pub async fn add_view(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE site.questions SET vues = vues + 1 WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Marks `answer` as the accepted answer of `question`, or unmarks it when
/// it already is. The accepted answer afterwards.
pub async fn toggle_accepted(
    pool: &PgPool,
    question: i64,
    answer: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE site.questions
            SET accepted_answer_id = CASE WHEN accepted_answer_id = $2 THEN NULL ELSE $2 END,
                updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1
         RETURNING accepted_answer_id",
    )
    .bind(question)
    .bind(answer)
    .fetch_one(pool)
    .await
}

// --- Answers ----------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AnswerRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub body: String,
    pub user_id: Option<i64>,
    pub author_pseudo: Option<String>,
    pub votes: i64,
    pub voted: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub hidden: bool,
    pub hidden_reason: Option<String>,
}

/// The answers of a question that are not deleted, oldest first.
pub async fn answers(
    pool: &PgPool,
    viewer: Option<i64>,
    question: i64,
) -> Result<Vec<AnswerRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id, a.parent_id, a.corps AS body, a.user_id, u.pseudo AS author_pseudo,
                (SELECT count(*) FROM site.forum_votes v
                  WHERE v.votable_type = 'answer' AND v.votable_id = a.id) AS votes,
                EXISTS (SELECT 1 FROM site.forum_votes v
                         WHERE v.votable_type = 'answer' AND v.votable_id = a.id
                           AND v.user_id = $1) AS voted,
                a.created_at AT TIME ZONE 'UTC' AS created_at,
                a.masque_le IS NOT NULL AS hidden, a.masque_motif AS hidden_reason
           FROM site.answers a
           LEFT JOIN site.users u ON u.id = a.user_id
          WHERE a.question_id = $2 AND a.deleted_at IS NULL
          ORDER BY a.created_at NULLS FIRST, a.id",
    )
    .bind(viewer)
    .bind(question)
    .fetch_all(pool)
    .await
}

/// What the routes need of an answer to act on it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AnswerRef {
    pub id: i64,
    pub question_id: i64,
    pub body: String,
    pub hidden: bool,
}

/// The answer `id`, unless it or its question is deleted.
pub async fn answer_ref(pool: &PgPool, id: i64) -> Result<Option<AnswerRef>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id, a.question_id, a.corps AS body, a.masque_le IS NOT NULL AS hidden
           FROM site.answers a
           JOIN site.questions q ON q.id = a.question_id
          WHERE a.id = $1 AND a.deleted_at IS NULL AND q.deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_answer(
    pool: &PgPool,
    question: i64,
    user_id: i64,
    parent_id: Option<i64>,
    body: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO site.answers (question_id, user_id, parent_id, corps, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(question)
    .bind(user_id)
    .bind(parent_id)
    .bind(body)
    .fetch_one(pool)
    .await
}

// --- Votes and reports ------------------------------------------------------

/// Adds the vote of `user_id`, or takes it back if there was one. Whether
/// the vote stands afterwards, and the target's votes.
pub async fn toggle_vote(
    pool: &PgPool,
    user_id: i64,
    target: Target,
    id: i64,
) -> Result<(bool, i64), sqlx::Error> {
    let voted: Option<i32> = sqlx::query_scalar(
        "WITH gone AS (
             DELETE FROM site.forum_votes
              WHERE user_id = $1 AND votable_type = $2 AND votable_id = $3
             RETURNING 1)
         INSERT INTO site.forum_votes (user_id, votable_type, votable_id)
         SELECT $1, $2, $3 WHERE NOT EXISTS (SELECT 1 FROM gone)
         ON CONFLICT (user_id, votable_type, votable_id) DO NOTHING
         RETURNING 1",
    )
    .bind(user_id)
    .bind(target.morph())
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let votes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM site.forum_votes WHERE votable_type = $1 AND votable_id = $2",
    )
    .bind(target.morph())
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok((voted.is_some(), votes))
}

/// A report for the moderators' queue (`statut` `nouveau`).
pub async fn create_report(
    pool: &PgPool,
    user_id: i64,
    target: Target,
    id: i64,
    kind: &str,
    message: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO site.signalements (signalable_type, signalable_id, user_id, type, message,
                                        created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         RETURNING id",
    )
    .bind(target.morph())
    .bind(id)
    .bind(user_id)
    .bind(kind)
    .bind(message)
    .fetch_one(pool)
    .await
}

// --- Translations -----------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TranslationRow {
    pub source_lang: Option<String>,
    pub target_lang: String,
    pub source_hash: String,
    pub title: Option<String>,
    pub body: String,
}

const TRANSLATION_COLUMNS: &str = "langue_source AS source_lang, langue_cible AS target_lang,
    source_hash, titre AS title, contenu AS body";

pub async fn translation(
    pool: &PgPool,
    target: Target,
    id: i64,
    lang: &str,
) -> Result<Option<TranslationRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TRANSLATION_COLUMNS} FROM site.forum_translations
          WHERE traduisible_type = $1 AND traduisible_id = $2 AND langue_cible = $3"
    )))
    .bind(target.morph())
    .bind(id)
    .bind(lang)
    .fetch_optional(pool)
    .await
}

/// Stores a translation, replacing the one of an older text.
pub async fn save_translation(
    pool: &PgPool,
    target: Target,
    id: i64,
    row: &TranslationRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site.forum_translations (traduisible_type, traduisible_id, langue_source,
                                              langue_cible, source_hash, titre, contenu,
                                              created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
         ON CONFLICT (traduisible_type, traduisible_id, langue_cible) DO UPDATE
            SET langue_source = EXCLUDED.langue_source, source_hash = EXCLUDED.source_hash,
                titre = EXCLUDED.titre, contenu = EXCLUDED.contenu,
                updated_at = EXCLUDED.updated_at",
    )
    .bind(target.morph())
    .bind(id)
    .bind(&row.source_lang)
    .bind(&row.target_lang)
    .bind(&row.source_hash)
    .bind(&row.title)
    .bind(&row.body)
    .execute(pool)
    .await?;
    Ok(())
}

// --- Moderation -------------------------------------------------------------

fn table(target: Target) -> &'static str {
    match target {
        Target::Question => "site.questions",
        Target::Answer => "site.answers",
    }
}

/// Whether the message is hidden; `None` when it does not exist or is
/// deleted.
pub async fn is_hidden(
    pool: &PgPool,
    target: Target,
    id: i64,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT masque_le IS NOT NULL FROM {} WHERE id = $1 AND deleted_at IS NULL",
        table(target)
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Hides the message, by `moderator`, for `reason`; `false` when it does
/// not exist or is deleted.
pub async fn hide(
    pool: &PgPool,
    target: Target,
    id: i64,
    moderator: i64,
    reason: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {} SET masque_le = now() AT TIME ZONE 'UTC', masque_par = $2, masque_motif = $3,
                       updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1 AND deleted_at IS NULL",
        table(target)
    )))
    .bind(id)
    .bind(moderator)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn unhide(pool: &PgPool, target: Target, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {} SET masque_le = NULL, masque_par = NULL, masque_motif = NULL,
                       updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1 AND deleted_at IS NULL",
        table(target)
    )))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// A question flag that is a date: `verrouille_le` or `epingle_le`.
#[derive(Debug, Clone, Copy)]
pub enum Flag {
    Locked,
    Pinned,
}

/// Sets the flag if it is not set, clears it otherwise. The flag
/// afterwards; `None` when the question does not exist or is deleted.
pub async fn toggle_flag(pool: &PgPool, id: i64, flag: Flag) -> Result<Option<bool>, sqlx::Error> {
    let column = match flag {
        Flag::Locked => "verrouille_le",
        Flag::Pinned => "epingle_le",
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "UPDATE site.questions
            SET {column} = CASE WHEN {column} IS NULL THEN now() AT TIME ZONE 'UTC' END,
                updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1 AND deleted_at IS NULL
         RETURNING {column} IS NOT NULL"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Soft deletes the message; `false` when it does not exist or is deleted
/// already.
pub async fn delete(pool: &PgPool, target: Target, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {} SET deleted_at = now() AT TIME ZONE 'UTC', updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1 AND deleted_at IS NULL",
        table(target)
    )))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Brings a soft deleted message back; `false` when there is no such
/// message, deleted or not.
pub async fn restore(pool: &PgPool, target: Target, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {} SET deleted_at = NULL, updated_at = now() AT TIME ZONE 'UTC' WHERE id = $1",
        table(target)
    )))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

// --- Muting -----------------------------------------------------------------

/// The highest permission level of account `id` (0 for none), or `None`
/// when there is no such account.
pub async fn permission_level(pool: &PgPool, id: i64) -> Result<Option<i16>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT max(r.niveau) FROM site.role_user ru
                            JOIN site.roles r ON r.id = ru.role_id
                           WHERE ru.user_id = u.id AND r.categorie = 'permission'), 0)::smallint
           FROM site.users u WHERE u.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Mutes `user_id` for `days` days, by `moderator`. The end of the mute.
pub async fn mute(
    pool: &PgPool,
    user_id: i64,
    days: i32,
    reason: &str,
    moderator: i64,
) -> Result<DateTime<Utc>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE site.users
            SET mute_jusqu_a = date_trunc('second', now() AT TIME ZONE 'UTC') + make_interval(days => $2),
                mute_motif = $3, mute_par = $4, updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1
         RETURNING mute_jusqu_a AT TIME ZONE 'UTC'",
    )
    .bind(user_id)
    .bind(days)
    .bind(reason)
    .bind(moderator)
    .fetch_one(pool)
    .await
}

/// Lifts the mute of `user_id`; `false` when there is no such account.
pub async fn unmute(pool: &PgPool, user_id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE site.users SET mute_jusqu_a = NULL, mute_motif = NULL, mute_par = NULL,
                               updated_at = now() AT TIME ZONE 'UTC'
          WHERE id = $1",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}
