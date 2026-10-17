//! Delivery of the webhook outbox to Laravel (`docs/contract/webhooks.md`).
//!
//! - Rows are written by the match end, in its transaction
//!   ([`super::store::finish`]); this worker sends the pending ones.
//! - One in-flight attempt per event: the row is locked with
//!   `FOR UPDATE SKIP LOCKED` for the duration of the attempt.
//! - Each attempt signs `timestamp + "." + body` with HMAC-SHA256 (§2),
//!   with a fresh timestamp.
//! - `200` → `sent`; `400`, `413`, `422` → `dead` at once; anything else
//!   (`401`, `429`, `5xx`, timeouts) → retried with exponential backoff and
//!   full jitter (base 10 s, factor 3, ceiling 1 h), `429` after its
//!   `Retry-After`; after 20 attempts or 24 hours the row is `dead`.
//!
//! Without `LARAVEL_WEBHOOK_URL` and `LARAVEL_WEBHOOK_SECRET` the worker
//! does not run and rows stay `pending`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::Sha256;
use sqlx::PgPool;
use sqlx::types::Json;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::config::Secret;
use crate::telemetry::{TraceContext, names};

/// Settings of the delivery worker.
#[derive(Debug, Clone)]
pub struct WebhookSettings {
    /// `LARAVEL_WEBHOOK_URL`.
    pub url: String,
    /// `LARAVEL_WEBHOOK_SECRET`.
    pub secret: Secret,
    /// `Host` header to send, when the URL points at an address
    /// (`LARAVEL_WEBHOOK_HOST`).
    pub host: Option<String>,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub max_attempts: i32,
    pub max_age: Duration,
    /// How often the worker looks for due rows when idle.
    pub poll_interval: Duration,
}

impl WebhookSettings {
    pub fn new(url: String, secret: Secret, host: Option<String>) -> Self {
        Self {
            url,
            secret,
            host,
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(3),
            max_attempts: 20,
            max_age: Duration::from_secs(24 * 60 * 60),
            poll_interval: Duration::from_secs(2),
        }
    }
}

/// `v1=` + hex(HMAC-SHA256(secret, timestamp + "." + body)).
pub fn signature(secret: &[u8], timestamp: i64, body: &[u8]) -> String {
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret) {
        Ok(mac) => mac,
        // HMAC accepts keys of any length.
        Err(_) => return "v1=".to_owned(),
    };
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut out = String::with_capacity(3 + 64);
    out.push_str("v1=");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Delay before the next attempt after `attempts` failed ones: uniform in
/// `[0, min(1 h, 10 s × 3^(attempts − 1))]`. `jitter` is in `[0, 1]`.
pub fn backoff(attempts: i32, jitter: f64) -> Duration {
    let exponent = attempts.saturating_sub(1).clamp(0, 20);
    let cap = (10.0 * 3f64.powi(exponent)).min(3600.0);
    Duration::from_secs_f64(cap * jitter.clamp(0.0, 1.0))
}

/// What to do with a row after an attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Sent,
    /// Never retried: dead letter.
    Dead(String),
    /// Retried after the delay (`None`: the backoff).
    Retry {
        error: String,
        after: Option<Duration>,
    },
}

/// The verdict for an HTTP answer (`webhooks.md` §6).
pub fn verdict(status: u16, retry_after: Option<&str>) -> Verdict {
    match status {
        200..=299 => Verdict::Sent,
        400 | 413 | 422 => Verdict::Dead(format!("HTTP {status}")),
        429 => Verdict::Retry {
            error: "HTTP 429".to_owned(),
            after: retry_after
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|secs| Duration::from_secs(secs.min(3600))),
        },
        _ => Verdict::Retry {
            error: format!("HTTP {status}"),
            after: None,
        },
    }
}

/// A due row, locked for one attempt.
#[derive(sqlx::FromRow)]
struct Row {
    event_id: Uuid,
    body: Json<serde_json::Value>,
    attempts: i32,
    created_at: DateTime<Utc>,
}

/// The outcome of one delivery, for tests and logs.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub event_id: Uuid,
    pub verdict: Verdict,
}

