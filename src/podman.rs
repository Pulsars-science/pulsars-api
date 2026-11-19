//! The containers' health for the admin dashboard, read from the snapshot
//! that `pulsars-podman-releve` writes every minute (`deploy/podman/`):
//! `{"taken_at", "ps", "inspect", "stats"}`, the raw JSON of `podman ps`,
//! `podman inspect` and `podman stats`. The API never talks to Podman: its
//! socket is root-equivalent.
//!
//! Podman's JSON changed between versions, so everything is read from
//! [`Value`] and both shapes are tolerated:
//!
//! - `ps`: `Names` an array (4.x, 5.x) or a string; `State` a string
//!   (`running`) or, in old versions, a number next to a `Status` such as
//!   `Up 2 hours (healthy)` or `Exited (0) 3 minutes ago`; `StartedAt` in
//!   Unix seconds; `Restarts`.
//! - `inspect`: `State.Status`, `State.StartedAt` (RFC 3339, nanoseconds,
//!   any offset; year 1 is "never"), the health in `State.Health.Status`
//!   or, before podman 4.3, `State.Healthcheck.Status` (`""` without a
//!   check), `RestartCount`.
//! - `stats`: snake case strings (`cpu_percent: "1.23%"`, `mem_usage:
//!   "45.06MB / 2.047GB"`) or the Go structure (`CPU`/`CPUPerc` a number
//!   or a string, `MemUsage` and `MemLimit` in bytes).
//!
//! A container whose entries cannot be read loses those fields, not the
//! response.

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncReadExt as _;

/// The largest snapshot read: a few dozen containers weigh far less.
pub const MAX_SNAPSHOT_BYTES: u64 = 2 * 1024 * 1024;

/// A snapshot older than this is stale: the timer stopped, or the host.
pub const STALE_AFTER_MINUTES: i64 = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Containers {
    pub available: bool,
    /// `not_configured`, `missing` or `unreadable` when not available.
    pub reason: Option<&'static str>,
    pub taken_at: Option<String>,
    pub stale: bool,
    pub containers: Vec<Container>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Container {
    pub name: String,
    pub image: String,
    /// Podman's state, lowercased: `running`, `exited`, `created`…
    pub state: String,
    /// `healthy`, `unhealthy` or `starting`; null without a health check.
    pub health: Option<&'static str>,
    pub started_at: Option<String>,
    pub restarts: i64,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<i64>,
    pub memory_limit_bytes: Option<i64>,
}

impl Containers {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            available: false,
            reason: Some(reason),
            taken_at: None,
            stale: false,
            containers: Vec::new(),
        }
    }
}

/// Reads the snapshot at `path` (`None`: not configured).
pub async fn read(path: Option<&Path>, now: DateTime<Utc>) -> Containers {
    let Some(path) = path else {
        return Containers::unavailable("not_configured");
    };
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Containers::unavailable("missing");
        }
        Err(error) => {
            tracing::warn!(error = %error, path = %path.display(), "cannot open the podman snapshot");
            return Containers::unavailable("unreadable");
        }
    };
    let mut bytes = Vec::new();
    if let Err(error) = file
        .take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
    {
        tracing::warn!(error = %error, path = %path.display(), "cannot read the podman snapshot");
        return Containers::unavailable("unreadable");
    }
    if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
        tracing::warn!(path = %path.display(), "the podman snapshot is too large");
        return Containers::unavailable("unreadable");
    }
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(snapshot) => summarize(&snapshot, now),
        Err(error) => {
            tracing::warn!(error = %error, path = %path.display(), "the podman snapshot is not JSON");
            Containers::unavailable("unreadable")
        }
    }
}

