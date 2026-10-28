//! The social part of the site (`openapi.yaml`, tags `members`, `friends`,
//! `rooms` and `conversations`): the member directory and public profiles,
//! follows, friends, rooms and private messages, as Laravel's
//! `ProfilPublicController`, `AbonnementController`, `AmitieController`,
//! `AmiQrController`, `Salon*Controller`, `InvitationSalonController`,
//! `DiscussionController` and `MessagePriveController` did.
//!
//! Called by the site (pulsars-web) from its server, with the member's
//! `Pulsars-Session`. Profiles and the directory are public (`MaybeMember`);
//! everything else needs a verified member who finished the welcome steps
//! (`Verified`), as Laravel's `auth`, `onboarding` and `verified`
//! middlewares. Answers are `no-store`: they change with the reader.
//!
//! Rooms and conversations are polled, as the Laravel pages did every five
//! seconds: `GET …/messages?after={id}` gives what is new since the last
//! message shown.

pub mod conversations;
pub mod friends;
pub mod members;
pub mod rooms;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::Response;
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::auth::{Data, no_store};
use super::params::{MAX_LIMIT, Params};
use super::public;
use crate::config::SITE_URL;
use crate::db::rooms::Window;
use crate::error::{ApiError, ParameterError};
use crate::links::encode;
use crate::rate_limit;
use crate::state::AppState;

/// The routes of this module.
pub fn router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/members", get(members::index))
        .route("/v1/members/{pseudo}", get(members::show))
        .route(
            "/v1/members/{pseudo}/follow",
            post(members::follow)
                .put(members::follow)
                .delete(members::unfollow),
        )
        .route(
            "/v1/members/{pseudo}/friend-requests",
            post(friends::request),
        )
        .route("/v1/friends", get(friends::index))
        .route("/v1/me/friend-qr", get(friends::qr))
        .route("/v1/me/friend-qr/regenerate", post(friends::renew_qr))
        .route("/v1/friends/scan", post(friends::scan))
        .route("/v1/friends/{id}", axum::routing::delete(friends::remove))
        .route("/v1/friends/{id}/accept", post(friends::accept))
        .route("/v1/rooms", get(rooms::index).post(rooms::create))
        .route("/v1/rooms/{slug}", get(rooms::show).delete(rooms::destroy))
        .route("/v1/rooms/{slug}/members", post(rooms::join))
        .route(
            "/v1/rooms/{slug}/members/me",
            axum::routing::delete(rooms::leave),
        )
        .route("/v1/rooms/{slug}/read", post(rooms::read))
        .route(
            "/v1/rooms/{slug}/messages",
            get(rooms::messages).post(rooms::post),
        )
        .route(
            "/v1/rooms/{slug}/invitations",
            get(rooms::invitations).post(rooms::invite),
        )
        .route(
            "/v1/rooms/{slug}/invitations/{token}",
            axum::routing::delete(rooms::revoke),
        )
        .route("/v1/invitations/{token}", get(rooms::invitation))
        .route("/v1/invitations/{token}/accept", post(rooms::accept))
        .route(
            "/v1/conversations",
            get(conversations::index).post(conversations::open),
        )
        .route("/v1/conversations/{id}", get(conversations::show))
        .route("/v1/conversations/{id}/read", post(conversations::read))
        .route(
            "/v1/conversations/{id}/messages",
            get(conversations::messages).post(conversations::post),
        )
        .layer(from_fn(public::require_json))
        .layer(from_fn_with_state(state.clone(), rate_limit::rate_limit))
}

// --- Shapes shared by the modules ---------------------------------------------

/// Another member, as lists show them.
#[derive(Debug, Serialize)]
pub struct MemberRef {
    pub id: String,
    pub pseudo: String,
    pub avatar_url: Option<String>,
}

/// The `author` (pseudo) and `author_id` of a message; both `null` once
/// the account is deleted.
pub(crate) fn author(id: Option<i64>, pseudo: Option<String>) -> (Option<String>, Option<String>) {
    match (id, pseudo) {
        (Some(id), Some(pseudo)) => (Some(pseudo), Some(id.to_string())),
        _ => (None, None),
    }
}

