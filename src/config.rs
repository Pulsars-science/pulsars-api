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
pub const GAME_ORIGIN: &str = "https://pulsars.science";

/// Origin of the Vite dev server, allowed only in development.
pub const DEV_GAME_ORIGIN: &str = "http://localhost:5173";

/// The Laravel site: issuer of game tokens and home of member profiles.
pub const SITE_URL: &str = "https://pulsars.science";

/// `aud` of the game tokens meant for this API.
pub const API_AUDIENCE: &str = "api.pulsars.science";

/// `iss` of the game tokens the API signs itself (`JWT_OWN_ISSUER`).
pub const API_ISSUER: &str = "https://api.pulsars.science";

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
    /// `SERVICE_KEY`: requests that carry it in `Pulsars-Service-Key` (the
    /// site, from the same machine) are not rate limited. `None`: every
    /// request is.
    pub service_key: Option<Secret>,
    /// The key private messages are sealed with (`MESSAGES_KEY`,
    /// `MESSAGES_KEY_VERSION`). Required in production; `None` in
    /// development: the private message routes answer 503.
    pub messages_key: Option<MessagesKey>,
    /// `ARGON2_MEMORY_KIB`: memory cost of the password hashes the API
    /// writes (19 456 KiB). Lowered only by the tests.
    pub argon2_memory_kib: u32,
    /// The key the API signs game tokens with (`JWT_SIGNING_KEY_FILE`,
    /// `JWT_SIGNING_KID`). `None`: `POST /v1/auth/token` answers 503.
    pub token_signer: Option<TokenSigner>,
    /// Where account mails go (`MAIL_SMTP_URL`, `MAIL_FROM`). `None`: they
    /// are only logged, without their link.
    pub mail: Option<MailConfig>,
    /// Translation of forum posts (`DEEPL_API_KEY`, `DEEPL_API_URL`,
    /// `DEEPL_LANGUAGES`). `None` without a key: the translation routes
    /// answer 503 `not_configured`, as Laravel did.
    pub deepl: Option<DeeplConfig>,
}

/// DeepL, which translates forum posts (Laravel's `services.deepl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeeplConfig {
    pub key: Secret,
    /// `https://api-free.deepl.com` or `https://api.deepl.com`, without
    /// trailing slash.
    pub url: String,
    /// The site's languages and DeepL's `target_lang` for each.
    pub languages: Vec<(String, String)>,
}

impl DeeplConfig {
    /// DeepL's `target_lang` for one of the site's languages.
    pub fn target(&self, lang: &str) -> Option<&str> {
        self.languages
            .iter()
            .find(|(site, _)| site == lang)
            .map(|(_, deepl)| deepl.as_str())
    }
}

/// DeepL's free endpoint, as in Laravel's `config/services.php`.
pub const DEEPL_URL: &str = "https://api-free.deepl.com";

/// The site's languages and their DeepL target, as in Laravel.
pub const DEEPL_LANGUAGES: &str = "fr=FR,en=EN-GB,ru=RU";

/// The SMTP server of account mails (`docs/contract/auth-sessions.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailConfig {
    /// `smtp://host:port` or `smtps://user:password@host:port`, as lettre
    /// reads it.
    pub smtp_url: Secret,
    /// `Pulsars <noreply@blackenium.com>`.
    pub from: String,
}

/// Sender of account mails when `MAIL_FROM` is not set, as in the site's
/// `docs/EMAIL.md`.
pub const MAIL_FROM: &str = "Pulsars <noreply@blackenium.com>";

/// The AES-256-GCM key of private messages (`src/crypto.rs`): 32 random
/// bytes in base64 (`openssl rand -base64 32`), and the version stored with
/// each message it seals, so a later key can take over.
#[derive(Clone, PartialEq, Eq)]
pub struct MessagesKey {
    pub version: i16,
    pub key: [u8; 32],
}

impl fmt::Debug for MessagesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessagesKey")
            .field("version", &self.version)
            .field("key", &"***")
            .finish()
    }
}

