//! `POST /v1/telemetry/page-view` and `POST /v1/consent`: what the site's
//! consent banner allows (`docs/privacy.md`, `crate::journal`).
//!
//! The site sends a page view only after the visitor accepted the "mesure"
//! cookies, with its `Pulsars-Visitor` id or a member's session; without
//! either nothing is written, whatever the site sends. Robots are ignored.
//! Both answer 204, `no-store`.

use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;

use super::auth::{Caller, empty};
use super::site;
use crate::auth::member::MaybeMember;
use crate::config::SITE_URL;
use crate::error::{ApiError, ParameterError};
use crate::extract::Json;
use crate::journal;
use crate::state::AppState;

/// The longest path or referrer kept.
pub const MAX_CHARS: usize = 512;

/// Highest version number of the banner accepted.
const MAX_VERSION: i64 = 1_000_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageViewBody {
    path: String,
    #[serde(default)]
    referrer: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsentBody {
    measure: bool,
    version: i64,
}

/// The path of a page of the site: starts with one `/`, without its query
/// and fragment, no control characters, at most [`MAX_CHARS`].
pub fn page_path(raw: &str) -> Result<String, &'static str> {
    let path = raw.trim();
    let path = path.split(['?', '#']).next().unwrap_or_default();
    if !path.starts_with('/') || path.starts_with("//") || path.contains('\\') {
        return Err("invalid");
    }
    if path.chars().any(char::is_control) {
        return Err("invalid");
    }
    if path.chars().count() > MAX_CHARS {
        return Err("too_long");
    }
    Ok(path.to_owned())
}

/// The origin (`scheme://host[:port]`) of an external referrer; none for
/// the site itself (and its subdomains), or for anything that is not an
/// http(s) address.
pub fn referrer_origin(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let (scheme, rest) = raw.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Never the credentials of `user:password@host`.
    let host_port = authority
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if host_port.is_empty()
        || host_port.len() > 255
        || !host_port
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
    {
        return None;
    }
    let host = if host_port.starts_with('[') {
        host_port.split(']').next().map(|h| format!("{h}]"))?
    } else {
        host_port.split(':').next()?.to_owned()
    };
    let site = SITE_URL.split("://").nth(1).unwrap_or_default();
    if host == site || host.ends_with(&format!(".{site}")) {
        return None;
    }
    Some(format!("{scheme}://{host_port}"))
}

/// `POST /v1/telemetry/page-view`: `{"path", "referrer"}`.
pub async fn page_view(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Json(body): Json<PageViewBody>,
) -> Result<Response, ApiError> {
    let path = page_path(&body.path).map_err(|code| {
        ApiError::InvalidFields(vec![ParameterError::field(
            "path",
            code,
            format!(
                "must be a path of the site, starting with `/`, at most {MAX_CHARS} characters"
            ),
        )])
    })?;
    if site::is_robot(caller.user_agent.as_deref()) {
        return Ok(empty());
    }
    let referrer = body.referrer.as_deref().and_then(referrer_origin);
    journal::page_view(
        &state.db,
        member.as_ref().map(|m| m.id()),
        caller.visitor,
        &path,
        referrer.as_deref(),
    )
    .await?;
    Ok(empty())
}

/// `POST /v1/consent`: `{"measure", "version"}`, the banner's answer,
/// kept as a proof with the member and/or the visitor id and the visitor
/// fingerprint (never the address).
pub async fn consent(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Json(body): Json<ConsentBody>,
) -> Result<Response, ApiError> {
    if !(1..=MAX_VERSION).contains(&body.version) {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            "version",
            "invalid",
            format!("must be between 1 and {MAX_VERSION}"),
        )]));
    }
    if site::is_robot(caller.user_agent.as_deref()) {
        return Ok(empty());
    }
    let hash =
        site::visitor_hash(&state, caller.ip.as_deref(), caller.user_agent.as_deref()).await?;
    journal::consent(
        &state.db,
        member.as_ref().map(|m| m.id()),
        caller.visitor,
        body.measure,
        i32::try_from(body.version).unwrap_or(i32::MAX),
        Some(&hash),
    )
    .await?;
    Ok(empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_lose_their_query_and_fragment() {
        assert_eq!(page_path("/cours/x?y=1#z").as_deref(), Ok("/cours/x"));
        assert_eq!(page_path(" / ").as_deref(), Ok("/"));
        assert_eq!(page_path("/forum#top").as_deref(), Ok("/forum"));
        assert_eq!(page_path("cours"), Err("invalid"));
        assert_eq!(page_path("https://evil.example/"), Err("invalid"));
        assert_eq!(page_path("//evil.example/x"), Err("invalid"));
        assert_eq!(page_path("/a\nb"), Err("invalid"));
        assert_eq!(
            page_path(&format!("/{}", "a".repeat(511))).map(|p| p.len()),
            Ok(512)
        );
        assert_eq!(page_path(&format!("/{}", "a".repeat(512))), Err("too_long"));
        // The query does not count.
        assert!(page_path(&format!("/x?{}", "q".repeat(600))).is_ok());
    }

    #[test]
    fn referrers_keep_their_origin_only() {
        assert_eq!(
            referrer_origin("https://www.google.com/search?q=pulsars").as_deref(),
            Some("https://www.google.com")
        );
        assert_eq!(
            referrer_origin("HTTP://User:Secret@Example.org:8080/x").as_deref(),
            Some("http://example.org:8080")
        );
        assert_eq!(
            referrer_origin("https://[2001:db8::1]:8443/p").as_deref(),
            Some("https://[2001:db8::1]:8443")
        );
        // The site itself, and its subdomains: none.
        assert_eq!(referrer_origin("https://pulsars.science/cours"), None);
        assert_eq!(referrer_origin("https://api.pulsars.science/"), None);
        assert_eq!(referrer_origin("android-app://com.google"), None);
        assert_eq!(referrer_origin("not a url"), None);
        assert_eq!(referrer_origin("https:///nohost"), None);
        assert_eq!(referrer_origin("https://ex ample.com/"), None);
    }
}
