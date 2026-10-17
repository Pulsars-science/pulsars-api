//! Typed configuration, read from environment variables once at startup.
//!
//! Every problem is collected before failing, so a misconfigured deployment
//! reports all of its mistakes at once instead of one per restart. The
//! service refuses to start while any value is missing or invalid.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use axum::http::HeaderValue;

/// Origin of the game front end in production.
pub const GAME_ORIGIN: &str = "https://game.pulsars.science";

/// Origin of the Vite dev server, allowed only in development.
pub const DEV_GAME_ORIGIN: &str = "http://localhost:5173";

/// The Laravel site: issuer of game tokens and home of member profiles.
pub const SITE_URL: &str = "https://pulsars.science";

/// `aud` of the game tokens meant for this API.
pub const API_AUDIENCE: &str = "api.pulsars.science";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

/// A value that must never appear in logs or `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(\"***\")")
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    /// Public HTTP listener.
    pub bind_addr: SocketAddr,
    /// Prometheus listener. Always a loopback or private address.
    pub metrics_addr: SocketAddr,
    pub database_url: Secret,
    pub database_max_connections: u32,
    pub redis_url: Secret,
    /// Laravel's public keys (`docs/contract/auth-jwt.md` §3). HTTPS only in
    /// production.
    pub jwks_url: String,
    /// Expected `iss` of game tokens.
    pub jwt_issuer: String,
    /// Expected `aud` of game tokens.
    pub jwt_audience: String,
    /// Base URL of Laravel's `public` disk, without trailing slash. Avatar
    /// URLs are `<base>/<avatar_path>`.
    pub public_storage_url: String,
    /// Origins allowed to call authenticated and game endpoints with
    /// credentials. Public content endpoints allow any origin.
    pub cors_allowed_origins: Vec<HeaderValue>,
    /// Requests per minute per client IP without a valid token
    /// (`docs/contract/README.md`, rate limits).
    pub rate_limit_anonymous: u32,
    /// Requests per minute per token `sub`.
    pub rate_limit_authenticated: u32,
    pub request_timeout: Duration,
    pub body_limit_bytes: usize,
    pub log_format: LogFormat,
    pub log_filter: String,
    /// How long in-flight requests get to finish after SIGTERM.
    pub shutdown_grace: Duration,
    /// Where match results go (`docs/contract/webhooks.md`). `None`: the
    /// outbox fills up and nothing is sent.
    pub webhook: Option<WebhookConfig>,
}

/// `LARAVEL_WEBHOOK_URL`, `LARAVEL_WEBHOOK_SECRET` and the optional
/// `LARAVEL_WEBHOOK_HOST`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookConfig {
    /// Laravel's receiver, usually on the loopback or a private network:
    /// plain http is accepted in production.
    pub url: String,
    /// At least 32 characters (`GAME_WEBHOOK_SECRET` on the Laravel side).
    pub secret: Secret,
    /// `Host` header to send instead of the URL's host.
    pub host: Option<String>,
}

/// Shortest webhook secret accepted.
const WEBHOOK_SECRET_MIN_CHARS: usize = 32;