/// `{"data": [...], "has_more": …}`: messages of a thread, oldest first, and
/// whether more lie beyond them in the direction asked.
#[derive(Debug, Serialize)]
pub struct Thread<T> {
    pub data: Vec<T>,
    pub has_more: bool,
}

/// The URL of a file of Laravel's `public` disk.
pub(crate) fn avatar_url(storage_url: &str, path: Option<String>) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| format!("{storage_url}/{}", encode(p.trim_start_matches('/'), true)))
}

pub(crate) fn timestamp(at: Option<DateTime<Utc>>) -> Option<String> {
    at.map(super::courses::timestamp)
}

/// `{"data": …}`, `no-store`.
pub(crate) fn data<T: Serialize>(status: StatusCode, data: T) -> Response {
    no_store(status, &Data { data })
}

pub(crate) fn not_found(detail: impl Into<String>, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: detail.into(),
        instance: Some(path.to_owned()),
    }
}

pub(crate) fn forbidden(detail: impl Into<String>, path: &str) -> ApiError {
    ApiError::Forbidden {
        detail: detail.into(),
        instance: Some(path.to_owned()),
    }
}

pub(crate) fn invalid(field: &str, code: &'static str, detail: &str) -> ApiError {
    ApiError::InvalidFields(vec![ParameterError::field(field, code, detail)])
}

/// A numeric id of the path (Laravel's `whereNumber`); anything else names
/// nothing.
pub(crate) fn path_id(raw: &str) -> Option<i64> {
    if raw.is_empty() || raw.len() > 18 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok().filter(|id| *id > 0)
}