/// Sends pending rows forever.
pub fn spawn_worker(pool: PgPool, settings: WebhookSettings) -> Result<JoinHandle<()>, String> {
    let client = client(&settings)?;
    let settings = Arc::new(settings);
    Ok(tokio::spawn(async move {
        tracing::info!(url = %settings.url, "webhook delivery started");
        let mut failures: u32 = 0;
        loop {
            match deliver_next(&pool, &client, &settings, None).await {
                Ok(Some(_)) => {
                    failures = 0;
                    continue;
                }
                Ok(None) => failures = 0,
                Err(error) => {
                    // The database is down: warn, then wait longer each time
                    // (up to a minute) instead of logging every poll.
                    failures = failures.saturating_add(1);
                    tracing::warn!(error = %error, failures, "webhook outbox unavailable");
                    let wait = settings.poll_interval * 2u32.saturating_pow(failures.min(5));
                    tokio::time::sleep(wait.min(Duration::from_secs(60))).await;
                    continue;
                }
            }
            if let Ok(pending) = sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM game.webhook_outbox WHERE status = 'pending'",
            )
            .fetch_one(&pool)
            .await
            {
                metrics::gauge!(names::WEBHOOK_PENDING).set(pending as f64);
            }
            tokio::time::sleep(settings.poll_interval).await;
        }
    }))
}

/// The HTTP client of the worker: no redirects, attempt timeouts.
pub fn client(settings: &WebhookSettings) -> Result<reqwest::Client, String> {
    crate::auth::jwks::http_client(false, settings.timeout, settings.connect_timeout)
}

/// Sends the oldest due row (or `only` that event), if any.
pub async fn deliver_next(
    pool: &PgPool,
    client: &reqwest::Client,
    settings: &WebhookSettings,
    only: Option<Uuid>,
) -> Result<Option<Delivery>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let row: Option<Row> = sqlx::query_as(
        "SELECT event_id, body, attempts, created_at
           FROM game.webhook_outbox
          WHERE status = 'pending' AND next_attempt_at <= now()
            AND ($1::uuid IS NULL OR event_id = $1)
          ORDER BY next_attempt_at
          LIMIT 1
          FOR UPDATE SKIP LOCKED",
    )
    .bind(only)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let started = Instant::now();
    let verdict = attempt(client, settings, row.event_id, &row.body.0).await;
    metrics::histogram!(names::WEBHOOK_LATENCY_SECONDS).record(started.elapsed().as_secs_f64());
    let attempts = row.attempts + 1;
    let status_label = match &verdict {
        Verdict::Sent => "sent",
        Verdict::Dead(_) => "rejected",
        Verdict::Retry { .. } => "failed",
    };
    metrics::counter!(names::WEBHOOK_ATTEMPTS_TOTAL, "status" => status_label).increment(1);

    let too_old = Utc::now() - row.created_at
        > chrono::Duration::from_std(settings.max_age).unwrap_or(chrono::Duration::days(1));
    let verdict = match verdict {
        Verdict::Retry { error, .. } if attempts >= settings.max_attempts || too_old => {
            Verdict::Dead(format!("{error} (gave up after {attempts} attempts)"))
        }
        other => other,
    };
    match &verdict {
        Verdict::Sent => {
            sqlx::query(
                "UPDATE game.webhook_outbox
                    SET status = 'sent', attempts = $2, sent_at = now(), last_error = NULL
                  WHERE event_id = $1",
            )
            .bind(row.event_id)
            .bind(attempts)
            .execute(&mut *tx)
            .await?;
            tracing::info!(event_id = %row.event_id, attempts, "webhook delivered");
        }
        Verdict::Dead(error) => {
            sqlx::query(
                "UPDATE game.webhook_outbox
                    SET status = 'dead', attempts = $2, last_error = $3
                  WHERE event_id = $1",
            )
            .bind(row.event_id)
            .bind(attempts)
            .bind(error)
            .execute(&mut *tx)
            .await?;
            metrics::counter!(names::WEBHOOK_DEAD_TOTAL).increment(1);
            tracing::error!(event_id = %row.event_id, attempts, error = %error,
                "webhook dead-lettered: replay with `pulsars-api webhooks replay <event_id>`");
        }
        Verdict::Retry { error, after } => {
            let jitter = super::random_below(1_000_001) as f64 / 1_000_000.0;
            let delay = after.unwrap_or_else(|| backoff(attempts, jitter));
            let delay = chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::hours(1));
            sqlx::query(
                "UPDATE game.webhook_outbox
                    SET attempts = $2, last_error = $3, next_attempt_at = now() + $4
                  WHERE event_id = $1",
            )
            .bind(row.event_id)
            .bind(attempts)
            .bind(error)
            .bind(delay)
            .execute(&mut *tx)
            .await?;
            if error == "HTTP 401" {
                // A secret rotation in progress or clock drift: someone must
                // look at it now (`webhooks.md` §6).
                tracing::error!(event_id = %row.event_id, attempts, "webhook refused: bad signature or clock");
            } else {
                tracing::warn!(event_id = %row.event_id, attempts, error = %error, "webhook failed, will retry");
            }
        }
    }
    tx.commit().await?;
    Ok(Some(Delivery {
        event_id: row.event_id,
        verdict,
    }))
}

