//! The admin journal (`/v1/admin/people/*`, `/v1/admin/activity`): people
//! found by e-mail, pseudo or visitor id, their summary, and their
//! timeline, merged from the action tables and `crate::journal`'s.
//!
//! Admin only: titles are read from `contenu` whatever the status, and a
//! room message is only ever its room's name, never its content.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// One kind of timeline item: where its rows are and how to read them.
/// Every field is SQL written here, never input.
struct Source {
    kind: &'static str,
    from: &'static str,
    at: &'static str,
    id: &'static str,
    user: &'static str,
    /// `None`: the rows have no visitor id.
    visitor: Option<&'static str>,
    filter: &'static str,
    data: &'static str,
    link: &'static str,
}

const SOURCES: &[Source] = &[
    Source {
        kind: "page_view",
        from: "site.page_views p",
        at: "p.at",
        id: "p.id",
        user: "p.user_id",
        visitor: Some("p.visitor_id"),
        filter: "true",
        data: "jsonb_build_object('path', p.path, 'referrer', p.referrer)",
        link: "p.path",
    },
    Source {
        kind: "security",
        from: "site.security_events s",
        at: "s.at",
        id: "s.id",
        user: "s.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('event', s.kind, 'ip', s.ip, 'user_agent', s.user_agent, 'detail', s.detail)",
        link: "NULL::text",
    },
    Source {
        kind: "consent",
        from: "site.consents c",
        at: "c.at",
        id: "c.id",
        user: "c.user_id",
        visitor: Some("c.visitor_id"),
        filter: "true",
        data: "jsonb_build_object('measure', c.measure, 'version', c.version)",
        link: "NULL::text",
    },
    Source {
        kind: "signup",
        from: "site.users u",
        at: "(u.created_at AT TIME ZONE 'UTC')",
        id: "u.id",
        user: "u.id",
        visitor: None,
        filter: "u.created_at IS NOT NULL",
        data: "'{}'::jsonb",
        link: "NULL::text",
    },
    Source {
        kind: "lesson_completed",
        from: "site.lesson_progress lp
               JOIN contenu.lessons l ON l.id = lp.lesson_id
               JOIN contenu.chapters ch ON ch.id = l.chapter_id
               JOIN contenu.courses c ON c.id = ch.course_id",
        at: "(lp.completed_at AT TIME ZONE 'UTC')",
        id: "lp.id",
        user: "lp.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('course_title', c.titre, 'lesson_title', l.titre)",
        link: "'/cours/' || c.slug || '/lecon/' || l.slug",
    },
    Source {
        kind: "exercise_attempt",
        from: "site.exercise_attempts a
               JOIN contenu.exercises e ON e.id = a.exercise_id
               JOIN contenu.chapters ch ON ch.id = e.chapter_id
               JOIN contenu.courses c ON c.id = ch.course_id",
        at: "(a.created_at AT TIME ZONE 'UTC')",
        id: "a.id",
        user: "a.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('course_title', c.titre, 'exercise_title', e.titre, 'correct', a.correct)",
        link: "'/cours/' || c.slug || '/exercice/' || e.id",
    },
    Source {
        kind: "course_enrolled",
        from: "site.enrollments en JOIN contenu.courses c ON c.id = en.course_id",
        at: "(en.started_at AT TIME ZONE 'UTC')",
        id: "en.id",
        user: "en.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('course_title', c.titre)",
        link: "'/cours/' || c.slug",
    },
    Source {
        kind: "game",
        from: "site.game_results g LEFT JOIN game.matches gm ON gm.id = g.match_id",
        at: "(g.ended_at AT TIME ZONE 'UTC')",
        id: "g.id",
        user: "g.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('mode', g.mode, 'subject', gm.subject, 'outcome', g.outcome, 'ranked', g.ranked)",
        link: "NULL::text",
    },
    Source {
        kind: "forum_question",
        from: "site.questions q",
        at: "(q.created_at AT TIME ZONE 'UTC')",
        id: "q.id",
        user: "q.user_id",
        visitor: None,
        filter: "q.created_at IS NOT NULL",
        data: "jsonb_build_object('title', q.titre)",
        link: "'/forum/q/' || q.slug",
    },
    Source {
        kind: "forum_answer",
        from: "site.answers fa JOIN site.questions q ON q.id = fa.question_id",
        at: "(fa.created_at AT TIME ZONE 'UTC')",
        id: "fa.id",
        user: "fa.user_id",
        visitor: None,
        filter: "fa.created_at IS NOT NULL",
        data: "jsonb_build_object('question_title', q.titre)",
        link: "'/forum/q/' || q.slug",
    },
    Source {
        kind: "room_message",
        // Never `rm.contenu`.
        from: "site.room_messages rm JOIN site.rooms r ON r.id = rm.room_id",
        at: "(rm.created_at AT TIME ZONE 'UTC')",
        id: "rm.id",
        user: "rm.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('room_name', r.nom)",
        link: "'/salons/' || r.slug",
    },
    Source {
        kind: "brains",
        from: "site.cerveaux_mouvements b",
        at: "(b.created_at AT TIME ZONE 'UTC')",
        id: "b.id",
        user: "b.user_id",
        visitor: None,
        filter: "true",
        data: "jsonb_build_object('amount', b.montant, 'reason', b.raison)",
        link: "NULL::text",
    },
    Source {
        kind: "download",
        from: "contenu.telechargements t JOIN contenu.exams x ON x.id = t.telechargeable_id",
        at: "(t.created_at AT TIME ZONE 'UTC')",
        id: "t.id",
        user: "t.user_id",
        visitor: None,
        filter: "t.telechargeable_type = 'exam'",
        data: "jsonb_build_object('exam_title', x.titre)",
        link: "'/examens/' || x.slug",
    },
];