/// Laravel's `TrimStrings` then `ConvertEmptyStringsToNull`.
pub(crate) fn trimmed(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// A message body: `required|string|max:2000`, after trimming.
pub(crate) fn message_body(body: Option<String>) -> Result<String, ApiError> {
    match trimmed(body) {
        None => Err(invalid("body", "required", "is required")),
        Some(body) if body.chars().count() > MESSAGE_MAX_CHARS => Err(invalid(
            "body",
            "too_long",
            "must be at most 2000 characters",
        )),
        Some(body) => Ok(body),
    }
}

pub(crate) const MESSAGE_MAX_CHARS: usize = 2000;

/// Messages a thread page shows, as Laravel's 100 latest.
pub(crate) const THREAD_LIMIT: u32 = 100;

/// `after`, `before` and `limit` of a thread: the latest messages by
/// default, older ones with `before`, newer ones with `after` (polling).
pub(crate) fn window(query: Option<&str>) -> Result<(Window, i64), ApiError> {
    let mut params = Params::parse(query);
    let after = params.text("after", 20);
    let before = params.text("before", 20);
    let limit = params.integer("limit", 1, MAX_LIMIT, THREAD_LIMIT);
    let mut errors = match params.finish() {
        Ok(_) => Vec::new(),
        Err(ApiError::InvalidParameter(errors)) => errors,
        Err(error) => return Err(error),
    };
    let mut id = |name: &str, raw: Option<String>| {
        let raw = raw?;
        let id = raw.parse::<i64>().ok().filter(|id| *id >= 0);
        if id.is_none() {
            errors.push(ParameterError {
                parameter: name.to_owned(),
                detail: "must be a message id".to_owned(),
                code: None,
            });
        }
        id
    };
    let after = id("after", after);
    let before = id("before", before);
    if after.is_some() && before.is_some() {
        errors.push(ParameterError {
            parameter: "before".to_owned(),
            detail: "cannot be combined with `after`".to_owned(),
            code: None,
        });
    }
    if !errors.is_empty() {
        return Err(ApiError::InvalidParameter(errors));
    }
    let window = match (after, before) {
        (Some(after), _) => Window::After(after),
        (None, Some(before)) => Window::Before(before),
        (None, None) => Window::Latest,
    };
    Ok((window, i64::from(limit)))
}

/// Cuts the `limit + 1` rows fetched for `window` to `limit`, and tells
/// whether more lay beyond.
pub(crate) fn cut<T>(mut rows: Vec<T>, window: Window, limit: i64) -> (Vec<T>, bool) {
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let more = rows.len() > limit;
    if more {
        match window {
            // Oldest first: the extra row is the oldest.
            Window::Latest | Window::Before(_) => {
                rows.drain(..rows.len() - limit);
            }
            Window::After(_) => rows.truncate(limit),
        }
    }
    (rows, more)
}

/// `n` random characters of `alphabet`.
pub(crate) fn random_token(n: usize, alphabet: &[u8]) -> Result<String, ApiError> {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness for a token: {e}")))?;
    // 256 is a multiple of neither 36 nor 62: reject the bytes past the last
    // whole multiple, so every character is equally likely.
    let len = alphabet.len();
    let ceiling = 256 - 256 % len;
    let mut out = String::with_capacity(n);
    let mut pool = bytes;
    while out.len() < n {
        for &b in &pool {
            if usize::from(b) < ceiling && out.len() < n {
                out.push(char::from(alphabet[usize::from(b) % len]));
            }
        }
        if out.len() < n {
            pool = vec![0u8; n];
            getrandom::fill(&mut pool)
                .map_err(|e| ApiError::Internal(format!("no randomness for a token: {e}")))?;
        }
    }
    Ok(out)
}

pub(crate) const LOWER_ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
pub(crate) const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// `https://pulsars.science/ami/{token}`: what the friend QR code encodes.
pub(crate) fn friend_link(token: &str) -> String {
    format!("{SITE_URL}/ami/{}", encode(token, false))
}

/// `https://pulsars.science/salons/invite/{token}`.
pub(crate) fn invite_link(token: &str) -> String {
    format!("{SITE_URL}/salons/invite/{}", encode(token, false))
}

/// Case-insensitive natural order (PHP's `SORT_NATURAL | SORT_FLAG_CASE`):
/// `ana2` before `ana10`.
pub(crate) fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut a), take(&mut b));
                let (xt, yt) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = xt.len().cmp(&yt.len()).then_with(|| xt.cmp(yt));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_ids_are_positive_decimals() {
        assert_eq!(path_id("42"), Some(42));
        for bad in ["", "0", "-1", "4a", "1e3", "99999999999999999999"] {
            assert_eq!(path_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn bodies_are_trimmed_and_bounded() {
        assert_eq!(message_body(Some("  salut ".into())).unwrap(), "salut");
        assert!(message_body(Some("   ".into())).is_err());
        assert!(message_body(None).is_err());
        assert!(message_body(Some("é".repeat(2000))).is_ok());
        assert!(message_body(Some("é".repeat(2001))).is_err());
    }

    #[test]
    fn windows_read_after_before_and_limit() {
        assert_eq!(window(None).unwrap(), (Window::Latest, 100));
        assert_eq!(
            window(Some("after=12&limit=5")).unwrap(),
            (Window::After(12), 5)
        );
        assert_eq!(window(Some("before=7")).unwrap(), (Window::Before(7), 100));
        for bad in [
            "after=x",
            "before=-3",
            "after=1&before=2",
            "limit=0",
            "limit=101",
        ] {
            assert!(window(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn cut_keeps_the_rows_next_to_the_anchor() {
        assert_eq!(cut(vec![1, 2, 3], Window::Latest, 2), (vec![2, 3], true));
        assert_eq!(cut(vec![1, 2, 3], Window::After(0), 2), (vec![1, 2], true));
        assert_eq!(cut(vec![1, 2], Window::Before(9), 2), (vec![1, 2], false));
    }

    #[test]
    fn tokens_use_their_alphabet() {
        let token = random_token(24, LOWER_ALNUM).unwrap();
        assert_eq!(token.len(), 24);
        assert!(token.bytes().all(|b| LOWER_ALNUM.contains(&b)));
        assert_ne!(token, random_token(24, LOWER_ALNUM).unwrap());
    }

    #[test]
    fn natural_order_ignores_case_and_reads_numbers() {
        let mut names = vec!["bob", "Ana10", "ana2", "Zoé", "alice"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["alice", "ana2", "Ana10", "bob", "Zoé"]);
    }
}