/// All the problems found in the environment.
#[derive(Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub problems: Vec<String>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "invalid configuration ({} problem(s)):",
            self.problems.len()
        )?;
        for problem in &self.problems {
            writeln!(f, "  - {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Reads the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_vars(std::env::vars())
    }

    /// Reads any set of variables. Used by tests.
    pub fn from_vars<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars: HashMap<String, String> = vars
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .filter(|(_, v)| !v.trim().is_empty())
            .collect();
        let mut reader = Reader {
            vars: &vars,
            problems: Vec::new(),
        };

        let environment = reader.required("PULSARS_ENV", |v| match v {
            "development" => Ok(Environment::Development),
            "production" => Ok(Environment::Production),
            _ => Err("must be `development` or `production`".to_owned()),
        });
        let bind_addr = reader.optional("BIND_ADDR", "127.0.0.1:8080", parse_socket_addr);
        let metrics_addr = reader.optional("METRICS_ADDR", "127.0.0.1:9464", parse_socket_addr);
        let database_url = reader.required("DATABASE_URL", |v| {
            parse_url_scheme(v, &["postgres", "postgresql"]).map(|()| Secret::new(v))
        });
        let database_max_connections =
            reader.optional("DATABASE_MAX_CONNECTIONS", "10", |v| parse_range(v, 1, 200));
        let redis_url = reader.required("REDIS_URL", |v| {
            parse_url_scheme(v, &["redis", "rediss"]).map(|()| Secret::new(v))
        });
        let rate_limit_anonymous = reader.optional("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "60", |v| {
            parse_range(v, 1, 1_000_000)
        });
        let rate_limit_authenticated =
            reader.optional("RATE_LIMIT_AUTHENTICATED_PER_MINUTE", "120", |v| {
                parse_range(v, 1, 1_000_000)
            });
        let request_timeout = reader
            .optional("REQUEST_TIMEOUT_SECS", "10", |v| parse_range(v, 1, 120))
            .map(|s| Duration::from_secs(u64::from(s)));
        let body_limit_bytes = reader
            .optional("BODY_LIMIT_BYTES", "65536", |v| {
                parse_range(v, 1024, 10 * 1024 * 1024)
            })
            .map(|b| b as usize);
        let log_format = reader.optional("LOG_FORMAT", "json", |v| match v {
            "json" => Ok(LogFormat::Json),
            "pretty" => Ok(LogFormat::Pretty),
            _ => Err("must be `json` or `pretty`".to_owned()),
        });
        let log_filter = reader.optional("RUST_LOG", "info,sqlx=warn,tower_http=info", |v| {
            Ok(v.to_owned())
        });
        let shutdown_grace = reader
            .optional("SHUTDOWN_GRACE_SECS", "20", |v| parse_range(v, 1, 300))
            .map(|s| Duration::from_secs(u64::from(s)));
        let origins_raw = reader.optional_raw("CORS_ALLOWED_ORIGINS");
        let jwks_raw = reader.required("JWKS_URL", |v| Ok(v.to_owned()));
        let jwt_issuer = reader.optional("JWT_ISSUER", SITE_URL, |v| Ok(v.to_owned()));
        let jwt_audience = reader.optional("JWT_AUDIENCE", API_AUDIENCE, |v| Ok(v.to_owned()));
        let storage_raw = reader.optional_raw("PUBLIC_STORAGE_URL");
        let webhook = parse_webhook(
            reader.optional_raw("LARAVEL_WEBHOOK_URL"),
            reader.optional_raw("LARAVEL_WEBHOOK_SECRET"),
            reader.optional_raw("LARAVEL_WEBHOOK_HOST"),
            &mut reader.problems,
        );

        // Cross-field checks need the individual values first.
        let cors_allowed_origins = environment.and_then(|env| {
            let origins = match origins_raw {
                Some(raw) => raw.split(',').map(|o| o.trim().to_owned()).collect(),
                None => default_origins(env),
            };
            match parse_origins(&origins, env) {
                Ok(parsed) => Some(parsed),
                Err(message) => {
                    reader
                        .problems
                        .push(format!("CORS_ALLOWED_ORIGINS: {message}"));
                    None
                }
            }
        });

        // Over plain http, anyone on the path could swap the keys.
        let jwks_url = match (environment, jwks_raw) {
            (Some(env), Some(raw)) => match parse_http_url(&raw, env) {
                Ok(url) => Some(url),
                Err(message) => {
                    reader.problems.push(format!("JWKS_URL: {message}"));
                    None
                }
            },
            _ => None,
        };
        let public_storage_url = environment.and_then(|env| {
            let raw = storage_raw.unwrap_or_else(|| format!("{SITE_URL}/storage"));
            match parse_http_url(&raw, env) {
                Ok(url) => Some(url.trim_end_matches('/').to_owned()),
                Err(message) => {
                    reader
                        .problems
                        .push(format!("PUBLIC_STORAGE_URL: {message}"));
                    None
                }
            }
        });

        if let (Some(bind), Some(metrics)) = (bind_addr, metrics_addr) {
            if !is_internal(metrics.ip()) {
                reader.problems.push(format!(
                    "METRICS_ADDR: {metrics} is not a loopback or private address; \
                     metrics must never be exposed publicly"
                ));
            }
            if bind.port() == metrics.port()
                && (bind.ip() == metrics.ip() || bind.ip().is_unspecified())
            {
                reader
                    .problems
                    .push("METRICS_ADDR: must not use the same port as BIND_ADDR".to_owned());
            }
        }

        if !reader.problems.is_empty() {
            return Err(ConfigError {
                problems: reader.problems,
            });
        }

        // Every value is `Some` when there is no problem.
        match (
            environment,
            bind_addr,
            metrics_addr,
            database_url,
            database_max_connections,
            redis_url,
            jwks_url,
            jwt_issuer,
            jwt_audience,
            public_storage_url,
            cors_allowed_origins,
            rate_limit_anonymous,
            rate_limit_authenticated,
            request_timeout,
            body_limit_bytes,
            log_format,
            log_filter,
            shutdown_grace,
        ) {
            (
                Some(environment),
                Some(bind_addr),
                Some(metrics_addr),
                Some(database_url),
                Some(database_max_connections),
                Some(redis_url),
                Some(jwks_url),
                Some(jwt_issuer),
                Some(jwt_audience),
                Some(public_storage_url),
                Some(cors_allowed_origins),
                Some(rate_limit_anonymous),
                Some(rate_limit_authenticated),
                Some(request_timeout),
                Some(body_limit_bytes),
                Some(log_format),
                Some(log_filter),
                Some(shutdown_grace),
            ) => Ok(Self {
                environment,
                bind_addr,
                metrics_addr,
                database_url,
                database_max_connections,
                redis_url,
                jwks_url,
                jwt_issuer,
                jwt_audience,
                public_storage_url,
                cors_allowed_origins,
                rate_limit_anonymous,
                rate_limit_authenticated,
                request_timeout,
                body_limit_bytes,
                log_format,
                log_filter,
                shutdown_grace,
                webhook,
            }),
            _ => Err(ConfigError {
                problems: vec!["internal: a value was not parsed".to_owned()],
            }),
        }
    }
}

