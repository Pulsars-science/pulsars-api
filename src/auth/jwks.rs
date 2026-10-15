//! Laravel's public keys (JWKS), cached as `docs/contract/auth-jwt.md` §3
//! requires:
//!
//! - fetched at startup, then refreshed in the background every 10 minutes;
//! - a failed refresh keeps the previous set, for up to 24 hours after the
//!   last success; after that, tokens are refused (503) until a fetch works;
//! - an unknown `kid` triggers an immediate refetch, at most once every 60
//!   seconds for the whole process, so a flood of random `kid`s cannot turn
//!   the API into a JWKS amplifier;
//! - only `OKP` / `Ed25519` / `EdDSA` signing keys are kept.
//!
//! A JWKS that cannot be loaded never crashes the process: `/healthz`
//! answers 503 until the first key is loaded.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::VerifyingKey;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::config::{Config, Environment};
use crate::telemetry::names;

/// Timings of the cache. Production uses [`JwksSettings::default`]; tests
/// shorten them.
#[derive(Debug, Clone)]
pub struct JwksSettings {
    /// Background refresh period after a successful fetch.
    pub refresh_interval: Duration,
    /// First retry delay after a failed fetch, doubled up to
    /// [`Self::max_retry_interval`].
    pub min_retry_interval: Duration,
    pub max_retry_interval: Duration,
    /// Minimum time between two refetches triggered by unknown `kid`s.
    pub min_refetch_interval: Duration,
    /// How long a set stays usable after the last successful fetch.
    pub max_stale: Duration,
    /// Whole-request timeout of a fetch.
    pub fetch_timeout: Duration,
    /// Largest JWKS document accepted. A set holds one to three keys.
    pub max_body_bytes: usize,
}

impl Default for JwksSettings {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(10 * 60),
            min_retry_interval: Duration::from_secs(1),
            max_retry_interval: Duration::from_secs(60),
            min_refetch_interval: Duration::from_secs(60),
            max_stale: Duration::from_secs(24 * 60 * 60),
            fetch_timeout: Duration::from_secs(5),
            max_body_bytes: 64 * 1024,
        }
    }
}

/// Result of a key lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Found(VerifyingKey),
    /// The set is usable but has no key with this `kid`, even after a
    /// refetch: `401 unknown_kid`.
    UnknownKid,
    /// No usable set (never loaded, or stale for too long): `503`.
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("unexpected status {0}")]
    Status(u16),
    #[error("document larger than {0} bytes")]
    TooLarge(usize),
    #[error("not a JWK set: {0}")]
    Invalid(String),
    #[error("no usable Ed25519 key in the set")]
    NoUsableKey,
}

/// Shared handle on the cached key set. Cheap to clone.
#[derive(Clone)]
pub struct Jwks {
    inner: Arc<Inner>,
}

struct Inner {
    url: String,
    client: reqwest::Client,
    settings: JwksSettings,
    keys: RwLock<KeySet>,
    /// Serializes fetches. Holds the time of the last refetch triggered by an
    /// unknown `kid`.
    fetch: Mutex<Option<Instant>>,
}

#[derive(Default)]
struct KeySet {
    keys: HashMap<String, VerifyingKey>,
    /// Time of the last successful fetch; `None` until the first one.
    loaded_at: Option<Instant>,
}

impl Jwks {
    pub fn new(config: &Config) -> Result<Self, String> {
        Self::with_settings(
            &config.jwks_url,
            config.environment == Environment::Production,
            JwksSettings::default(),
        )
    }

    /// `https_only` refuses plain http, including through a redirect.
    pub fn with_settings(
        url: &str,
        https_only: bool,
        settings: JwksSettings,
    ) -> Result<Self, String> {
        let client = http_client(https_only, settings.fetch_timeout)?;
        Ok(Self {
            inner: Arc::new(Inner {
                url: url.to_owned(),
                client,
                settings,
                keys: RwLock::new(KeySet::default()),
                fetch: Mutex::new(None),
            }),
        })
    }