/// Every kind of timeline item, in the order of [`SOURCES`].
pub fn kinds() -> Vec<&'static str> {
    SOURCES.iter().map(|s| s.kind).collect()
}

/// Whose timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Member(i64),
    Visitor(Uuid),
    Everybody,
}

impl Scope {
    /// The kinds this timeline can have: a visitor's are page views and
    /// consents only.
    pub fn kinds(self) -> Vec<&'static str> {
        SOURCES
            .iter()
            .filter(|s| !matches!(self, Self::Visitor(_)) || s.visitor.is_some())
            .map(|s| s.kind)
            .collect()
    }
}

/// Where a page of the timeline starts: after this item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct After {
    pub at: DateTime<Utc>,
    pub kind: String,
    pub id: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventRow {
    pub kind: String,
    pub id: i64,
    pub at: DateTime<Utc>,
    pub user_id: Option<i64>,
    pub visitor_id: Option<Uuid>,
    pub data: sqlx::types::Json<Value>,
    pub link: Option<String>,
    pub pseudo: Option<String>,
}

/// The query of a timeline page over `kinds` (all of [`Scope::kinds`]
/// when empty): `$1`–`$3` the cursor (null for the first page), `$4` the
/// number of rows, `$5` the member or visitor.
fn timeline_sql(scope: Scope, kinds: &[&str]) -> String {
    let branches: Vec<String> = SOURCES
        .iter()
        .filter(|s| kinds.is_empty() || kinds.contains(&s.kind))
        .filter_map(|s| {
            let scoped = match scope {
                Scope::Member(_) => format!("{} = $5", s.user),
                Scope::Visitor(_) => format!("{} = $5", s.visitor?),
                Scope::Everybody => "true".to_owned(),
            };
            Some(format!(
                "(SELECT '{kind}'::text AS kind, {id}::bigint AS id, {at} AS at,
                         {user}::bigint AS user_id, {visitor}::uuid AS visitor_id,
                         {data} AS data, {link}::text AS link
                    FROM {from}
                   WHERE {filter} AND {scoped}
                     AND ($1::timestamptz IS NULL
                          OR ({at}, '{kind}'::text, {id}::bigint) < ($1, $2::text, $3::bigint))
                   ORDER BY {at} DESC, {id} DESC
                   LIMIT $4)",
                kind = s.kind,
                id = s.id,
                at = s.at,
                user = s.user,
                visitor = s.visitor.unwrap_or("NULL"),
                data = s.data,
                link = s.link,
                from = s.from,
                filter = s.filter,
            ))
        })
        .collect();
    if branches.is_empty() {
        return String::new();
    }
    format!(
        "SELECT e.kind, e.id, e.at, e.user_id, e.visitor_id, e.data, e.link, m.pseudo::text AS pseudo
           FROM ({}) e
           LEFT JOIN site.users m ON m.id = e.user_id
          ORDER BY e.at DESC, e.kind DESC, e.id DESC
          LIMIT $4",
        branches.join("\n UNION ALL\n")
    )
}