struct Reader<'a> {
    vars: &'a HashMap<String, String>,
    problems: Vec<String>,
}

impl Reader<'_> {
    fn required<T>(
        &mut self,
        name: &str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        match self.vars.get(name) {
            Some(value) => self.parse(name, value.trim(), parse),
            None => {
                self.problems.push(format!("{name}: missing"));
                None
            }
        }
    }

    fn optional<T>(
        &mut self,
        name: &str,
        default: &str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        let value = self.vars.get(name).map_or(default, |v| v.trim());
        self.parse(name, value, parse)
    }

    fn optional_raw(&self, name: &str) -> Option<String> {
        self.vars.get(name).map(|v| v.trim().to_owned())
    }

    fn parse<T>(
        &mut self,
        name: &str,
        value: &str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        match parse(value) {
            Ok(parsed) => Some(parsed),
            Err(message) => {
                // Never echo the value: it may be a secret.
                self.problems.push(format!("{name}: {message}"));
                None
            }
        }
    }
}

fn parse_socket_addr(value: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|_| "must be an IP address and port, e.g. 127.0.0.1:8080".to_owned())
}

fn parse_range(value: &str, min: u32, max: u32) -> Result<u32, String> {
    match value.parse::<u32>() {
        Ok(n) if (min..=max).contains(&n) => Ok(n),
        _ => Err(format!("must be an integer between {min} and {max}")),
    }
}

