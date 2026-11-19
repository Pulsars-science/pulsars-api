//! The HTTP traffic kept day by day (`site.http_daily`), for the admin
//! dashboard: the Prometheus metrics live in memory only and restart at
//! zero with the process.
//!
//! - [`HttpDaily::record`] adds a request to an in-process accumulator, by
//!   UTC day: a count, a 5xx count, and its latency in one of the buckets of
//!   [`BOUNDS_MS`] (finer than Prometheus', for a useful p95).
//! - [`HttpDaily::flush`] adds the accumulator to the table, every
//!   [`FLUSH_EVERY`] and once more at shutdown (`main.rs`), with sums: every
//!   instance and every restart adds up. A failed flush puts its counts
//!   back, to be added by the next one.
//! - [`p95`] reads a percentile back from summed buckets.
//!
//! WebSocket upgrades, `/metrics` and the health check are not counted
//! ([`counted`]): a match lasts minutes, and probes are not traffic.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::http::StatusCode;
use chrono::{NaiveDate, Utc};
use sqlx::PgPool;

/// Upper bounds of the latency buckets, in milliseconds (a request of
/// exactly a bound falls in its bucket). One more bucket holds everything
/// slower than the last. Rows of `site.http_daily` are read with these: keep
/// them fixed.
pub const BOUNDS_MS: [u32; 15] = [
    5, 10, 25, 50, 75, 100, 150, 200, 300, 500, 750, 1000, 2000, 5000, 10000,
];

/// Number of buckets: one per bound and the overflow.
pub const BUCKETS: usize = BOUNDS_MS.len() + 1;

/// How often the accumulator is added to the table.
pub const FLUSH_EVERY: Duration = Duration::from_secs(60);

/// Routes that are not traffic: the match WebSocket (long-lived), the
/// health check and the metrics.
const NOT_COUNTED: [&str; 3] = ["/v1/game/ws", "/healthz", "/metrics"];

/// What a day adds up to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Day {
    pub requests: i64,
    pub errors_5xx: i64,
    pub buckets: [i64; BUCKETS],
}

impl Day {
    fn add(&mut self, other: &Day) {
        self.requests += other.requests;
        self.errors_5xx += other.errors_5xx;
        for (mine, theirs) in self.buckets.iter_mut().zip(other.buckets) {
            *mine += theirs;
        }
    }
}

/// The accumulator, shared by every request of the process.
#[derive(Debug, Clone, Default)]
pub struct HttpDaily(Arc<Mutex<HashMap<NaiveDate, Day>>>);

/// Whether a request to `route` answered with `status` counts.
pub fn counted(route: &str, status: StatusCode) -> bool {
    status != StatusCode::SWITCHING_PROTOCOLS && !NOT_COUNTED.contains(&route)
}

/// The bucket of a latency.
pub fn bucket(latency: Duration) -> usize {
    let ms = latency.as_secs_f64() * 1000.0;
    BOUNDS_MS
        .iter()
        .position(|&bound| ms <= f64::from(bound))
        .unwrap_or(BOUNDS_MS.len())
}

impl HttpDaily {
    fn days(&self) -> MutexGuard<'_, HashMap<NaiveDate, Day>> {
        // A panic while holding the lock leaves counts, not a broken state.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts a request answered today (UTC).
    pub fn record(&self, status: StatusCode, latency: Duration) {
        self.record_on(Utc::now().date_naive(), status, latency);
    }

    pub fn record_on(&self, day: NaiveDate, status: StatusCode, latency: Duration) {
        let mut days = self.days();
        let entry = days.entry(day).or_default();
        entry.requests += 1;
        if status.is_server_error() {
            entry.errors_5xx += 1;
        }
        entry.buckets[bucket(latency)] += 1;
    }

    /// What is waiting to be flushed.
    pub fn pending(&self) -> HashMap<NaiveDate, Day> {
        self.days().clone()
    }

    /// Adds what was counted since the last flush to `site.http_daily`, in
    /// one transaction. On failure the counts go back into the accumulator
    /// (with what was counted meanwhile), for the next flush.
    pub async fn flush(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let taken = std::mem::take(&mut *self.days());
        if taken.is_empty() {
            return Ok(());
        }
        let written = write(pool, &taken).await;
        if written.is_err() {
            let mut days = self.days();
            for (day, counts) in &taken {
                days.entry(*day).or_default().add(counts);
            }
        }
        written
    }

    /// Flushes every [`FLUSH_EVERY`], forever; a failure is logged and
    /// retried at the next tick.
    pub async fn run_flusher(self, pool: PgPool) {
        let mut tick = tokio::time::interval(FLUSH_EVERY);
        // The first tick is immediate: nothing to flush yet.
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(error) = self.flush(&pool).await {
                tracing::warn!(error = %error, "cannot write the daily HTTP counts, kept for the next try");
            }
        }
    }
}