async fn attempt(
    client: &reqwest::Client,
    settings: &WebhookSettings,
    event_id: Uuid,
    body: &serde_json::Value,
) -> Verdict {
    let bytes = match serde_json::to_vec(body) {
        Ok(bytes) => bytes,
        Err(error) => return Verdict::Dead(format!("cannot serialize: {error}")),
    };
    let timestamp = Utc::now().timestamp();
    let signature = signature(settings.secret.expose().as_bytes(), timestamp, &bytes);
    let trace = TraceContext::from_header(None);
    let mut request = client
        .post(&settings.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .header("X-Pulsars-Event-Id", event_id.to_string())
        .header("X-Pulsars-Timestamp", timestamp.to_string())
        .header("X-Pulsars-Signature", signature)
        .header("traceparent", trace.to_traceparent());
    if let Some(host) = &settings.host {
        request = request.header(reqwest::header::HOST, host);
    }
    match request.body(bytes).send().await {
        Ok(response) => {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            verdict(response.status().as_u16(), retry_after.as_deref())
        }
        Err(error) if error.is_timeout() => Verdict::Retry {
            error: "timeout".to_owned(),
            after: None,
        },
        Err(error) if error.is_connect() => Verdict::Retry {
            error: "connection failed".to_owned(),
            after: None,
        },
        Err(_) => Verdict::Retry {
            error: "request failed".to_owned(),
            after: None,
        },
    }
}

/// `pulsars-api webhooks replay <event_id>`: puts a row back in the queue
/// for an immediate attempt. Returns whether the row exists.
pub async fn replay(pool: &PgPool, event_id: Uuid) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE game.webhook_outbox
            SET status = 'pending', attempts = 0, next_attempt_at = now(), last_error = NULL,
                created_at = now()
          WHERE event_id = $1",
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4231, test case 2.
    #[test]
    fn hmac_matches_rfc_4231() {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"Jefe").expect("any key length");
        mac.update(b"what do ya want for nothing?");
        let digest: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// The value Laravel's `VerifierSignatureJeu` computes:
    /// `hash_hmac('sha256', '1792059631.{"a":1}', 'secret')`.
    #[test]
    fn signature_covers_timestamp_and_body() {
        let sig = signature(b"secret", 1_792_059_631, br#"{"a":1}"#);
        assert_eq!(
            sig,
            "v1=b305c34683cce3c165e1442e57477422c32a722df7b87cb345b66ce9193e61f7"
        );
        assert_ne!(sig, signature(b"secret", 1_792_059_632, br#"{"a":1}"#));
        assert_ne!(sig, signature(b"other", 1_792_059_631, br#"{"a":1}"#));
        assert_ne!(sig, signature(b"secret", 1_792_059_631, br#"{"a":2}"#));
    }

    #[test]
    fn backoff_grows_by_three_up_to_an_hour() {
        let full = |n| backoff(n, 1.0).as_secs();
        assert_eq!(
            (1..=8).map(full).collect::<Vec<_>>(),
            vec![10, 30, 90, 270, 810, 2430, 3600, 3600]
        );
        assert_eq!(backoff(3, 0.0), Duration::ZERO);
        assert_eq!(backoff(2, 0.5).as_secs(), 15);
        assert_eq!(full(100), 3600);
    }

    #[test]
    fn verdicts_follow_the_contract() {
        assert_eq!(verdict(200, None), Verdict::Sent);
        for dead in [400, 413, 422] {
            assert!(matches!(verdict(dead, None), Verdict::Dead(_)), "{dead}");
        }
        for retry in [401, 404, 500, 502, 503] {
            assert!(
                matches!(verdict(retry, None), Verdict::Retry { after: None, .. }),
                "{retry}"
            );
        }
        assert_eq!(
            verdict(429, Some("42")),
            Verdict::Retry {
                error: "HTTP 429".to_owned(),
                after: Some(Duration::from_secs(42))
            }
        );
    }
}