/// The containers of a snapshot, problems first (not running, or
/// unhealthy), then by name. Unreadable without a valid `taken_at` or a
/// `ps` list (podman failed).
pub fn summarize(snapshot: &Value, now: DateTime<Utc>) -> Containers {
    let Some(taken_at) = snapshot
        .get("taken_at")
        .and_then(Value::as_str)
        .and_then(rfc3339)
    else {
        return Containers::unavailable("unreadable");
    };
    let stale = now - taken_at > Duration::minutes(STALE_AFTER_MINUTES);
    let Some(ps) = snapshot.get("ps").and_then(Value::as_array) else {
        let mut unreadable = Containers::unavailable("unreadable");
        unreadable.taken_at = Some(crate::routes::courses::timestamp(taken_at));
        unreadable.stale = stale;
        return unreadable;
    };
    let inspect = list(snapshot.get("inspect"));
    let stats = list(snapshot.get("stats"));

    let mut containers: Vec<Container> = ps
        .iter()
        .filter_map(|entry| container(entry, &inspect, &stats))
        .collect();
    containers.sort_by(|a, b| {
        a.is_fine()
            .cmp(&b.is_fine())
            .then_with(|| a.name.cmp(&b.name))
    });
    Containers {
        available: true,
        reason: None,
        taken_at: Some(crate::routes::courses::timestamp(taken_at)),
        stale,
        containers,
    }
}

impl Container {
    fn is_fine(&self) -> bool {
        self.state == "running" && self.health != Some("unhealthy")
    }
}

fn list(value: Option<&Value>) -> Vec<&Value> {
    value
        .and_then(Value::as_array)
        .map(|entries| entries.iter().filter(|e| e.is_object()).collect())
        .unwrap_or_default()
}

/// The first string of the keys found.
fn text<'a>(entry: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| entry.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn integer(entry: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        let value = entry.get(*key)?;
        value
            .as_i64()
            .or_else(|| value.as_f64().map(|f| f as i64))
            .or_else(|| value.as_str()?.trim().parse().ok())
    })
}

fn rfc3339(text: &str) -> Option<DateTime<Utc>> {
    let at = DateTime::parse_from_rfc3339(text.trim())
        .ok()?
        .with_timezone(&Utc);
    // Go's zero time: never started.
    (at.timestamp() > 0).then_some(at)
}

/// `Names` as an array or a string; inspect's `Name` (Docker's has a `/`).
fn name(ps: &Value, inspect: Option<&Value>) -> Option<String> {
    let from_ps = match ps.get("Names") {
        Some(Value::Array(names)) => names.iter().find_map(Value::as_str),
        Some(Value::String(names)) => names.split(',').next(),
        _ => None,
    }
    .or_else(|| text(ps, &["Name"]));
    from_ps
        .or_else(|| inspect.and_then(|i| text(i, &["Name"])))
        .map(|n| n.trim().trim_start_matches('/').to_owned())
        .filter(|n| !n.is_empty())
}

fn id(entry: &Value) -> Option<&str> {
    text(entry, &["Id", "ID", "id", "ContainerID"])
}

/// Whether two ids name the same container: `stats` gives short ones.
fn same_id(a: &str, b: &str) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 12 && long.starts_with(short)
}

