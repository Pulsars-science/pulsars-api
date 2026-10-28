//! The member's dashboard, brains and the XP leaderboard (`openapi.yaml`,
//! tag `dashboard`), as Laravel's `TableauDeBordController`,
//! `ClassementController` and `Experience::meilleurs` (the home page's
//! best members).

use std::collections::HashMap;

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Serialize;
use serde_json::{Value, json};

use super::auth::no_store;
use super::courses::timestamp;
use super::cursor;
use super::params::{Lang, Params};
use super::public::{Page, Pagination, translated};
use super::site::{self, avatar_url, ok};
use crate::auth::member::{Member, Verified};
use crate::db::brains::{self, Movement};
use crate::db::learning::percent;
use crate::db::members::MemberRole;
use crate::error::ApiError;
use crate::state::AppState;

/// Movements shown on the dashboard.
const MOVEMENTS: i64 = 20;
/// Course suggestions on the dashboard.
const SUGGESTIONS: i64 = 4;

#[derive(Debug, Serialize)]
pub struct Dashboard {
    pub in_progress: Vec<CourseCard>,
    pub completed: Vec<CourseCard>,
    pub stats: Stats,
    pub brains: Brains,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Serialize)]
pub struct CourseCard {
    /// The course's own title, as Laravel showed it (not translated).
    pub title: String,
    pub slug: String,
    pub difficulty: String,
    pub progress: Gauge,
    /// The last lesson completed (`derniere_lesson_id`), to resume at.
    pub resume_lesson: Option<ResumeLesson>,
}

#[derive(Debug, Serialize)]
pub struct Gauge {
    pub done: i64,
    pub total: i64,
    pub percent: i64,
}

#[derive(Debug, Serialize)]
pub struct ResumeLesson {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub courses_started: i64,
    pub courses_completed: i64,
    /// Distinct exercises tried, and solved at least once.
    pub exercises_attempted: i64,
    pub exercises_solved: i64,
}

#[derive(Debug, Serialize)]
pub struct Brains {
    pub balance: i64,
    pub movements: Vec<BrainMovement>,
}

#[derive(Debug, Serialize)]
pub struct BrainMovement {
    pub id: i64,
    pub date: String,
    /// `lecon`, `partie`, `victoire`, `saison`…
    pub reason: String,
    pub amount: i32,
    /// The lesson's title in `lang`, while the lesson exists.
    pub lesson_title: Option<String>,
    pub season: Option<String>,
    pub subject: Option<String>,
    pub rank: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub title: String,
    pub slug: String,
    pub description: String,
    pub difficulty: String,
}

#[derive(sqlx::FromRow)]
struct CardRow {
    title: String,
    slug: String,
    difficulty: String,
    completed: bool,
    done: i64,
    total: i64,
    resume_slug: Option<String>,
    resume_title: Option<String>,
}