/// Up to `fetch` items of a timeline, newest first, after `after`.
pub async fn timeline(
    pool: &PgPool,
    scope: Scope,
    kinds: &[&str],
    after: Option<&After>,
    fetch: i64,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let sql = timeline_sql(scope, kinds);
    if sql.is_empty() {
        return Ok(Vec::new());
    }
    // Built from this module's constants only.
    let query = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(sql))
        .bind(after.map(|a| a.at))
        .bind(after.map(|a| a.kind.clone()))
        .bind(after.map(|a| a.id))
        .bind(fetch);
    let query = match scope {
        Scope::Member(id) => query.bind(id),
        Scope::Visitor(id) => query.bind(id),
        Scope::Everybody => query,
    };
    query.fetch_all(pool).await
}

/// When a member was last seen: their latest page view or session use.
const LAST_SEEN: &str = "GREATEST(
        (SELECT max(p.at) FROM site.page_views p WHERE p.user_id = u.id),
        (SELECT max(s.last_used_at) FROM site.auth_sessions s WHERE s.user_id = u.id))";

const ROLES: &str = "ARRAY(SELECT r.slug::text FROM site.role_user ru
                       JOIN site.roles r ON r.id = ru.role_id
                      WHERE ru.user_id = u.id ORDER BY r.slug)";

#[derive(Debug, sqlx::FromRow)]
pub struct FoundMember {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub verified: bool,
    pub roles: Vec<String>,
}

/// `\`, `%` and `_` taken literally in a `LIKE` pattern.
fn like_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Members whose e-mail or pseudo contains `q` (case-insensitive), those
/// that start with it first; 20 at most.
pub async fn search_members(pool: &PgPool, q: &str) -> Result<Vec<FoundMember>, sqlx::Error> {
    let escaped = like_escape(&q.to_lowercase());
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT u.id, u.pseudo::text AS pseudo, u.email::text AS email,
                u.created_at AT TIME ZONE 'UTC' AS created_at, {LAST_SEEN} AS last_seen_at,
                u.email_verified_at IS NOT NULL AS verified, {ROLES} AS roles
           FROM site.users u
          WHERE lower(u.pseudo) LIKE $1 OR lower(u.email) LIKE $1
          ORDER BY (lower(u.pseudo) LIKE $2 OR lower(u.email) LIKE $2) DESC,
                   lower(u.pseudo), u.id
          LIMIT 20"
    )))
    .bind(format!("%{escaped}%"))
    .bind(format!("{escaped}%"))
    .fetch_all(pool)
    .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct VisitorRow {
    pub visitor_id: Uuid,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub page_views: i64,
    pub member_id: Option<i64>,
}

/// A visitor id seen in page views or consents: when, how many page views,
/// and the member it was linked to.
pub async fn visitor(pool: &PgPool, id: Uuid) -> Result<Option<VisitorRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT $1::uuid AS visitor_id, min(v.at) AS first_seen_at, max(v.at) AS last_seen_at,
                count(*) FILTER (WHERE v.page)::bigint AS page_views,
                max(v.user_id) FILTER (WHERE v.page) AS member_id
           FROM (SELECT at, user_id, true AS page FROM site.page_views WHERE visitor_id = $1
                 UNION ALL
                 SELECT at, user_id, false FROM site.consents WHERE visitor_id = $1) v
         HAVING count(*) > 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct MemberRow {
    pub id: i64,
    pub pseudo: String,
    pub email: String,
    pub created_at: Option<DateTime<Utc>>,
    pub verified: bool,
    pub onboarded: bool,
    pub roles: Vec<String>,
    pub xp: i64,
    pub brains: i64,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub visitor_ids: Vec<Uuid>,
    pub page_views: i64,
    pub lessons_completed: i64,
    pub exercise_attempts: i64,
    pub games: i64,
    pub forum_posts: i64,
    pub room_messages: i64,
    pub downloads: i64,
    pub logins: i64,
}

