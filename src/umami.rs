//! The Umami API, which measures the site's audience (Laravel's
//! `App\Services\Umami`): distinct visitors and page views, without cookies
//! and without keeping addresses. The admin dashboard asks it for its
//! numbers instead of counting them again.
//!
//! Every failure is absorbed: an Umami that is down or misconfigured gives
//! `None`, and the dashboard says so instead of failing for one statistic.
//! The API token is kept in Redis (twelve hours) and renewed once on a 401.

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use fred::prelude::{ClientLike as _, Expiration, KeysInterface as _};
use serde_json::Value;

use crate::config::UmamiConfig;

const TOKEN_KEY: &str = "umami:token";
const TOKEN_TTL_SECS: i64 = 12 * 3600;
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct Umami<'a> {
    config: &'a UmamiConfig,
    redis: &'a fred::prelude::Client,
    http: reqwest::Client,
}

/// Visitors and page views over a period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub visitors: i64,
    pub page_views: i64,
}

impl<'a> Umami<'a> {
    pub fn new(config: &'a UmamiConfig, redis: &'a fred::prelude::Client) -> Option<Self> {
        let http = crate::http_client::http_client(false, TIMEOUT, TIMEOUT)
            .map_err(|error| tracing::warn!(%error, "umami: no HTTP client"))
            .ok()?;
        Some(Self {
            config,
            redis,
            http,
        })
    }

    /// When the website was created in Umami: the start of the measures.
    pub async fn measured_since(&self) -> Option<DateTime<Utc>> {
        let site = self.get("", &[]).await?;
        site.get("createdAt")?
            .as_str()?
            .parse::<DateTime<Utc>>()
            .ok()
    }

    pub async fn stats(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Option<Stats> {
        let data = self.get("stats", &period(from, to)).await?;
        Some(Stats {
            visitors: count(data.get("visitors")),
            page_views: count(data.get("pageviews")),
        })
    }

    /// Distinct visitors day by day.
    pub async fn visitors_by_day(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<(NaiveDate, i64)>> {
        let mut query = period(from, to);
        query.push(("unit", "day".to_owned()));
        query.push(("timezone", "UTC".to_owned()));
        let data = self.get("pageviews", &query).await?;
        // `2026-10-13 00:00:00`: the day is enough.
        Some(
            data.get("sessions")?
                .as_array()?
                .iter()
                .filter_map(|point| {
                    let day = point.get("x")?.as_str()?.get(..10)?;
                    Some((
                        NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?,
                        count(point.get("y")),
                    ))
                })
                .collect(),
        )
    }

    /// The most viewed paths, with their views.
    pub async fn top_pages(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: u32,
    ) -> Option<Vec<(String, i64)>> {
        let mut query = period(from, to);
        query.push(("type", "path".to_owned()));
        query.push(("limit", limit.to_string()));
        let data = self.get("metrics", &query).await?;
        Some(
            data.as_array()?
                .iter()
                .filter_map(|row| Some((row.get("x")?.as_str()?.to_owned(), count(row.get("y")))))
                .collect(),
        )
    }

    /// `GET /api/websites/{id}/{path}`, with the token; a 401 renews it once.
    async fn get(&self, path: &str, query: &[(&str, String)]) -> Option<Value> {
        let url = format!(
            "{}/api/websites/{}/{path}",
            self.config.url,
            crate::links::encode(&self.config.website_id, false)
        );
        let url = url.trim_end_matches('/').to_owned();
        let token = self.token(false).await?;
        let mut response = self.send(&url, query, &token).await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            let token = self.token(true).await?;
            response = self.send(&url, query, &token).await?;
        }
        read(response, &url).await
    }

    async fn send(
        &self,
        url: &str,
        query: &[(&str, String)],
        token: &str,
    ) -> Option<reqwest::Response> {
        let query = {
            let mut serializer = form_urlencoded::Serializer::new(String::new());
            for (name, value) in query {
                serializer.append_pair(name, value);
            }
            serializer.finish()
        };
        let url = if query.is_empty() {
            url.to_owned()
        } else {
            format!("{url}?{query}")
        };
        self.http
            .get(url)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| tracing::warn!(%error, "umami unreachable"))
            .ok()
    }

    /// The API token, from Redis unless `renew`; else from `/api/auth/login`.
    async fn token(&self, renew: bool) -> Option<String> {
        let cached = self.redis.is_connected() && !renew;
        if cached && let Ok(Some(token)) = self.redis.get::<Option<String>, _>(TOKEN_KEY).await {
            return Some(token);
        }
        let url = format!("{}/api/auth/login", self.config.url);
        let body = serde_json::json!({
            "username": self.config.username,
            "password": self.config.password.expose(),
        });
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|error| tracing::warn!(%error, "umami unreachable"))
            .ok()?;
        let token = read(response, &url)
            .await?
            .get("token")?
            .as_str()?
            .to_owned();
        if self.redis.is_connected() {
            let _: Result<(), _> = self
                .redis
                .set(
                    TOKEN_KEY,
                    token.as_str(),
                    Some(Expiration::EX(TOKEN_TTL_SECS)),
                    None,
                    false,
                )
                .await;
        }
        Some(token)
    }
}

fn period(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<(&'static str, String)> {
    vec![
        ("startAt", from.timestamp_millis().to_string()),
        ("endAt", to.timestamp_millis().to_string()),
    ]
}

/// A count, given as a number or (older Umami) as `{"value": n}`.
fn count(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().unwrap_or_default(),
        Some(Value::Object(o)) => o.get("value").and_then(Value::as_i64).unwrap_or_default(),
        Some(Value::String(s)) => s.parse().unwrap_or_default(),
        _ => 0,
    }
}

async fn read(response: reqwest::Response, url: &str) -> Option<Value> {
    if !response.status().is_success() {
        tracing::warn!(
            url,
            status = response.status().as_u16(),
            "umami refused the request"
        );
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_come_as_numbers_or_values() {
        assert_eq!(count(Some(&serde_json::json!(12))), 12);
        assert_eq!(count(Some(&serde_json::json!({"value": 7, "prev": 3}))), 7);
        assert_eq!(count(None), 0);
    }
}