fn container(ps: &Value, inspect: &[&Value], stats: &[&Value]) -> Option<Container> {
    if !ps.is_object() {
        return None;
    }
    let ps_id = id(ps);
    let ps_name = name(ps, None);
    let matches = |entry: &Value, entry_name: Option<String>| match (ps_id, id(entry)) {
        (Some(a), Some(b)) => same_id(a, b),
        _ => ps_name.is_some() && entry_name == ps_name,
    };
    let details = inspect
        .iter()
        .copied()
        .find(|entry| matches(entry, name(entry, None)));
    let name = name(ps, details).or_else(|| ps_id.map(|id| id.chars().take(12).collect()))?;
    let usage = stats.iter().copied().find(|entry| {
        matches(entry, text(entry, &["name", "Name"]).map(str::to_owned))
            || text(entry, &["name", "Name"]) == Some(name.as_str())
    });

    let state_info = details.and_then(|d| d.get("State"));
    let status_line = text(ps, &["Status"]).unwrap_or_default();
    let state = state_info
        .and_then(|s| text(s, &["Status"]))
        .or_else(|| text(ps, &["State"]))
        .map(str::to_lowercase)
        .unwrap_or_else(|| state_from_status(status_line));

    let health = state_info
        .and_then(|s| {
            ["Health", "Healthcheck"]
                .iter()
                .find_map(|key| text(s.get(*key)?, &["Status"]))
        })
        .or_else(|| health_in_status(status_line))
        .and_then(health);

    let started_at = state_info
        .and_then(|s| text(s, &["StartedAt"]))
        .and_then(rfc3339)
        .or_else(|| {
            integer(ps, &["StartedAt"])
                .filter(|&t| t > 0)
                .and_then(|t| DateTime::from_timestamp(t, 0))
        })
        .filter(|_| state != "created");

    let restarts = details
        .and_then(|d| integer(d, &["RestartCount"]))
        .or_else(|| integer(ps, &["Restarts", "RestartCount"]))
        .unwrap_or(0)
        .max(0);

    // A stopped container's numbers are zeros or `--`: none.
    let usage = usage.filter(|_| state == "running");
    let cpu_percent = usage.and_then(cpu);
    let (memory_bytes, memory_limit_bytes) = usage.map(memory).unwrap_or_default();

    Some(Container {
        name,
        image: text(ps, &["Image"])
            .or_else(|| details.and_then(|d| text(d, &["ImageName", "Image"])))
            .unwrap_or_default()
            .to_owned(),
        state,
        health,
        started_at: started_at.map(crate::routes::courses::timestamp),
        restarts,
        cpu_percent,
        memory_bytes,
        memory_limit_bytes,
    })
}

fn health(status: &str) -> Option<&'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "healthy" => Some("healthy"),
        "unhealthy" => Some("unhealthy"),
        "starting" => Some("starting"),
        _ => None,
    }
}

/// `Up 2 hours (healthy)`: the health between brackets.
fn health_in_status(status: &str) -> Option<&str> {
    let start = status.rfind('(')?;
    let end = status[start..].find(')')? + start;
    Some(&status[start + 1..end])
}

/// The state of an old `ps`, whose `State` is a number: from `Status`.
fn state_from_status(status: &str) -> String {
    let first = status
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match first.as_str() {
        "up" => "running".to_owned(),
        "" => "unknown".to_owned(),
        other => other.to_owned(),
    }
}

fn cpu(stats: &Value) -> Option<f64> {
    ["cpu_percent", "CPUPerc", "CPU", "cpu"]
        .iter()
        .find_map(|key| {
            let value = stats.get(*key)?;
            value
                .as_f64()
                .or_else(|| {
                    value
                        .as_str()?
                        .trim()
                        .trim_end_matches('%')
                        .trim()
                        .parse()
                        .ok()
                })
                .filter(|p: &f64| p.is_finite() && *p >= 0.0)
                .map(|p| (p * 100.0).round() / 100.0)
        })
}

/// Usage and limit: numbers (`MemUsage`, `MemLimit`) or one string
/// `"45.06MB / 2.047GB"` (`mem_usage`, or `MemUsage` as a string).
fn memory(stats: &Value) -> (Option<i64>, Option<i64>) {
    let number = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| stats.get(*key)?.as_f64())
            .map(|n| n as i64)
    };
    let mut usage = number(&["MemUsage", "mem_usage_bytes"]);
    let mut limit = number(&["MemLimit", "mem_limit", "mem_limit_bytes"]);
    let line = ["mem_usage", "MemUsage", "MemUsageBytes"]
        .iter()
        .find_map(|key| stats.get(*key)?.as_str());
    if let Some(line) = line {
        let mut parts = line.split('/');
        if usage.is_none() {
            usage = parts.next().and_then(bytes);
        } else {
            parts.next();
        }
        if limit.is_none() {
            limit = parts.next().and_then(bytes);
        }
    }
    (
        usage.filter(|&n| n >= 0),
        // No limit is the host's memory, or 0: none either way when 0.
        limit.filter(|&n| n > 0),
    )
}