/// `GET /v1/me/dashboard?lang=`.
pub async fn dashboard(
    State(state): State<AppState>,
    Verified(member): Verified,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let lang = site::lang(query.as_deref())?;
    let id = member.id();
    let cards: Vec<CardRow> = sqlx::query_as(
        "SELECT c.titre::text AS title, c.slug::text AS slug, c.difficulte::text AS difficulty,
                (en.completed_at IS NOT NULL) AS completed,
                (SELECT count(*) FROM site.lesson_progress lp
                   JOIN contenu.lessons l ON l.id = lp.lesson_id
                   JOIN contenu.chapters ch ON ch.id = l.chapter_id
                  WHERE lp.user_id = $1 AND ch.course_id = c.id) AS done,
                (SELECT count(*) FROM contenu.lessons l
                   JOIN contenu.chapters ch ON ch.id = l.chapter_id
                  WHERE ch.course_id = c.id) AS total,
                dl.slug::text AS resume_slug, dl.titre::text AS resume_title
           FROM site.enrollments en
           JOIN contenu.courses c ON c.id = en.course_id
           LEFT JOIN contenu.lessons dl ON dl.id = en.derniere_lesson_id
          WHERE en.user_id = $1
          ORDER BY en.updated_at DESC NULLS LAST, en.id DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let (attempted, solved): (i64, i64) = sqlx::query_as(
        "SELECT count(DISTINCT exercise_id), count(DISTINCT exercise_id) FILTER (WHERE correct)
           FROM site.exercise_attempts WHERE user_id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let balance = crate::db::members::brains(&state.db, id).await?;
    let movements = brains::history(&state.db, id, None, MOVEMENTS).await?;
    let suggestions = crate::db::learning::suggestions(&state.db, id, SUGGESTIONS).await?;
    let started = i64::try_from(cards.len()).unwrap_or(0);
    let (completed, in_progress): (Vec<CardRow>, Vec<CardRow>) =
        cards.into_iter().partition(|c| c.completed);
    let to_card = |c: CardRow| CourseCard {
        progress: Gauge {
            done: c.done,
            total: c.total,
            percent: percent(c.done, c.total),
        },
        resume_lesson: c
            .resume_slug
            .zip(c.resume_title)
            .map(|(slug, title)| ResumeLesson { slug, title }),
        title: c.title,
        slug: c.slug,
        difficulty: c.difficulty,
    };
    Ok(ok(Dashboard {
        stats: Stats {
            courses_started: started,
            courses_completed: i64::try_from(completed.len()).unwrap_or(0),
            exercises_attempted: attempted,
            exercises_solved: solved,
        },
        in_progress: in_progress.into_iter().map(to_card).collect(),
        completed: completed.into_iter().map(to_card).collect(),
        brains: Brains {
            balance: i64::from(balance),
            movements: movements_of(&state, movements, lang).await?,
        },
        suggestions: suggestions
            .into_iter()
            .map(|(title, slug, description, difficulty)| Suggestion {
                title,
                slug,
                description,
                difficulty,
            })
            .collect(),
    }))
}

/// Movements with the title of their lesson, in one query for all.
async fn movements_of(
    state: &AppState,
    movements: Vec<Movement>,
    lang: Lang,
) -> Result<Vec<BrainMovement>, ApiError> {
    let detail = |m: &Movement, key: &str| -> Option<Value> {
        m.details.as_ref().and_then(|d| d.0.get(key).cloned())
    };
    let lesson_ids: Vec<i64> = movements
        .iter()
        .filter_map(|m| detail(m, "lesson_id").and_then(|v| v.as_i64()))
        .collect();
    let titles: HashMap<i64, String> = if lesson_ids.is_empty() {
        HashMap::new()
    } else {
        let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
            "SELECT l.id, l.titre::text,
                    (SELECT nullif(t.champs::jsonb ->> 'titre', '')
                       FROM contenu.contenu_translations t
                      WHERE t.traduisible_type = 'lesson' AND t.traduisible_id = l.id
                        AND t.langue = $2)
               FROM contenu.lessons l WHERE l.id = ANY($1)",
        )
        .bind(&lesson_ids)
        .bind(lang.translation())
        .fetch_all(&state.db)
        .await?;
        rows.into_iter()
            .map(|(id, title, tr)| (id, translated(title, tr).0))
            .collect()
    };
    Ok(movements
        .into_iter()
        .map(|m| BrainMovement {
            lesson_title: detail(&m, "lesson_id")
                .and_then(|v| v.as_i64())
                .and_then(|id| titles.get(&id).cloned()),
            season: detail(&m, "season").and_then(|v| v.as_str().map(str::to_owned)),
            subject: detail(&m, "subject").and_then(|v| v.as_str().map(str::to_owned)),
            rank: detail(&m, "rank").and_then(|v| v.as_i64()),
            id: m.id,
            date: timestamp(m.created_at),
            reason: m.reason,
            amount: m.amount,
        })
        .collect())
}

/// `GET /v1/me/brains?limit=&cursor=&lang=`: the movements, newest first.
pub async fn brains_history(
    State(state): State<AppState>,
    member: Member,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    let limit = params.limit();
    let cursor = params.cursor();
    let filters = params.finish()?;
    let pagination = Pagination::new("me-brains", lang, &filters, limit);
    let after = match cursor {
        Some(c) => Some(cursor::time_id_key(&cursor::decode(
            &c,
            &pagination.fingerprint,
        )?)?),
        None => None,
    };
    let rows = brains::history(&state.db, member.id(), after, pagination.fetch()).await?;
    let more = rows.len() > limit as usize;
    let mut rows = rows;
    rows.truncate(limit as usize);
    let next_cursor = rows
        .last()
        .filter(|_| more)
        .map(|m| cursor::encode(cursor::time_id(m.created_at, m.id), &pagination.fingerprint));
    let data = movements_of(&state, rows, lang).await?;
    Ok(no_store(
        StatusCode::OK,
        &Page {
            data,
            next_cursor,
            total: None,
        },
    ))
}

// --- Leaderboard -------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Line {
    pub rank: i64,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub xp: i64,
    pub level: i64,
    pub followers: i64,
    pub followed: bool,
    pub me: bool,
    pub roles: Vec<Role>,
}

#[derive(Debug, Serialize)]
pub struct Role {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
}

#[derive(Debug, Serialize)]
struct Leaderboard {
    data: Vec<Line>,
    next_cursor: Option<String>,
    /// The member's own line, wherever it is.
    me: Line,
}

#[derive(sqlx::FromRow)]
struct LineRow {
    id: i64,
    rank: i64,
    pseudo: String,
    avatar: Option<String>,
    xp: i64,
    level: i64,
    followers: i64,
    followed: bool,
}

/// Every account by XP (most first), then pseudo; `$1` the reader.
macro_rules! ranked {
    () => {
        "WITH ranked AS (
             SELECT u.id, u.pseudo::text AS pseudo, u.avatar::text AS avatar,
                    x.total::bigint AS xp, x.level::bigint AS level,
                    row_number() OVER (ORDER BY x.total DESC, u.pseudo COLLATE \"C\") AS rank
               FROM site.users u JOIN site.member_xp x ON x.user_id = u.id)
         SELECT r.id, r.rank, r.pseudo, r.avatar, r.xp, r.level,
                (SELECT count(*) FROM site.abonnements a WHERE a.suivi_id = r.id) AS followers,
                EXISTS (SELECT 1 FROM site.abonnements a
                         WHERE a.abonne_id = $1 AND a.suivi_id = r.id) AS followed
           FROM ranked r"
    };
}