fn parse_url_scheme(value: &str, schemes: &[&str]) -> Result<(), String> {
    let scheme = value.split_once("://").map(|(s, _)| s);
    match scheme {
        Some(s) if schemes.contains(&s) => Ok(()),
        _ => Err(format!(
            "must be a URL starting with {}://",
            schemes.join(":// or ")
        )),
    }
}

/// An absolute http(s) URL with a host and no credentials; http only in
/// development. The value is not secret, so it may be echoed.
fn parse_http_url(value: &str, environment: Environment) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "must be an absolute URL".to_owned())?;
    match (url.scheme(), environment) {
        ("https", _) | ("http", Environment::Development) => {}
        ("http", Environment::Production) => {
            return Err("must use https in production".to_owned());
        }
        _ => return Err("must be an http:// or https:// URL".to_owned()),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("must have a host".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("must not contain credentials".to_owned());
    }
    Ok(value.to_owned())
}

/// Both the URL and the secret, or neither.
fn parse_webhook(
    url: Option<String>,
    secret: Option<String>,
    host: Option<String>,
    problems: &mut Vec<String>,
) -> Option<WebhookConfig> {
    let (url, secret) = match (url, secret) {
        (None, None) => {
            if host.is_some() {
                problems.push("LARAVEL_WEBHOOK_HOST: requires LARAVEL_WEBHOOK_URL".to_owned());
            }
            return None;
        }
        (Some(_), None) => {
            problems.push("LARAVEL_WEBHOOK_SECRET: required with LARAVEL_WEBHOOK_URL".to_owned());
            return None;
        }
        (None, Some(_)) => {
            problems.push("LARAVEL_WEBHOOK_URL: required with LARAVEL_WEBHOOK_SECRET".to_owned());
            return None;
        }
        (Some(url), Some(secret)) => (url, secret),
    };
    let mut ok = true;
    // Laravel listens on the loopback or a private network: http is fine
    // in production too.
    if let Err(message) = parse_http_url(&url, Environment::Development) {
        problems.push(format!("LARAVEL_WEBHOOK_URL: {message}"));
        ok = false;
    }
    if secret.chars().count() < WEBHOOK_SECRET_MIN_CHARS {
        problems.push(format!(
            "LARAVEL_WEBHOOK_SECRET: must be at least {WEBHOOK_SECRET_MIN_CHARS} characters"
        ));
        ok = false;
    }
    if let Some(host) = &host
        && (host.is_empty()
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':')))
    {
        problems.push("LARAVEL_WEBHOOK_HOST: must be a host name, e.g. pulsars.science".to_owned());
        ok = false;
    }
    ok.then(|| WebhookConfig {
        url,
        secret: Secret::new(secret),
        host,
    })
}

fn default_origins(environment: Environment) -> Vec<String> {
    match environment {
        Environment::Production => vec![GAME_ORIGIN.to_owned()],
        Environment::Development => vec![GAME_ORIGIN.to_owned(), DEV_GAME_ORIGIN.to_owned()],
    }
}

fn parse_origins(origins: &[String], environment: Environment) -> Result<Vec<HeaderValue>, String> {
    let mut parsed = Vec::with_capacity(origins.len());
    for origin in origins {
        if origin == "*" {
            return Err("`*` is not allowed: these origins receive credentials".to_owned());
        }
        let (scheme, rest) = origin
            .split_once("://")
            .ok_or_else(|| format!("`{origin}` is not an origin (scheme://host[:port])"))?;
        if rest.is_empty() || rest.contains('/') {
            return Err(format!("`{origin}` must have no path or trailing slash"));
        }
        match (scheme, environment) {
            ("https", _) | ("http", Environment::Development) => {}
            ("http", Environment::Production) => {
                return Err(format!(
                    "`{origin}` uses http, which is refused in production"
                ));
            }
            _ => return Err(format!("`{origin}` has an unsupported scheme")),
        }
        let value = HeaderValue::from_str(origin)
            .map_err(|_| format!("`{origin}` is not a valid header value"))?;
        parsed.push(value);
    }
    if parsed.is_empty() {
        return Err("at least one origin is required".to_owned());
    }
    Ok(parsed)
}

fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        // fc00::/7: unique local addresses.
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Vec<(&'static str, &'static str)> {
        vec![
            ("PULSARS_ENV", "production"),
            (
                "DATABASE_URL",
                "postgres://pulsars_api:secret@localhost/pulsars",
            ),
            ("REDIS_URL", "redis://localhost:6379"),
            ("JWKS_URL", "https://pulsars.science/.well-known/jwks.json"),
        ]
    }

    fn with(extra: &[(&'static str, &'static str)]) -> Result<Config, ConfigError> {
        let mut vars = base();
        for (k, v) in extra {
            vars.retain(|(key, _)| key != k);
            vars.push((k, v));
        }
        Config::from_vars(vars)
    }

    #[test]
    fn minimal_production_config_uses_defaults() {
        let config = with(&[]).expect("valid config");
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:8080");
        assert_eq!(config.metrics_addr.to_string(), "127.0.0.1:9464");
        assert_eq!(
            config.cors_allowed_origins,
            vec![HeaderValue::from_static(GAME_ORIGIN)]
        );
        assert_eq!(config.body_limit_bytes, 65536);
        assert_eq!(config.log_format, LogFormat::Json);
        assert_eq!(config.jwt_issuer, "https://pulsars.science");
        assert_eq!(config.jwt_audience, "api.pulsars.science");
        assert_eq!(config.public_storage_url, "https://pulsars.science/storage");
        assert_eq!(config.rate_limit_anonymous, 60);
        assert_eq!(config.rate_limit_authenticated, 120);
    }

    #[test]
    fn jwks_url_must_be_https_in_production() {
        let err = with(&[("JWKS_URL", "http://pulsars.test/.well-known/jwks.json")])
            .expect_err("must fail");
        assert_eq!(err.problems, vec!["JWKS_URL: must use https in production"]);
        let dev = with(&[
            ("PULSARS_ENV", "development"),
            ("JWKS_URL", "http://localhost:8000/.well-known/jwks.json"),
        ])
        .expect("http is fine in development");
        assert_eq!(dev.jwks_url, "http://localhost:8000/.well-known/jwks.json");
    }

    #[test]
    fn rejects_unusable_jwks_urls() {
        for bad in [
            "pulsars.science/.well-known/jwks.json",
            "ftp://pulsars.science/jwks.json",
            "https://user:pass@pulsars.science/jwks.json",
        ] {
            let err = with(&[("JWKS_URL", bad)]).expect_err(bad);
            assert!(err.problems[0].starts_with("JWKS_URL: "), "{bad}");
            assert!(
                !err.problems[0].contains("pass@"),
                "never echoes credentials"
            );
        }
    }

    #[test]
    fn storage_url_loses_its_trailing_slash() {
        let config =
            with(&[("PUBLIC_STORAGE_URL", "https://cdn.example/storage/")]).expect("valid");
        assert_eq!(config.public_storage_url, "https://cdn.example/storage");
        assert!(with(&[("PUBLIC_STORAGE_URL", "http://cdn.example/storage")]).is_err());
    }

    #[test]
    fn development_adds_the_vite_origin() {
        let config = with(&[("PULSARS_ENV", "development")]).expect("valid config");
        assert!(
            config
                .cors_allowed_origins
                .contains(&HeaderValue::from_static(DEV_GAME_ORIGIN))
        );
    }

    #[test]
    fn reports_every_missing_variable_at_once() {
        let err = Config::from_vars(Vec::<(String, String)>::new()).expect_err("must fail");
        assert_eq!(
            err.problems,
            vec![
                "PULSARS_ENV: missing",
                "DATABASE_URL: missing",
                "REDIS_URL: missing",
                "JWKS_URL: missing"
            ]
        );
    }

    #[test]
    fn empty_values_count_as_missing() {
        let err = with(&[("REDIS_URL", "  ")]).expect_err("must fail");
        assert_eq!(err.problems, vec!["REDIS_URL: missing"]);
    }

    #[test]
    fn never_echoes_secret_values() {
        let err =
            with(&[("DATABASE_URL", "mysql://user:hunter2@db/pulsars")]).expect_err("must fail");
        let text = err.to_string();
        assert!(text.contains("DATABASE_URL"));
        assert!(!text.contains("hunter2"));
    }

    #[test]
    fn debug_output_hides_secrets() {
        let config = with(&[]).expect("valid config");
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret@"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn refuses_public_metrics_address() {
        let err = with(&[("METRICS_ADDR", "0.0.0.0:9464")]).expect_err("must fail");
        assert!(err.problems[0].starts_with("METRICS_ADDR"));
        assert!(with(&[("METRICS_ADDR", "10.0.0.5:9464")]).is_ok());
        assert!(with(&[("METRICS_ADDR", "[::1]:9464")]).is_ok());
    }

    #[test]
    fn refuses_metrics_on_the_public_port() {
        let err = with(&[("BIND_ADDR", "0.0.0.0:9464")]).expect_err("must fail");
        assert!(err.problems.iter().any(|p| p.contains("same port")));
    }

    #[test]
    fn refuses_wildcard_and_http_origins_in_production() {
        assert!(with(&[("CORS_ALLOWED_ORIGINS", "*")]).is_err());
        assert!(with(&[("CORS_ALLOWED_ORIGINS", "http://game.pulsars.science")]).is_err());
        assert!(with(&[("CORS_ALLOWED_ORIGINS", "https://game.pulsars.science/")]).is_err());
        let ok = with(&[(
            "CORS_ALLOWED_ORIGINS",
            "https://a.example, https://b.example",
        )])
        .expect("valid");
        assert_eq!(ok.cors_allowed_origins.len(), 2);
    }

    #[test]
    fn the_webhook_needs_its_url_and_secret_together() {
        assert_eq!(with(&[]).expect("valid").webhook, None);
        let secret = "0123456789abcdef0123456789abcdef";
        let config = with(&[
            (
                "LARAVEL_WEBHOOK_URL",
                "http://127.0.0.1:8000/internal/jeu/evenements",
            ),
            ("LARAVEL_WEBHOOK_SECRET", secret),
            ("LARAVEL_WEBHOOK_HOST", "pulsars.science"),
        ])
        .expect("http on the loopback is fine in production");
        let webhook = config.webhook.expect("configured");
        assert_eq!(webhook.host.as_deref(), Some("pulsars.science"));
        assert!(!format!("{webhook:?}").contains(secret));

        let err =
            with(&[("LARAVEL_WEBHOOK_URL", "http://127.0.0.1:8000/x")]).expect_err("no secret");
        assert_eq!(
            err.problems,
            vec!["LARAVEL_WEBHOOK_SECRET: required with LARAVEL_WEBHOOK_URL"]
        );
        let err = with(&[
            ("LARAVEL_WEBHOOK_URL", "ftp://x/y"),
            ("LARAVEL_WEBHOOK_SECRET", "short"),
        ])
        .expect_err("invalid");
        assert_eq!(err.problems.len(), 2);
        assert!(!err.to_string().contains("short"));
    }

    #[test]
    fn rejects_out_of_range_numbers() {
        let err = with(&[
            ("DATABASE_MAX_CONNECTIONS", "0"),
            ("REQUEST_TIMEOUT_SECS", "abc"),
            ("RATE_LIMIT_ANONYMOUS_PER_MINUTE", "0"),
        ])
        .expect_err("must fail");
        assert_eq!(err.problems.len(), 3);
    }
}