/// `12.3MB`, `1GB`, `512KiB`, `0B`, `2.047GB`: decimal or binary units,
/// as go-units writes them. `--`: none.
fn bytes(text: &str) -> Option<i64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.parse().ok()?;
    let factor: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1e3,
        "m" | "mb" => 1e6,
        "g" | "gb" => 1e9,
        "t" | "tb" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * factor).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    const PODMAN4: &str = include_str!("../tests/fixtures/podman/podman4.json");
    const PODMAN5: &str = include_str!("../tests/fixtures/podman/podman5.json");

    /// The sort key the response promises, for the tests.
    fn by_problem_then_name(containers: &[Container]) -> bool {
        let keys: Vec<(bool, &str)> = containers
            .iter()
            .map(|c| (c.is_fine(), c.name.as_str()))
            .collect();
        keys.windows(2).all(|w| w[0] <= w[1])
    }

    /// Containers by name, for lookups in tests and callers.
    fn by_name(containers: &[Container]) -> HashMap<&str, &Container> {
        containers.iter().map(|c| (c.name.as_str(), c)).collect()
    }

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    #[test]
    fn sizes_read_both_unit_systems() {
        assert_eq!(bytes("0B"), Some(0));
        assert_eq!(bytes("45.06MB"), Some(45_060_000));
        assert_eq!(bytes(" 2.047GB "), Some(2_047_000_000));
        assert_eq!(bytes("512KiB"), Some(524_288));
        assert_eq!(bytes("1.5GiB"), Some(1_610_612_736));
        assert_eq!(bytes("12.3kB"), Some(12_300));
        assert_eq!(bytes("--"), None);
        assert_eq!(bytes("lots"), None);
    }

    #[test]
    fn a_podman_4_snapshot() {
        let snapshot: Value = serde_json::from_str(PODMAN4).unwrap();
        let result = summarize(&snapshot, at("2024-05-01T12:01:00Z"));
        assert!(result.available);
        assert_eq!(result.reason, None);
        assert_eq!(result.taken_at.as_deref(), Some("2024-05-01T12:00:00Z"));
        assert!(!result.stale);
        assert!(by_problem_then_name(&result.containers));
        let names: Vec<&str> = result.containers.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["pulsars-redis", "pulsars-api", "pulsars-db", "umami"]
        );

        let all = by_name(&result.containers);
        let db = all["pulsars-db"];
        assert_eq!(db.image, "docker.io/library/postgres:17-alpine");
        assert_eq!(db.state, "running");
        // `State.Healthcheck` of podman before 4.3.
        assert_eq!(db.health, Some("healthy"));
        // Nanoseconds and an offset, brought back to UTC.
        assert_eq!(db.started_at.as_deref(), Some("2024-05-01T08:00:00Z"));
        assert_eq!(db.restarts, 0);
        assert_eq!(db.cpu_percent, Some(1.23));
        assert_eq!(db.memory_bytes, Some(45_060_000));
        assert_eq!(db.memory_limit_bytes, Some(2_047_000_000));

        let api = all["pulsars-api"];
        assert_eq!(api.health, Some("starting"));
        assert_eq!(api.restarts, 2);
        assert_eq!(api.memory_bytes, Some(13_002_342));

        // An empty health status: no check.
        assert_eq!(all["umami"].health, None);
        assert_eq!(all["umami"].cpu_percent, Some(0.0));

        let redis = all["pulsars-redis"];
        assert_eq!(redis.state, "exited");
        assert_eq!(redis.restarts, 5);
        assert_eq!(redis.cpu_percent, None);
        assert_eq!(redis.memory_bytes, None);
        assert_eq!(redis.started_at.as_deref(), Some("2024-05-01T11:50:00Z"));
    }

    #[test]
    fn a_podman_5_snapshot() {
        let snapshot: Value = serde_json::from_str(PODMAN5).unwrap();
        let result = summarize(&snapshot, at("2026-10-29T12:10:00Z"));
        assert!(result.available);
        // Taken ten minutes ago: the timer stopped.
        assert!(result.stale);
        let names: Vec<&str> = result.containers.iter().map(|c| c.name.as_str()).collect();
        // Unhealthy and created first, then the healthy ones by name.
        assert_eq!(
            names,
            [
                "pulsars-migrate",
                "pulsars-web",
                "pulsars-api",
                "pulsars-db",
                "umami"
            ]
        );
        let all = by_name(&result.containers);
        let web = all["pulsars-web"];
        assert_eq!(web.state, "running");
        assert_eq!(web.health, Some("unhealthy"));
        assert_eq!(web.restarts, 1);
        assert_eq!(web.cpu_percent, Some(12.5));
        assert_eq!(web.memory_bytes, Some(157_286_400));
        assert_eq!(web.memory_limit_bytes, Some(536_870_912));
        let api = all["pulsars-api"];
        assert_eq!(api.health, Some("healthy"));
        assert_eq!(api.cpu_percent, Some(0.37));
        assert_eq!(api.started_at.as_deref(), Some("2026-10-29T09:15:42Z"));
        // No `Health` at all: no check.
        assert_eq!(all["umami"].health, None);
        // A limit of 0 is no limit.
        assert_eq!(all["umami"].memory_limit_bytes, None);
        let migrate = all["pulsars-migrate"];
        assert_eq!(migrate.state, "created");
        assert_eq!(migrate.started_at, None);
        assert_eq!(migrate.cpu_percent, None);
    }

    #[test]
    fn old_and_broken_entries_do_not_fail_the_rest() {
        let snapshot = json!({
            "taken_at": "2026-10-29T12:00:00Z",
            "ps": [
                // `State` a number and `Names` a string, as podman 1 and 2.
                {"ID": "0123456789abcdef", "Names": "legacy", "Image": "x",
                 "State": 3, "Status": "Up 5 minutes (unhealthy)", "StartedAt": 1_761_739_200},
                {"Id": "fedcba9876543210", "Names": ["gone"], "Status": "Exited (1) 2 minutes ago"},
                "not an object",
                {"Image": "nameless, idless"},
                {"Id": "aaaaaaaaaaaaaaaa", "Names": [], "State": "running"}
            ],
            "inspect": null,
            "stats": "garbage",
        });
        let result = summarize(&snapshot, at("2026-10-29T12:00:30Z"));
        assert!(result.available);
        let all = by_name(&result.containers);
        assert_eq!(result.containers.len(), 3);
        assert_eq!(all["legacy"].state, "running");
        assert_eq!(all["legacy"].health, Some("unhealthy"));
        assert_eq!(
            all["legacy"].started_at.as_deref(),
            Some("2025-10-29T12:00:00Z")
        );
        assert_eq!(all["gone"].state, "exited");
        assert_eq!(all["gone"].image, "");
        // No name: its short id.
        assert_eq!(all["aaaaaaaaaaaa"].state, "running");
        assert!(by_problem_then_name(&result.containers));
    }

    #[test]
    fn a_snapshot_without_its_essentials_is_unreadable() {
        let now = at("2026-10-29T12:00:00Z");
        for snapshot in [
            json!([]),
            json!({"ps": []}),
            json!({"taken_at": "yesterday", "ps": []}),
        ] {
            let result = summarize(&snapshot, now);
            assert!(!result.available);
            assert_eq!(result.reason, Some("unreadable"));
            assert_eq!(result.taken_at, None);
        }
        // Podman failed: when is known, what is not.
        let result = summarize(
            &json!({"taken_at": "2026-10-29T11:59:00Z", "ps": null, "inspect": null, "stats": null}),
            now,
        );
        assert_eq!(result.reason, Some("unreadable"));
        assert_eq!(result.taken_at.as_deref(), Some("2026-10-29T11:59:00Z"));
        assert!(!result.stale);
        // No container at all is a valid answer.
        let result = summarize(
            &json!({"taken_at": "2026-10-29T11:59:00Z", "ps": [], "inspect": [], "stats": []}),
            now,
        );
        assert!(result.available);
        assert!(result.containers.is_empty());
    }
}
