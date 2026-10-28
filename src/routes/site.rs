//! What the member routes of the site share (`/v1/me/*` and the other
//! routes pulsars-web calls for its pages): `{"data": …}` bodies that are
//! never cached, the `lang` of translated text, Laravel's hourly cadences,
//! and the pseudonymous fingerprint of a visitor.

use axum::http::StatusCode;
use axum::response::Response;
use fred::prelude::{ClientLike as _, KeysInterface as _};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use tokio::sync::OnceCell;

use super::auth::{Data, no_store};
use super::params::{Lang, Params};
use crate::error::{ApiError, ParameterError};
use crate::state::AppState;

/// How long a Redis command of a cadence may take before it is skipped.
const REDIS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// `200 {"data": …}`, `Cache-Control: no-store`.
pub fn ok<T: Serialize>(data: T) -> Response {
    no_store(StatusCode::OK, &Data { data })
}

/// What an action earned, as Laravel's `xp` and `cerveaux` flashes:
/// `null` when nothing.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Rewards {
    pub xp: Option<i32>,
    pub brains: Option<i32>,
}

impl Rewards {
    pub fn new(xp: i32, brains: i32) -> Self {
        Self {
            xp: (xp > 0).then_some(xp),
            brains: (brains > 0).then_some(brains),
        }
    }
}

#[derive(Debug, Serialize)]
struct Rewarded<T> {
    data: T,
    rewards: Rewards,
}

/// `200 {"data": …, "rewards": {"xp", "brains"}}`, `no-store`.
pub fn rewarded<T: Serialize>(status: StatusCode, data: T, rewards: Rewards) -> Response {
    no_store(status, &Rewarded { data, rewards })
}

/// `201 {"data": …}`, `Cache-Control: no-store`.
pub fn created<T: Serialize>(data: T) -> Response {
    no_store(StatusCode::CREATED, &Data { data })
}

/// The public address of a file of the `public` disk (an avatar),
/// as `/v1/me` builds it.
pub fn avatar_url(state: &AppState, path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.is_empty()).map(|p| {
        format!(
            "{}/{}",
            state.config.public_storage_url,
            crate::links::encode(p.trim_start_matches('/'), true)
        )
    })
}

pub fn not_found(detail: impl Into<String>, path: &str) -> ApiError {
    ApiError::NotFound {
        detail: detail.into(),
        instance: Some(path.to_owned()),
    }
}

/// `?lang=fr|en|ru` and nothing else that is known.
pub fn lang(query: Option<&str>) -> Result<Lang, ApiError> {
    let mut params = Params::parse(query);
    let lang = params.lang();
    params.finish()?;
    Ok(lang)
}

/// One more of an hourly allowance, as Laravel's `verifierCadence`: a
/// counter kept an hour after its last increment, refused (422 on `field`,
/// code `too_many`) once it reached `max`. Counted only for a request that
/// is otherwise valid. Redis down: let through, like the rate limits.
pub async fn cadence(
    state: &AppState,
    action: &str,
    who: &str,
    max: i64,
    field: &str,
    detail: &str,
) -> Result<(), ApiError> {
    let redis = &state.redis;
    if !redis.is_connected() {
        return Ok(());
    }
    let key = format!("pulsars-api:cadence:{action}:{who}");
    let count = tokio::time::timeout(REDIS_TIMEOUT, redis.get::<Option<i64>, _>(&key)).await;
    let count = match count {
        Ok(Ok(count)) => count.unwrap_or(0),
        _ => return Ok(()),
    };
    if count >= max {
        return Err(ApiError::InvalidFields(vec![ParameterError::field(
            field,
            "too_many",
            detail.to_owned(),
        )]));
    }
    let pipeline = redis.pipeline();
    let queued: Result<(), fred::error::Error> = async {
        pipeline.incr::<(), _>(&key).await?;
        pipeline.expire::<(), _>(&key, 3600, None).await?;
        Ok(())
    }
    .await;
    if queued.is_ok() {
        let _ = tokio::time::timeout(REDIS_TIMEOUT, pipeline.all::<(i64, i64)>()).await;
    }
    Ok(())
}

static SALT: OnceCell<String> = OnceCell::const_new();

/// The salt of visitor fingerprints, drawn once by the migration
/// (`site.reglages`, `sel_visiteurs`).
async fn salt(state: &AppState) -> Result<&'static str, ApiError> {
    let salt = SALT
        .get_or_try_init(|| async {
            let salt: Option<String> = sqlx::query_scalar(
                "SELECT valeur::jsonb ->> 0 FROM site.reglages WHERE cle = 'sel_visiteurs'",
            )
            .fetch_optional(&state.db)
            .await?
            .flatten();
            salt.ok_or_else(|| ApiError::Internal("no `sel_visiteurs` in site.reglages".to_owned()))
        })
        .await?;
    Ok(salt.as_str())
}

/// A pseudonymous, stable fingerprint of a visitor, as Laravel's
/// `Visiteur::empreinte`: SHA-256 of address, browser and a salt only the
/// database knows. No address is ever stored in clear.
pub async fn visitor_hash(
    state: &AppState,
    ip: Option<&str>,
    user_agent: Option<&str>,
) -> Result<String, ApiError> {
    let salt = salt(state).await?;
    let digest = Sha256::digest(
        format!(
            "{}|{}|{salt}",
            ip.unwrap_or_default(),
            user_agent.unwrap_or_default()
        )
        .as_bytes(),
    );
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Clients that announce themselves as automated (Laravel's
/// `Visiteur::SIGNATURES`).
const ROBOTS: &[&str] = &[
    "bot",
    "crawler",
    "spider",
    "slurp",
    "archiver",
    "facebookexternalhit",
    "curl",
    "wget",
    "python",
    "scrapy",
    "java/",
    "go-http-client",
    "okhttp",
    "libwww",
    "httpclient",
    "axios",
    "node-fetch",
    "guzzle",
    "postman",
    "headless",
    "phantomjs",
    "selenium",
    "puppeteer",
    "playwright",
    "lighthouse",
    "pagespeed",
    "gtmetrix",
    "uptime",
    "monitor",
    "pingdom",
    "semrush",
    "ahrefs",
    "mj12",
    "dotbot",
    "petalbot",
    "dataprovider",
    "zgrab",
    "masscan",
    "nuclei",
    "expanse",
    "preview",
    "whatsapp",
    "telegrambot",
    "discordbot",
    "slackbot",
    "embed",
];

/// Whether the browser is a robot, from its `User-Agent` (relayed by the
/// site): none, or a known signature. The old site also refused an `Accept`
/// without `text/html`; behind the site's relay that header is the site's,
/// not the browser's, so it is not looked at.
pub fn is_robot(user_agent: Option<&str>) -> bool {
    let agent = user_agent.unwrap_or_default().to_lowercase();
    agent.is_empty() || ROBOTS.iter().any(|s| agent.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_announce_themselves() {
        assert!(is_robot(None));
        assert!(is_robot(Some("")));
        assert!(is_robot(Some(
            "Googlebot/2.1 (+http://www.google.com/bot.html)"
        )));
        assert!(is_robot(Some("curl/8.4.0")));
        assert!(!is_robot(Some(
            "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/126"
        )));
    }
}
