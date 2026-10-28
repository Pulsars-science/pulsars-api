//! `GET /v1/members` (the search), `GET /v1/members/{pseudo}` (a public
//! profile) and `POST`/`DELETE /v1/members/{pseudo}/follow`, as Laravel's
//! `ProfilPublicController` and `AbonnementController`.
//!
//! A profile never shows an e-mail address, a locale, social logins or a
//! moderation state, to anyone. The brains balance shows only when the
//! member made it public, or to the member themselves.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Serialize;

use super::{avatar_url, data, not_found, timestamp};
use crate::auth::member::{MaybeMember, Verified};
use crate::db::friends::{self, Friendship};
use crate::db::members::{self, ShownRole, XpCounts};
use crate::error::ApiError;
use crate::links;
use crate::routes::params::Params;
use crate::routes::public::Page;
use crate::state::AppState;

/// Members per page of the search (`paginate(24)`).
const PER_PAGE: i64 = 24;
/// Longest search accepted.
const SEARCH_MAX_CHARS: usize = 100;

/// Points per action (`Experience::BAREME`, `PARTIE_JOUEE`), in the order
/// of Laravel's detail.
const POINTS: [(&str, i64); 7] = [
    ("lesson", 10),
    ("exercise", 8),
    ("course", 40),
    ("question", 4),
    ("answer", 4),
    ("accepted_answer", 12),
    ("games", 3),
];

#[derive(Debug, Serialize)]
pub struct Role {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub color: String,
    /// Permission level: 0 for a subject or a distinction.
    pub level: i16,
}

/// A member as the search lists them (`ProfilPublicController::resume`).
#[derive(Debug, Serialize)]
pub struct Summary {
    pub id: String,
    pub pseudo: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub roles: Vec<Role>,
}

#[derive(Debug, Serialize)]
pub struct Profile {
    #[serde(flatten)]
    pub summary: Summary,
    pub school_level: Option<String>,
    pub class: Option<String>,
    pub member_since: Option<String>,
    /// Forum questions and answers, deleted ones aside.
    pub question_count: i64,
    pub answer_count: i64,
    pub profile_url: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct XpLine {
    pub key: &'static str,
    pub count: i64,
    pub unit_points: i64,
    pub xp: i64,
}

/// `Experience::pour`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Xp {
    pub total: i64,
    pub level: i64,
    /// XP at which the current level starts.
    pub level_start: i64,
    /// XP at which the next level starts.
    pub next_level_at: i64,
    /// Progress inside the current level, for a bar.
    pub in_level: i64,
    pub for_level: i64,
    pub breakdown: Vec<XpLine>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Brains {
    pub balance: i64,
    /// Hidden from others: only its owner sees this.
    pub private: bool,
}

/// Where the reader stands with the member (`etatAmitie`).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FriendshipState {
    /// `visitor` (nobody signed in), `self`, `none`, `friends`, `sent`
    /// (the reader asked) or `received` (the member asked the reader).
    pub state: &'static str,
    /// The friendship, to accept or remove it.
    pub id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Follow {
    pub following: bool,
}

#[derive(Debug, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub profile: Profile,
    pub xp: Xp,
    /// `null` when hidden from the reader.
    pub brains: Option<Brains>,
    pub friendship: FriendshipState,
    /// `null` for a visitor or on one's own profile.
    pub following: Option<Follow>,
}

#[derive(Debug, Serialize)]
pub struct FollowResult {
    pub following: bool,
    pub friendship: FriendshipState,
}

/// `GET /v1/members?search=…&page=…`: members whose pseudo contains
/// `search` (or `q`), ignoring case. Without it, nobody: the site publishes no directory of
/// its members, one looks for someone one knows.
pub async fn index(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let search = search_term(query.as_deref())?;
    let mut params = Params::parse(query.as_deref());
    let page = params.page(false).unwrap_or(1);
    params.finish()?;
    let Some(search) = search else {
        return Ok(list(Vec::new(), 0));
    };
    let pattern = like_pattern(&search);
    let offset = i64::from(page - 1) * PER_PAGE;
    let rows = members::search(&state.db, &pattern, PER_PAGE, offset).await?;
    let total = match rows.first() {
        Some(row) => row.total,
        None => members::count_matching(&state.db, &pattern).await?,
    };
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let mut roles = members::roles_of(&state.db, &ids).await?;
    let storage = &state.config.public_storage_url;
    let items = rows
        .into_iter()
        .map(|row| Summary {
            id: row.id.to_string(),
            avatar_url: avatar_url(storage, row.avatar_path),
            roles: take_roles(&mut roles, row.id),
            pseudo: row.pseudo,
            bio: row.bio,
        })
        .collect();
    Ok(list(items, total))
}