    /// True once a key set has been loaded. `/healthz` reports not ready
    /// until then.
    pub fn is_loaded(&self) -> bool {
        self.read().loaded_at.is_some()
    }

    /// The key for `kid`, refetching the set once when the `kid` is unknown
    /// or the set unusable, unless another refetch happened less than
    /// [`JwksSettings::min_refetch_interval`] ago.
    pub async fn key(&self, kid: &str) -> Lookup {
        match self.cached(kid) {
            Lookup::Found(key) => return Lookup::Found(key),
            Lookup::UnknownKid | Lookup::Unavailable => {}
        }
        self.refetch_on_demand().await;
        self.cached(kid)
    }

    fn cached(&self, kid: &str) -> Lookup {
        let set = self.read();
        let usable = set
            .loaded_at
            .is_some_and(|at| at.elapsed() <= self.inner.settings.max_stale);
        if !usable {
            return Lookup::Unavailable;
        }
        match set.keys.get(kid) {
            Some(key) => Lookup::Found(*key),
            None => Lookup::UnknownKid,
        }
    }

    async fn refetch_on_demand(&self) {
        // Requests arriving during a fetch wait for it, then see its result.
        let mut last = self.inner.fetch.lock().await;
        if last.is_some_and(|at| at.elapsed() < self.inner.settings.min_refetch_interval) {
            return;
        }
        *last = Some(Instant::now());
        tracing::info!("unknown kid or no usable key: refetching the JWKS");
        // Failures are logged by `fetch_and_store`.
        let _ = self.fetch_and_store().await;
    }

    /// Fetches the set now and replaces the cache on success. On failure the
    /// previous set is kept.
    pub async fn refresh(&self) -> Result<usize, FetchError> {
        let _guard = self.inner.fetch.lock().await;
        self.fetch_and_store().await
    }

    async fn fetch_and_store(&self) -> Result<usize, FetchError> {
        let result = self.fetch().await;
        let outcome = if result.is_ok() { "ok" } else { "error" };
        metrics::counter!(names::JWKS_FETCHES_TOTAL, "result" => outcome).increment(1);
        match result {
            Ok(keys) => {
                let count = keys.len();
                let mut set = self
                    .inner
                    .keys
                    .write()
                    .unwrap_or_else(PoisonError::into_inner);
                let changed = set.loaded_at.is_none()
                    || set.keys.len() != keys.len()
                    || !keys.keys().all(|kid| set.keys.contains_key(kid));
                set.keys = keys;
                set.loaded_at = Some(Instant::now());
                let mut kids: Vec<&str> = set.keys.keys().map(String::as_str).collect();
                kids.sort_unstable();
                if changed {
                    tracing::info!(kids = ?kids, "JWKS loaded");
                } else {
                    tracing::debug!(kids = ?kids, "JWKS refreshed");
                }
                metrics::gauge!(names::JWKS_KEYS).set(count as f64);
                Ok(count)
            }
            Err(error) => {
                let last_success = self.read().loaded_at.map(|at| at.elapsed().as_secs());
                match last_success {
                    None => tracing::error!(
                        error = %error,
                        "cannot load the JWKS: authenticated requests answer 503 until it loads"
                    ),
                    Some(age) if age > self.inner.settings.max_stale.as_secs() => {
                        tracing::error!(
                            error = %error,
                            last_success_secs_ago = age,
                            "JWKS refresh failed and the cached keys are too old: tokens are refused"
                        );
                    }
                    Some(age) => tracing::warn!(
                        error = %error,
                        last_success_secs_ago = age,
                        "JWKS refresh failed, keeping the previous keys"
                    ),
                }
                Err(error)
            }
        }
    }