/// `GET /v1/leaderboard/xp?limit=&cursor=`: Laravel's `/classement`, every
/// member, XP 0 included.
pub async fn leaderboard(
    State(state): State<AppState>,
    Verified(member): Verified,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let limit = params.limit_or(50);
    let cursor = params.cursor();
    let filters = params.finish()?;
    let pagination = Pagination::new("leaderboard-xp", Lang::Fr, &filters, limit);
    let after = match cursor {
        Some(c) => cursor::id_key(&cursor::decode(&c, &pagination.fingerprint)?)?,
        None => 0,
    };
    let mut rows: Vec<LineRow> = sqlx::query_as(concat!(
        ranked!(),
        " WHERE r.rank > $2 ORDER BY r.rank LIMIT $3"
    ))
    .bind(member.id())
    .bind(after)
    .bind(pagination.fetch())
    .fetch_all(&state.db)
    .await?;
    let mine: LineRow = sqlx::query_as(concat!(ranked!(), " WHERE r.id = $1"))
        .bind(member.id())
        .fetch_one(&state.db)
        .await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = rows
        .last()
        .filter(|_| more)
        .map(|r| cursor::encode(vec![json!(r.rank)], &pagination.fingerprint));
    let mut ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    ids.push(mine.id);
    let roles = roles_of(&state, &ids).await?;
    let line = |r: LineRow, roles: &HashMap<i64, Vec<MemberRole>>| Line {
        rank: r.rank,
        avatar_url: avatar_url(&state, r.avatar.as_deref()),
        pseudo: r.pseudo,
        xp: r.xp,
        level: r.level,
        followers: r.followers,
        followed: r.followed,
        me: r.id == member.id(),
        roles: roles
            .get(&r.id)
            .map(|list| {
                list.iter()
                    .map(|role| Role {
                        slug: role.slug.clone(),
                        name: role.name.clone(),
                        category: role.category.clone(),
                        color: role.color.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    Ok(no_store(
        StatusCode::OK,
        &Leaderboard {
            data: rows.into_iter().map(|r| line(r, &roles)).collect(),
            next_cursor,
            me: line(mine, &roles),
        },
    ))
}

async fn roles_of(
    state: &AppState,
    ids: &[i64],
) -> Result<HashMap<i64, Vec<MemberRole>>, ApiError> {
    let rows: Vec<(i64, String, String, String, String)> = sqlx::query_as(
        "SELECT user_id, slug::text, name::text, category::text, color::text
           FROM site.member_roles WHERE user_id = ANY($1)
          ORDER BY user_id, permission_level DESC, display_order, slug",
    )
    .bind(ids)
    .fetch_all(&state.db)
    .await?;
    let mut roles: HashMap<i64, Vec<MemberRole>> = HashMap::new();
    for (user_id, slug, name, category, color) in rows {
        roles.entry(user_id).or_default().push(MemberRole {
            slug,
            name,
            category,
            color,
        });
    }
    Ok(roles)
}

#[derive(Debug, Serialize)]
pub struct Best {
    pub rank: i64,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub xp: i64,
    pub level: i64,
}

/// `GET /v1/leaderboard/top?limit=5`: the home page's best members, those
/// with some XP only (`Experience::meilleurs`). Public: pseudo, avatar, XP
/// and level are public on every member's page.
pub async fn top(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let mut params = Params::parse(query.as_deref());
    let limit = params.integer("limit", 1, 10, 5);
    params.finish()?;
    let rows: Vec<(String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT u.pseudo::text, u.avatar::text, x.total::bigint, x.level::bigint
           FROM site.users u JOIN site.member_xp x ON x.user_id = u.id
          WHERE x.total > 0
          ORDER BY x.total DESC, u.pseudo COLLATE \"C\"
          LIMIT $1",
    )
    .bind(i64::from(limit))
    .fetch_all(&state.db)
    .await?;
    Ok(ok(rows
        .into_iter()
        .zip(1..)
        .map(|((pseudo, avatar, xp, level), rank)| Best {
            rank,
            avatar_url: avatar_url(&state, avatar.as_deref()),
            pseudo,
            xp,
            level,
        })
        .collect::<Vec<_>>()))
}

/// `GET /v1/rewards`: what actions earn (`Experience::BAREME`,
/// `Cerveaux`), for the "+8 XP" of the exercise cards.
pub async fn rewards() -> Response {
    ok(json!({
        "xp": {
            "lesson": super::learning::LESSON_XP,
            "exercise": super::practice::EXERCISE_XP,
            "course": 40, "question": 4, "answer": 4, "accepted_answer": 12,
            "game": 3, "game_win": 5, "game_daily_cap": 30,
        },
        "brains": {
            "lesson": brains::LESSON, "game": brains::MATCH, "victory": brains::WIN,
            "game_daily_cap": brains::MATCH_DAILY_CAP,
        },
    }))
}