fn list(items: Vec<Summary>, total: i64) -> Response {
    crate::routes::auth::no_store(
        StatusCode::OK,
        &Page {
            data: items,
            next_cursor: None,
            total: Some(total),
        },
    )
}

/// `search` (or `q`, as the content lists), trimmed; `None` when absent or
/// blank (Laravel's `trim(...) === ''`).
fn search_term(query: Option<&str>) -> Result<Option<String>, ApiError> {
    let raw = form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .find(|(name, _)| name == "search" || name == "q")
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    match raw {
        Some(q) if q.chars().count() > SEARCH_MAX_CHARS => Err(ApiError::InvalidParameter(vec![
            crate::error::ParameterError {
                parameter: "search".to_owned(),
                detail: format!("must be at most {SEARCH_MAX_CHARS} characters"),
                code: None,
            },
        ])),
        other => Ok(other),
    }
}

/// `%q%`, with `%`, `_` and `\` matched literally.
fn like_pattern(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('%');
    for c in value.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// The roles of `user_id`, removed from `roles` (sorted by user).
fn take_roles(roles: &mut Vec<ShownRole>, user_id: i64) -> Vec<Role> {
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(roles)
        .into_iter()
        .partition(|r| r.user_id == user_id);
    *roles = rest;
    mine.into_iter()
        .map(|r| Role {
            slug: r.slug,
            name: r.name,
            category: r.category,
            color: r.color,
            level: r.level,
        })
        .collect()
}

/// `GET /v1/members/{pseudo}`.
pub async fn show(
    State(state): State<AppState>,
    MaybeMember(reader): MaybeMember,
    Path(pseudo): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let profile = members::find_by_pseudo(&state.db, &pseudo)
        .await?
        .ok_or_else(|| no_member(&pseudo, uri.path()))?;
    let ids = [profile.id];
    let (counts, mut roles) = tokio::try_join!(
        members::xp_counts(&state.db, profile.id),
        members::roles_of(&state.db, &ids),
    )?;
    let reader_id = reader.as_ref().map(|m| m.id());
    let own = reader_id == Some(profile.id);

    let brains = (profile.brains_public || own).then(|| Brains {
        balance: i64::from(profile.brains),
        private: !profile.brains_public,
    });
    let (friendship, follow) = match reader_id {
        None => (
            FriendshipState {
                state: "visitor",
                id: None,
            },
            None,
        ),
        Some(_) if own => (
            FriendshipState {
                state: "self",
                id: None,
            },
            None,
        ),
        Some(reader_id) => {
            let (between, following) = tokio::try_join!(
                friends::between(&state.db, reader_id, profile.id),
                friends::follows(&state.db, reader_id, profile.id),
            )?;
            (
                friendship_state(between.as_ref(), reader_id),
                Some(Follow { following }),
            )
        }
    };

    let storage = &state.config.public_storage_url;
    let shown = Shown {
        profile: Profile {
            profile_url: links::member(&profile.pseudo),
            summary: Summary {
                id: profile.id.to_string(),
                avatar_url: avatar_url(storage, profile.avatar_path),
                roles: take_roles(&mut roles, profile.id),
                pseudo: profile.pseudo,
                bio: profile.bio,
            },
            school_level: profile.school_level,
            class: profile.class,
            member_since: timestamp(profile.member_since),
            question_count: profile.question_count,
            answer_count: profile.answer_count,
        },
        xp: xp(&counts),
        brains,
        friendship,
        following: follow,
    };
    Ok(data(StatusCode::OK, shown))
}

/// `POST /v1/members/{pseudo}/follow`. Following back makes friends.
/// Following oneself does nothing (`Abonnements::suivre`).
pub async fn follow(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(pseudo): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let target = member_id(&state, &pseudo, uri.path()).await?;
    if target != me.id() {
        friends::follow(&state.db, me.id(), target).await?;
    }
    follow_result(&state, me.id(), target).await
}

/// `DELETE /v1/members/{pseudo}/follow`. A friendship born of a mutual
/// follow goes too; one born of a request stays.
pub async fn unfollow(
    State(state): State<AppState>,
    Verified(me): Verified,
    Path(pseudo): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let target = member_id(&state, &pseudo, uri.path()).await?;
    friends::unfollow(&state.db, me.id(), target).await?;
    follow_result(&state, me.id(), target).await
}

async fn follow_result(state: &AppState, me: i64, target: i64) -> Result<Response, ApiError> {
    if me == target {
        return Ok(data(
            StatusCode::OK,
            FollowResult {
                following: false,
                friendship: FriendshipState {
                    state: "self",
                    id: None,
                },
            },
        ));
    }
    let (following, between) = tokio::try_join!(
        friends::follows(&state.db, me, target),
        friends::between(&state.db, me, target),
    )?;
    Ok(data(
        StatusCode::OK,
        FollowResult {
            following,
            friendship: friendship_state(between.as_ref(), me),
        },
    ))
}

/// The id of the member `pseudo`, or 404.
pub(crate) async fn member_id(state: &AppState, pseudo: &str, path: &str) -> Result<i64, ApiError> {
    members::id_by_pseudo(&state.db, pseudo)
        .await?
        .ok_or_else(|| no_member(pseudo, path))
}

fn no_member(pseudo: &str, path: &str) -> ApiError {
    not_found(format!("No member has the pseudo {pseudo:?}."), path)
}

pub(crate) fn friendship_state(friendship: Option<&Friendship>, reader: i64) -> FriendshipState {
    match friendship {
        None => FriendshipState {
            state: "none",
            id: None,
        },
        Some(f) => FriendshipState {
            state: if f.is_accepted() {
                "friends"
            } else if f.requested_by_id == reader {
                "sent"
            } else {
                "received"
            },
            id: Some(f.id),
        },
    }
}

/// The detail, total and level of `Experience::pour`. Level `n` starts at
/// `50 × (n − 1) × n` XP.
fn xp(counts: &XpCounts) -> Xp {
    let numbers = [
        counts.lessons,
        counts.exercises,
        counts.courses,
        counts.questions,
        counts.answers,
        counts.accepted_answers,
        counts.games,
    ];
    let breakdown: Vec<XpLine> = POINTS
        .iter()
        .zip(numbers)
        .map(|(&(key, points), count)| XpLine {
            key,
            count,
            unit_points: points,
            // Games are capped per day: their XP is not count × points.
            xp: if key == "games" {
                counts.game_xp
            } else {
                count * points
            },
        })
        .collect();
    let total: i64 = breakdown.iter().map(|line| line.xp).sum();
    let start = |n: i64| 50 * (n - 1) * n;
    let mut level = 1;
    while start(level + 1) <= total {
        level += 1;
    }
    Xp {
        total,
        level,
        level_start: start(level),
        next_level_at: start(level + 1),
        in_level: total - start(level),
        for_level: start(level + 1) - start(level),
        breakdown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_follows_the_laravel_scale() {
        let none = xp(&XpCounts::default());
        assert_eq!((none.total, none.level, none.next_level_at), (0, 1, 100));
        assert_eq!(none.breakdown.len(), 7);

        let counts = XpCounts {
            lessons: 10,
            exercises: 1,
            games: 4,
            game_xp: 30,
            ..XpCounts::default()
        };
        let x = xp(&counts);
        // 100 + 8 + 30 (capped, not 4 × 3).
        assert_eq!(x.total, 138);
        assert_eq!((x.level, x.level_start, x.next_level_at), (2, 100, 300));
        assert_eq!((x.in_level, x.for_level), (38, 200));
        assert_eq!(
            x.breakdown[6],
            XpLine {
                key: "games",
                count: 4,
                unit_points: 3,
                xp: 30
            }
        );
        let at = |total: i64| {
            xp(&XpCounts {
                lessons: total / 10,
                ..XpCounts::default()
            })
            .level
        };
        assert_eq!((at(90), at(100), at(290), at(300)), (1, 2, 2, 3));
    }

    #[test]
    fn the_search_term_is_trimmed_and_matched_literally() {
        assert_eq!(search_term(None).unwrap(), None);
        assert_eq!(search_term(Some("q=")).unwrap(), None);
        assert_eq!(search_term(Some("q=%20%20")).unwrap(), None);
        assert_eq!(
            search_term(Some("q=+cami+")).unwrap().as_deref(),
            Some("cami")
        );
        assert_eq!(
            search_term(Some("search=bo")).unwrap().as_deref(),
            Some("bo")
        );
        assert!(search_term(Some(&format!("q={}", "a".repeat(101)))).is_err());
        assert_eq!(like_pattern("a_b%c"), "%a\\_b\\%c%");
    }
}