/// What the API needs to issue game tokens (`docs/contract/auth-jwt.md`).
#[derive(Clone)]
pub struct TokenSigner {
    /// `YYYY-MM-ed25519-N`.
    pub kid: String,
    pub key: ed25519_dalek::SigningKey,
    /// `JWT_OWN_ISSUER`, `https://api.pulsars.science` by default.
    pub issuer: String,
    /// `JWT_TOKEN_TTL_SECS`, 300 by default.
    pub ttl_secs: i64,
}

impl fmt::Debug for TokenSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenSigner")
            .field("kid", &self.kid)
            .field("key", &"***")
            .field("issuer", &self.issuer)
            .field("ttl_secs", &self.ttl_secs)
            .finish()
    }
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

/// Shortest service key accepted.
const SERVICE_KEY_MIN_CHARS: usize = 32;

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
        let argon2_memory_kib = reader.optional("ARGON2_MEMORY_KIB", "19456", |v| {
            parse_range(v, 8, 4_194_304)
        });
        let mail_from = reader.optional("MAIL_FROM", MAIL_FROM, |v| {
            v.parse::<lettre::message::Mailbox>()
                .map(|_| v.to_owned())
                .map_err(|e| format!("must be a mailbox such as `{MAIL_FROM}`: {e}"))
        });
        let mail = match reader.optional_raw("MAIL_SMTP_URL") {
            None => None,
            Some(url) if url.starts_with("smtp://") || url.starts_with("smtps://") => {
                mail_from.clone().map(|from| MailConfig {
                    smtp_url: Secret::new(url),
                    from,
                })
            }
            Some(_) => {
                reader
                    .problems
                    .push("MAIL_SMTP_URL: must start with smtp:// or smtps://".to_owned());
                None
            }
        };
        let signer_file = reader.optional_raw("JWT_SIGNING_KEY_FILE");
        let signer_kid = reader.optional_raw("JWT_SIGNING_KID");
        let own_issuer = reader.optional("JWT_OWN_ISSUER", API_ISSUER, |v| Ok(v.to_owned()));
        let token_ttl = reader.optional("JWT_TOKEN_TTL_SECS", "300", |v| parse_range(v, 60, 3600));
        let deepl_key = reader.optional_raw("DEEPL_API_KEY");
        let deepl_url_raw = reader.optional_raw("DEEPL_API_URL");
        let deepl_languages =
            reader.optional("DEEPL_LANGUAGES", DEEPL_LANGUAGES, parse_deepl_languages);
        let service_key = match reader.optional_raw("SERVICE_KEY") {
            Some(key) if key.chars().count() < SERVICE_KEY_MIN_CHARS => {
                reader.problems.push(format!(
                    "SERVICE_KEY: must be at least {SERVICE_KEY_MIN_CHARS} characters"
                ));
                None
            }
            key => key.map(Secret::new),
        };

        let messages_key_raw = reader.optional_raw("MESSAGES_KEY");
        let messages_key_version = reader.optional("MESSAGES_KEY_VERSION", "1", |v| {
            parse_range(v, 1, i16::MAX as u32).map(|n| n as i16)
        });
        let messages_key = match (messages_key_raw, environment) {
            (Some(raw), _) => match parse_messages_key(&raw) {
                Ok(key) => messages_key_version.map(|version| MessagesKey { version, key }),
                Err(message) => {
                    reader.problems.push(format!("MESSAGES_KEY: {message}"));
                    None
                }
            },
            (None, Some(Environment::Production)) => {
                reader.problems.push(
                    "MESSAGES_KEY: required in production (32 random bytes in base64: \
                     openssl rand -base64 32)"
                        .to_owned(),
                );
                None
            }
            (None, _) => None,
        };

        let token_signer = match (signer_file, signer_kid) {
            (None, None) => None,
            (Some(file), Some(kid)) => match load_signer(&file, &kid, environment) {
                Ok((kid, key)) => Some(TokenSigner {
                    kid,
                    key,
                    issuer: own_issuer.clone().unwrap_or_else(|| API_ISSUER.to_owned()),
                    ttl_secs: i64::from(token_ttl.unwrap_or(300)),
                }),
                Err(message) => {
                    reader.problems.push(message);
                    None
                }
            },
            (Some(_), None) => {
                reader
                    .problems
                    .push("JWT_SIGNING_KID: required with JWT_SIGNING_KEY_FILE".to_owned());
                None
            }
            (None, Some(_)) => {
                reader
                    .problems
                    .push("JWT_SIGNING_KEY_FILE: required with JWT_SIGNING_KID".to_owned());
                None
            }
        };

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

        let deepl_url = environment.and_then(|env| {
            let raw = deepl_url_raw.unwrap_or_else(|| DEEPL_URL.to_owned());
            match parse_http_url(&raw, env) {
                Ok(url) => Some(url.trim_end_matches('/').to_owned()),
                Err(message) => {
                    reader.problems.push(format!("DEEPL_API_URL: {message}"));
                    None
                }
            }
        });
        let deepl = match (deepl_key, deepl_url, deepl_languages) {
            (Some(key), Some(url), Some(languages)) => Some(DeeplConfig {
                key: Secret::new(key),
                url,
                languages,
            }),
            _ => None,
        };

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
                service_key,
                messages_key,
                argon2_memory_kib: argon2_memory_kib
                    .unwrap_or(crate::auth::password::DEFAULT_MEMORY_KIB),
                token_signer,
                mail,
                deepl,
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