    async fn fetch(&self) -> Result<HashMap<String, VerifyingKey>, FetchError> {
        let max = self.inner.settings.max_body_bytes;
        let mut response = self
            .inner
            .client
            .get(&self.inner.url)
            .header(
                reqwest::header::ACCEPT,
                "application/jwk-set+json, application/json",
            )
            .send()
            .await
            .map_err(|e| FetchError::Request(e.to_string()))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|len| len > max as u64)
        {
            return Err(FetchError::TooLarge(max));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| FetchError::Request(e.to_string()))?
        {
            if body.len() + chunk.len() > max {
                return Err(FetchError::TooLarge(max));
            }
            body.extend_from_slice(&chunk);
        }
        parse_jwks(&body)
    }

    /// Fetches at once, then keeps the set fresh until the task is aborted.
    /// Failures are retried with an exponential backoff, never slower than
    /// the normal refresh period.
    pub fn spawn_refresh(&self) -> JoinHandle<()> {
        let jwks = self.clone();
        tokio::spawn(async move {
            let settings = &jwks.inner.settings;
            let mut retry = settings.min_retry_interval;
            loop {
                let wait = match jwks.refresh().await {
                    Ok(_) => {
                        retry = settings.min_retry_interval;
                        settings.refresh_interval
                    }
                    Err(_) => {
                        let wait = retry.min(settings.refresh_interval);
                        retry = (retry * 2).min(settings.max_retry_interval);
                        wait
                    }
                };
                tokio::time::sleep(wait).await;
            }
        })
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, KeySet> {
        // The lock only guards plain data: a panic while holding it cannot
        // leave the set half-written in a harmful way.
        self.inner
            .keys
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Makes the cached set look `age` old. Tests only.
    #[cfg(any(test, feature = "test-util"))]
    pub fn age_by(&self, age: Duration) {
        let mut set = self
            .inner
            .keys
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        set.loaded_at = set.loaded_at.and_then(|at| at.checked_sub(age));
    }
}

/// The HTTP client: rustls with the `ring` provider and the Mozilla roots
/// (`webpki-roots`, as sqlx does), no redirects, short timeouts.
fn http_client(https_only: bool, timeout: Duration) -> Result<reqwest::Client, String> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS configuration: {e}"))?
    .with_root_certificates(roots)
    .with_no_client_auth();

    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .https_only(https_only)
        // A redirect could lead to plain http or another host: the JWKS URL
        // must point at the document itself.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(timeout)
        .user_agent(concat!("pulsars-api/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("cannot build the JWKS HTTP client: {e}"))
}

/// Keeps the usable keys of a JWK set (RFC 7517), indexed by `kid`.
fn parse_jwks(body: &[u8]) -> Result<HashMap<String, VerifyingKey>, FetchError> {
    let document: Value =
        serde_json::from_slice(body).map_err(|e| FetchError::Invalid(e.to_string()))?;
    let entries = document
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| FetchError::Invalid("no `keys` array".to_owned()))?;

    let mut keys = HashMap::new();
    for entry in entries {
        let kid = entry.get("kid").and_then(Value::as_str).unwrap_or_default();
        match usable_key(entry) {
            Ok(key) => match keys.entry(kid.to_owned()) {
                Entry::Occupied(_) => {
                    tracing::warn!(kid, "duplicate kid in the JWKS, keeping the first key");
                }
                Entry::Vacant(slot) => {
                    slot.insert(key);
                }
            },
            Err(Skip::PrivateKey) => {
                // A published private key is compromised: never use it, and
                // make sure someone notices.
                tracing::error!(kid, "the JWKS publishes a private key (`d`); key ignored");
            }
            Err(Skip::Unsupported(why)) => {
                tracing::debug!(kid, why, "JWKS key ignored");
            }
        }
    }
    if keys.is_empty() {
        return Err(FetchError::NoUsableKey);
    }
    Ok(keys)
}

enum Skip {
    PrivateKey,
    Unsupported(&'static str),
}

fn usable_key(entry: &Value) -> Result<VerifyingKey, Skip> {
    let field = |name: &str| entry.get(name).and_then(Value::as_str);
    if field("kid").is_none_or(str::is_empty) {
        return Err(Skip::Unsupported("no kid"));
    }
    if field("kty") != Some("OKP") || field("crv") != Some("Ed25519") {
        return Err(Skip::Unsupported("not an Ed25519 key"));
    }
    if field("alg") != Some("EdDSA") {
        return Err(Skip::Unsupported("alg is not EdDSA"));
    }
    if entry.get("use").is_some_and(|u| u.as_str() != Some("sig")) {
        return Err(Skip::Unsupported("not a signing key"));
    }
    if entry.get("d").is_some() {
        return Err(Skip::PrivateKey);
    }
    let x = field("x").ok_or(Skip::Unsupported("no x"))?;
    let bytes: [u8; 32] = URL_SAFE_NO_PAD
        .decode(x)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(Skip::Unsupported("x is not 32 base64url bytes"))?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| Skip::Unsupported("x is not a valid point"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::testing::{jwk, signing_key};
    use serde_json::json;

    fn parse(document: &Value) -> Result<Vec<String>, FetchError> {
        let mut kids: Vec<String> = parse_jwks(document.to_string().as_bytes())?
            .into_keys()
            .collect();
        kids.sort();
        Ok(kids)
    }

    #[test]
    fn keeps_only_ed25519_signing_keys() {
        let key = signing_key();
        let good = jwk(&key, "good");
        let mut no_alg = jwk(&key, "no-alg");
        no_alg.as_object_mut().map(|o| o.remove("alg"));
        let mut rsa = jwk(&key, "rsa");
        rsa["kty"] = json!("RSA");
        let mut x25519 = jwk(&key, "x25519");
        x25519["crv"] = json!("X25519");
        let mut hs = jwk(&key, "hs");
        hs["alg"] = json!("HS256");
        let mut enc = jwk(&key, "enc");
        enc["use"] = json!("enc");
        let mut private = jwk(&key, "private");
        private["d"] = json!("nWGxne_9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A");
        let mut short = jwk(&key, "short");
        short["x"] = json!("AAAA");
        let mut no_kid = jwk(&key, "");
        no_kid.as_object_mut().map(|o| o.remove("kid"));
        let mut no_use = jwk(&key, "no-use");
        no_use.as_object_mut().map(|o| o.remove("use"));

        let kids = parse(
            &json!({"keys": [good, no_alg, rsa, x25519, hs, enc, private,
            short, no_kid, no_use]}),
        )
        .expect("usable keys");
        assert_eq!(kids, vec!["good", "no-use"]);
    }

    #[test]
    fn the_rfc_8037_example_key_is_accepted() {
        let document = json!({"keys": [{
            "kty": "OKP", "crv": "Ed25519", "alg": "EdDSA", "use": "sig",
            "kid": "2026-10-ed25519-1",
            "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
        }]});
        assert_eq!(parse(&document).expect("valid"), vec!["2026-10-ed25519-1"]);
    }

    #[test]
    fn a_set_without_usable_keys_is_an_error() {
        assert!(matches!(
            parse(&json!({"keys": []})),
            Err(FetchError::NoUsableKey)
        ));
        assert!(matches!(
            parse(&json!({"nokeys": []})),
            Err(FetchError::Invalid(_))
        ));
        assert!(matches!(parse_jwks(b"<html>"), Err(FetchError::Invalid(_))));
    }

    #[test]
    fn duplicate_kids_keep_the_first_key() {
        let first = signing_key();
        let second = signing_key();
        let keys = parse_jwks(
            json!({"keys": [jwk(&first, "k"), jwk(&second, "k")]})
                .to_string()
                .as_bytes(),
        )
        .expect("valid");
        assert_eq!(keys.get("k"), Some(&first.verifying_key()));
    }

    #[tokio::test]
    async fn nothing_is_usable_before_the_first_fetch() {
        let jwks = Jwks::with_settings(
            "http://127.0.0.1:1/jwks.json",
            false,
            JwksSettings {
                fetch_timeout: Duration::from_millis(500),
                ..JwksSettings::default()
            },
        )
        .expect("client");
        assert!(!jwks.is_loaded());
        assert_eq!(jwks.key("any").await, Lookup::Unavailable);
        assert!(jwks.refresh().await.is_err());
        assert!(!jwks.is_loaded());
    }
}