async fn write(pool: &PgPool, days: &HashMap<NaiveDate, Day>) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (day, counts) in days {
        sqlx::query(
            "INSERT INTO site.http_daily AS h (day, requests, errors_5xx, buckets)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (day) DO UPDATE
                SET requests = h.requests + EXCLUDED.requests,
                    errors_5xx = h.errors_5xx + EXCLUDED.errors_5xx,
                    buckets = ARRAY(
                        SELECT coalesce(a, 0) + coalesce(b, 0)
                          FROM unnest(h.buckets, EXCLUDED.buckets) WITH ORDINALITY AS t(a, b, n)
                         ORDER BY n)",
        )
        .bind(day)
        .bind(counts.requests)
        .bind(counts.errors_5xx)
        .bind(counts.buckets.as_slice())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// The 95th percentile of the latencies counted in `buckets`, in whole
/// milliseconds: linear inside the bucket that crosses 95 %, the overflow
/// bucket read as its lower bound. `None` without any request. Buckets past
/// [`BUCKETS`] are read as overflow too.
pub fn p95(buckets: &[i64]) -> Option<i64> {
    let total: i64 = buckets.iter().map(|&n| n.max(0)).sum();
    if total == 0 {
        return None;
    }
    let target = total as f64 * 0.95;
    let mut below = 0i64;
    for (i, &n) in buckets.iter().enumerate() {
        let n = n.max(0);
        if n > 0 && (below + n) as f64 >= target {
            let lower = if i == 0 {
                0.0
            } else {
                f64::from(BOUNDS_MS[(i - 1).min(BOUNDS_MS.len() - 1)])
            };
            let Some(&upper) = BOUNDS_MS.get(i) else {
                return Some(lower as i64);
            };
            let share = (target - below as f64) / n as f64;
            return Some((lower + share * (f64::from(upper) - lower)).round() as i64);
        }
        below += n;
    }
    // Unreachable with `total > 0`; the last bound as a safe answer.
    Some(i64::from(BOUNDS_MS[BOUNDS_MS.len() - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &[(usize, i64)]) -> [i64; BUCKETS] {
        let mut buckets = [0; BUCKETS];
        for &(i, n) in pairs {
            buckets[i] = n;
        }
        buckets
    }

    #[test]
    fn latencies_fall_in_their_bucket() {
        assert_eq!(bucket(Duration::from_micros(0)), 0);
        assert_eq!(bucket(Duration::from_millis(5)), 0);
        assert_eq!(bucket(Duration::from_micros(5_001)), 1);
        assert_eq!(bucket(Duration::from_millis(120)), 6);
        assert_eq!(bucket(Duration::from_millis(10_000)), 14);
        assert_eq!(bucket(Duration::from_secs(60)), 15);
    }

    #[test]
    fn the_p95_is_interpolated_in_its_bucket() {
        assert_eq!(p95(&[0; BUCKETS]), None);
        assert_eq!(p95(&[]), None);
        // 100 requests between 0 and 5 ms: the 95th at 4.75 ms.
        assert_eq!(p95(&with(&[(0, 100)])), Some(5));
        // 90 fast ones, 10 between 100 and 150 ms: 95 is halfway in.
        assert_eq!(p95(&with(&[(0, 90), (6, 10)])), Some(125));
        // 95 exactly reached at the end of a bucket: its upper bound.
        assert_eq!(p95(&with(&[(1, 95), (9, 5)])), Some(10));
        // Empty buckets in between are skipped.
        assert_eq!(p95(&with(&[(0, 50), (12, 50)])), Some(1900));
    }

    #[test]
    fn the_overflow_bucket_is_its_lower_bound() {
        assert_eq!(p95(&with(&[(15, 1)])), Some(10_000));
        assert_eq!(p95(&with(&[(0, 10), (15, 90)])), Some(10_000));
        // A longer array (more buckets than known) is overflow too.
        let mut longer = vec![0; BUCKETS + 2];
        longer[BUCKETS + 1] = 3;
        assert_eq!(p95(&longer), Some(10_000));
    }

    #[test]
    fn upgrades_probes_and_metrics_are_not_counted() {
        assert!(counted("/v1/courses", StatusCode::OK));
        assert!(counted("unmatched", StatusCode::NOT_FOUND));
        assert!(!counted("/v1/game/ws", StatusCode::BAD_REQUEST));
        assert!(!counted("/v1/courses", StatusCode::SWITCHING_PROTOCOLS));
        assert!(!counted("/healthz", StatusCode::OK));
        assert!(!counted("/metrics", StatusCode::OK));
    }

    #[test]
    fn requests_add_up_by_day() {
        let daily = HttpDaily::default();
        let day = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        daily.record_on(day, StatusCode::OK, Duration::from_millis(3));
        daily.record_on(day, StatusCode::BAD_GATEWAY, Duration::from_millis(400));
        let pending = daily.pending();
        assert_eq!(pending[&day].requests, 2);
        assert_eq!(pending[&day].errors_5xx, 1);
        assert_eq!(pending[&day].buckets, with(&[(0, 1), (9, 1)]));
    }
}