/// Whether `kid` reads `YYYY-MM-ed25519-N` (Laravel's `ClesJeu` format).
pub fn is_valid_kid(kid: &str) -> bool {
    let Some((date, counter)) = kid.split_once("-ed25519-") else {
        return false;
    };
    let date_ok = date.len() == 7
        && date.as_bytes()[4] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 4 || b.is_ascii_digit());
    let counter_ok = !counter.is_empty()
        && counter.len() <= 6
        && !counter.starts_with('0')
        && counter.bytes().all(|b| b.is_ascii_digit());
    date_ok && counter_ok
}

/// The 32-byte Ed25519 seed in `path`, base64 (as Laravel's
/// `storage/app/private/jeu/<kid>.key`). In production the file must not be
/// readable by its group or others.
fn load_signer(
    path: &str,
    kid: &str,
    environment: Option<Environment>,
) -> Result<(String, ed25519_dalek::SigningKey), String> {
    use base64::Engine as _;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

    if !is_valid_kid(kid) {
        return Err("JWT_SIGNING_KID: must read YYYY-MM-ed25519-N".to_owned());
    }
    let problem = |message: String| format!("JWT_SIGNING_KEY_FILE: {message}");
    let metadata = std::fs::metadata(path).map_err(|e| problem(format!("{path}: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if environment == Some(Environment::Production)
            && metadata.permissions().mode() & 0o077 != 0
        {
            return Err(problem(format!(
                "{path} is readable by its group or others; chmod 600 it"
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = (&metadata, environment);
    let raw = std::fs::read_to_string(path).map_err(|e| problem(format!("{path}: {e}")))?;
    let raw = raw.trim();
    let seed = STANDARD
        .decode(raw)
        .or_else(|_| URL_SAFE_NO_PAD.decode(raw))
        .map_err(|_| problem(format!("{path} is not base64")))?;
    let seed: [u8; 32] = seed
        .try_into()
        .map_err(|_| problem(format!("{path} does not hold a 32-byte seed")))?;
    Ok((kid.to_owned(), ed25519_dalek::SigningKey::from_bytes(&seed)))
}

/// `fr=FR,en=EN-GB`: the site's language, `=`, DeepL's `target_lang`.
fn parse_deepl_languages(value: &str) -> Result<Vec<(String, String)>, String> {
    let message = || "must look like `fr=FR,en=EN-GB,ru=RU`".to_owned();
    value
        .split(',')
        .map(|pair| {
            let (site, deepl) = pair.split_once('=').ok_or_else(message)?;
            let (site, deepl) = (site.trim(), deepl.trim());
            let valid = |s: &str| {
                !s.is_empty()
                    && s.len() <= 10
                    && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            };
            if valid(site) && valid(deepl) {
                Ok((site.to_owned(), deepl.to_owned()))
            } else {
                Err(message())
            }
        })
        .collect()
}

/// 32 bytes in standard base64, padded or not. The value is never echoed.
fn parse_messages_key(value: &str) -> Result<[u8; 32], String> {
    use base64::Engine as _;
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};

    let bytes = STANDARD
        .decode(value)
        .or_else(|_| STANDARD_NO_PAD.decode(value))
        .map_err(|_| "is not base64".to_owned())?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
        format!(
            "must decode to 32 bytes, not {} (openssl rand -base64 32)",
            bytes.len()
        )
    })
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
            (
                "MESSAGES_KEY",
                "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=",
            ),
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
        assert!(with(&[("CORS_ALLOWED_ORIGINS", "http://pulsars.science")]).is_err());
        assert!(with(&[("CORS_ALLOWED_ORIGINS", "https://pulsars.science/")]).is_err());
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
    fn deepl_needs_a_key_and_has_laravel_defaults() {
        assert_eq!(with(&[]).expect("valid").deepl, None);
        let config = with(&[("DEEPL_API_KEY", "a-key:fx")]).expect("valid");
        let deepl = config.deepl.expect("configured");
        assert_eq!(deepl.url, "https://api-free.deepl.com");
        assert_eq!(deepl.target("en"), Some("EN-GB"));
        assert_eq!(deepl.target("ru"), Some("RU"));
        assert_eq!(deepl.target("de"), None);
        assert!(!format!("{deepl:?}").contains("a-key"));

        let config = with(&[
            ("DEEPL_API_KEY", "k"),
            ("DEEPL_API_URL", "https://api.deepl.com/"),
            ("DEEPL_LANGUAGES", "fr=FR, en=EN-US"),
        ])
        .expect("valid");
        let deepl = config.deepl.expect("configured");
        assert_eq!(deepl.url, "https://api.deepl.com");
        assert_eq!(deepl.target("en"), Some("EN-US"));

        let error = with(&[
            ("DEEPL_API_URL", "http://deepl.example"),
            ("DEEPL_LANGUAGES", "fr"),
        ])
        .expect_err("invalid");
        assert_eq!(
            error.problems,
            vec![
                "DEEPL_LANGUAGES: must look like `fr=FR,en=EN-GB,ru=RU`",
                "DEEPL_API_URL: must use https in production",
            ]
        );
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

    #[test]
    fn kids_follow_the_laravel_format() {
        for kid in ["2026-10-ed25519-1", "2027-01-ed25519-123456"] {
            assert!(is_valid_kid(kid), "{kid}");
        }
        for kid in [
            "",
            "2026-10-ed25519-0",
            "2026-10-ed25519-01",
            "26-10-ed25519-1",
            "2026-10-rsa-1",
            "2026-10-ed25519-1234567",
            "2026_10-ed25519-1",
        ] {
            assert!(!is_valid_kid(kid), "{kid}");
        }
    }

    #[test]
    fn the_signing_key_is_read_from_its_file() {
        use base64::Engine as _;
        let dir = std::env::temp_dir().join(format!("pulsars-key-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("k.key");
        std::fs::write(
            &path,
            format!(
                "{}\n",
                base64::engine::general_purpose::STANDARD.encode([7u8; 32])
            ),
        )
        .unwrap();
        // `with` takes static strings: a test may leak one.
        let file: &'static str = Box::leak(path.to_str().unwrap().to_owned().into_boxed_str());

        let config = with(&[
            ("JWT_SIGNING_KEY_FILE", file),
            ("JWT_SIGNING_KID", "2026-10-ed25519-1"),
        ]);
        #[cfg(unix)]
        {
            // Readable by others: refused in production (base() is production).
            let problems = config.unwrap_err().problems;
            assert_eq!(problems.len(), 1, "{problems:?}");
            assert!(problems[0].contains("chmod 600"), "{problems:?}");
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let config = with(&[
            ("JWT_SIGNING_KEY_FILE", file),
            ("JWT_SIGNING_KID", "2026-10-ed25519-1"),
        ])
        .unwrap();
        let signer = config.token_signer.expect("signer");
        assert_eq!(signer.kid, "2026-10-ed25519-1");
        assert_eq!(signer.issuer, API_ISSUER);
        assert_eq!(signer.ttl_secs, 300);
        assert_eq!(signer.key.to_bytes(), [7u8; 32]);
        assert!(
            !format!("{signer:?}").contains("[7"),
            "the key never shows in Debug"
        );

        let problems = with(&[("JWT_SIGNING_KEY_FILE", file)])
            .unwrap_err()
            .problems;
        assert_eq!(
            problems,
            ["JWT_SIGNING_KID: required with JWT_SIGNING_KEY_FILE"]
        );
        let problems = with(&[("JWT_SIGNING_KEY_FILE", file), ("JWT_SIGNING_KID", "bad")])
            .unwrap_err()
            .problems;
        assert_eq!(problems, ["JWT_SIGNING_KID: must read YYYY-MM-ed25519-N"]);
        std::fs::write(&path, "not base64!").unwrap();
        let problems = with(&[
            ("JWT_SIGNING_KEY_FILE", file),
            ("JWT_SIGNING_KID", "2026-10-ed25519-1"),
        ])
        .unwrap_err()
        .problems;
        assert!(problems[0].ends_with("is not base64"), "{problems:?}");
        assert!(with(&[]).unwrap().token_signer.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mail_needs_an_smtp_url_and_a_mailbox() {
        assert_eq!(with(&[]).unwrap().mail, None);
        let config = with(&[("MAIL_SMTP_URL", "smtp://127.0.0.1:1025")]).unwrap();
        assert_eq!(
            config.mail,
            Some(MailConfig {
                smtp_url: Secret::new("smtp://127.0.0.1:1025"),
                from: MAIL_FROM.to_owned(),
            })
        );
        let problems = with(&[
            ("MAIL_SMTP_URL", "http://x"),
            ("MAIL_FROM", "not a mailbox"),
        ])
        .unwrap_err()
        .problems;
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("MAIL_FROM: "), "{problems:?}");
        assert_eq!(
            problems[1],
            "MAIL_SMTP_URL: must start with smtp:// or smtps://"
        );
    }

    #[test]
    fn a_service_key_must_be_long() {
        let problems = with(&[("SERVICE_KEY", "short")]).unwrap_err().problems;
        assert_eq!(problems, ["SERVICE_KEY: must be at least 32 characters"]);
        let key = "s".repeat(32);
        let config =
            Config::from_vars(base().into_iter().chain([("SERVICE_KEY", key.as_str())])).unwrap();
        assert_eq!(config.service_key, Some(Secret::new(key.clone())));
        assert_eq!(with(&[]).unwrap().service_key, None);
    }

    #[test]
    fn the_messages_key_is_32_bytes_of_base64() {
        let config = with(&[]).unwrap();
        let key = config.messages_key.expect("a key");
        assert_eq!((key.version, key.key), (1, [7u8; 32]));
        assert!(
            !format!("{key:?}").contains('7'),
            "the key never shows in Debug"
        );

        let config = with(&[("MESSAGES_KEY_VERSION", "3")]).unwrap();
        assert_eq!(config.messages_key.map(|k| k.version), Some(3));
        let problems = with(&[("MESSAGES_KEY_VERSION", "0")]).unwrap_err().problems;
        assert_eq!(
            problems,
            ["MESSAGES_KEY_VERSION: must be an integer between 1 and 32767"]
        );

        let problems = with(&[("MESSAGES_KEY", "c2hvcnQ=")]).unwrap_err().problems;
        assert_eq!(
            problems,
            ["MESSAGES_KEY: must decode to 32 bytes, not 5 (openssl rand -base64 32)"]
        );
        let problems = with(&[("MESSAGES_KEY", "not base64!")])
            .unwrap_err()
            .problems;
        assert_eq!(problems, ["MESSAGES_KEY: is not base64"]);

        // Required in production, optional in development.
        let vars = base().into_iter().filter(|(k, _)| *k != "MESSAGES_KEY");
        let problems = Config::from_vars(vars).unwrap_err().problems;
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("MESSAGES_KEY: required in production"));
        let vars = base()
            .into_iter()
            .filter(|(k, _)| *k != "MESSAGES_KEY")
            .map(|(k, v)| (k, if k == "PULSARS_ENV" { "development" } else { v }));
        assert_eq!(Config::from_vars(vars).unwrap().messages_key, None);
    }
}