/// A member's summary and counts, ever.
pub async fn member(pool: &PgPool, id: i64) -> Result<Option<MemberRow>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT u.id, u.pseudo::text AS pseudo, u.email::text AS email,
                u.created_at AT TIME ZONE 'UTC' AS created_at,
                u.email_verified_at IS NOT NULL AS verified,
                u.onboarding_at IS NOT NULL AS onboarded, {ROLES} AS roles,
                coalesce((SELECT x.total FROM site.member_xp x WHERE x.user_id = u.id), 0)::bigint AS xp,
                coalesce((SELECT b.balance FROM site.member_brains b WHERE b.user_id = u.id), 0)::bigint
                    AS brains,
                {LAST_SEEN} AS last_seen_at,
                ARRAY(SELECT v.visitor_id FROM (
                          SELECT visitor_id, max(at) AS at FROM site.page_views
                           WHERE user_id = u.id AND visitor_id IS NOT NULL GROUP BY 1
                          UNION
                          SELECT visitor_id, max(at) FROM site.consents
                           WHERE user_id = u.id AND visitor_id IS NOT NULL GROUP BY 1) v
                       GROUP BY v.visitor_id ORDER BY max(v.at) DESC LIMIT 20) AS visitor_ids,
                (SELECT count(*) FROM site.page_views WHERE user_id = u.id) AS page_views,
                (SELECT count(*) FROM site.lesson_progress WHERE user_id = u.id) AS lessons_completed,
                (SELECT count(*) FROM site.exercise_attempts WHERE user_id = u.id) AS exercise_attempts,
                (SELECT count(*) FROM site.game_results WHERE user_id = u.id) AS games,
                (SELECT count(*) FROM site.questions WHERE user_id = u.id)
                  + (SELECT count(*) FROM site.answers WHERE user_id = u.id) AS forum_posts,
                (SELECT count(*) FROM site.room_messages WHERE user_id = u.id) AS room_messages,
                (SELECT count(*) FROM contenu.telechargements WHERE user_id = u.id) AS downloads,
                (SELECT count(*) FROM site.security_events
                  WHERE user_id = u.id AND kind IN ('login', 'oauth_login')) AS logins
           FROM site.users u WHERE u.id = $1"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// The latest consent answer of a member or a visitor.
pub async fn latest_consent(
    pool: &PgPool,
    member: Option<i64>,
    visitor: Option<Uuid>,
) -> Result<Option<(bool, DateTime<Utc>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT measure, at FROM site.consents
          WHERE ($1::bigint IS NOT NULL AND user_id = $1) OR ($2::uuid IS NOT NULL AND visitor_id = $2)
          ORDER BY at DESC, id DESC LIMIT 1",
    )
    .bind(member)
    .bind(visitor)
    .fetch_optional(pool)
    .await
}

/// Whether member `id` exists.
pub async fn member_exists(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM site.users WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visitor_timeline_has_page_views_and_consents_only() {
        assert_eq!(
            Scope::Visitor(Uuid::nil()).kinds(),
            ["page_view", "consent"]
        );
        assert_eq!(Scope::Everybody.kinds().len(), SOURCES.len());
        let sql = timeline_sql(Scope::Visitor(Uuid::nil()), &[]);
        assert!(sql.contains("p.visitor_id = $5"));
        assert!(!sql.contains("room_messages"));
        assert!(timeline_sql(Scope::Visitor(Uuid::nil()), &["game"]).is_empty());
    }

    #[test]
    fn room_messages_never_carry_their_content() {
        let sql = timeline_sql(Scope::Everybody, &["room_message"]);
        assert!(sql.contains("room_name"));
        assert!(!sql.contains("contenu,") && !sql.contains("rm.contenu"));
        assert!(!sql.contains("$5"));
    }

    #[test]
    fn like_patterns_are_literal() {
        assert_eq!(like_escape("a_b%c\\"), "a\\_b\\%c\\\\");
    }
}
