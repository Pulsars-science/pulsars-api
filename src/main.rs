//! Entry point: configuration, listeners and graceful shutdown.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use fred::prelude::ClientLike as _;
use pulsars_api::auth::Jwks;
use pulsars_api::config::Config;
use pulsars_api::game::season;
use pulsars_api::game::webhook::{self, WebhookSettings};
use pulsars_api::game::{Game, GameSettings, store};
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes, telemetry};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// `EX_CONFIG` from sysexits.h: systemd and operators can tell a
/// configuration error from a crash.
const EXIT_CONFIG: u8 = 78;

#[tokio::main]
async fn main() -> ExitCode {
    // Needs no configuration: it makes the key the configuration names.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [command, kid, dir] = args.as_slice()
        && command == "jwt-keygen"
    {
        return jwt_keygen(kid, dir);
    }

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            // Logging is not set up yet: plain stderr.
            eprintln!("{error}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };

    if let Err(error) = telemetry::init_logging(config.log_format, &config.log_filter) {
        eprintln!("{error}");
        return ExitCode::from(EXIT_CONFIG);
    }

    // `pulsars-api migrate`, `pulsars-api check-content`,
    // `pulsars-api webhooks replay <event_id>` (docs/contract/webhooks.md §7).
    if !args.is_empty() {
        return command(&config, &args).await;
    }

    match run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "fatal error");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage: pulsars-api [migrate | check-content | webhooks replay <event_id> \
                     | jwt-keygen <kid> <dir>]";

/// `pulsars-api jwt-keygen <kid> <dir>`: writes a new Ed25519 seed to
/// `<dir>/<kid>.key` (mode 0600, never over an existing file), in the format
/// `JWT_SIGNING_KEY_FILE` reads, and prints the public key.
fn jwt_keygen(kid: &str, dir: &str) -> ExitCode {
    use base64::Engine as _;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use std::io::Write as _;

    if !pulsars_api::config::is_valid_kid(kid) {
        eprintln!("kid must read YYYY-MM-ed25519-N, e.g. 2026-10-ed25519-1");
        return ExitCode::from(EXIT_USAGE);
    }
    let mut seed = [0u8; 32];
    if let Err(error) = getrandom::fill(&mut seed) {
        eprintln!("no randomness: {error}");
        return ExitCode::FAILURE;
    }
    let path = std::path::Path::new(dir).join(format!("{kid}.key"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let written = options
        .open(&path)
        .and_then(|mut file| writeln!(file, "{}", STANDARD.encode(seed)));
    if let Err(error) = written {
        eprintln!("{}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    let public = ed25519_dalek::SigningKey::from_bytes(&seed).verifying_key();
    println!("wrote {}", path.display());
    println!("x = {}", URL_SAFE_NO_PAD.encode(public.as_bytes()));
    println!("JWT_SIGNING_KEY_FILE={}", path.display());
    println!("JWT_SIGNING_KID={kid}");
    ExitCode::SUCCESS
}

/// One-off commands: `migrate`, `check-content` and
/// `webhooks replay <event_id>`.
async fn command(config: &Config, args: &[String]) -> ExitCode {
    if let [command] = args {
        match command.as_str() {
            "migrate" => return migrate_command(config).await,
            "check-content" => return check_content_command(config).await,
            _ => {}
        }
    }
    let [group, action, event_id] = args else {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    if group != "webhooks" || action != "replay" {
        eprintln!("usage: pulsars-api [webhooks replay <event_id>]");
        return ExitCode::from(EXIT_USAGE);
    }
    let Ok(event_id) = uuid::Uuid::parse_str(event_id) else {
        eprintln!("not a UUID: {event_id}");
        return ExitCode::from(EXIT_USAGE);
    };
    let pool = match db::connect_lazy(config) {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("invalid DATABASE_URL: {error}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    match webhook::replay(&pool, event_id).await {
        Ok(true) => {
            println!("event {event_id} queued for delivery");
            ExitCode::SUCCESS
        }
        Ok(false) => {
            eprintln!("no webhook event {event_id}");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("database error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `pulsars-api check-content`: compares what the API reads of `contenu`
/// with the `api_read` views it replaces, on the real data. Fails when a
/// single row differs (deploy/sql/README.md, phase 1).
async fn check_content_command(config: &Config) -> ExitCode {
    let pool = match db::connect_lazy(config) {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("invalid DATABASE_URL: {error}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let comparisons = match db::sources::compare_with_views(&pool).await {
        Ok(comparisons) => comparisons,
        Err(error) => {
            eprintln!("database error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut same = true;
    for c in &comparisons {
        let verdict = if c.is_same() { "same" } else { "DIFFERENT" };
        println!(
            "{:<24} {:>6} rows  {:>4} missing  {:>4} extra  {verdict}",
            c.view, c.rows, c.missing, c.extra
        );
        same &= c.is_same();
    }
    if same {
        println!("content: the API reads exactly what the api_read views show");
        ExitCode::SUCCESS
    } else {
        eprintln!("content: some rows differ from the api_read views");
        ExitCode::FAILURE
    }
}

/// `EX_USAGE` from sysexits.h.
const EXIT_USAGE: u8 = 64;

/// `pulsars-api migrate`: applies the embedded migrations, then exits.
///
/// Connects with `DATABASE_MIGRATION_URL` (the `pulsars_migrate` role, which
/// may create tables), or `DATABASE_URL` when it is not set, as long as the
/// service's role still owns its schemas. `deploy-api` runs it before
/// switching to a new release; the service itself never migrates.
async fn migrate_command(config: &Config) -> ExitCode {
    let url = std::env::var("DATABASE_MIGRATION_URL")
        .ok()
        .filter(|url| !url.is_empty());
    let source = if url.is_some() {
        "DATABASE_MIGRATION_URL"
    } else {
        "DATABASE_URL"
    };
    let url = url.unwrap_or_else(|| config.database_url.expose().to_owned());
    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("cannot connect with {source}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let before = match store::pending(&pool).await {
        Ok(pending) => pending,
        Err(error) => {
            eprintln!("cannot read the applied migrations: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = store::migrate(&pool).await {
        eprintln!("migrations failed (with {source}): {error}");
        return ExitCode::FAILURE;
    }
    match before.as_slice() {
        [] => println!("migrations: nothing to apply"),
        versions => println!(
            "migrations: applied {}",
            versions
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
    pool.close().await;
    ExitCode::SUCCESS
}

/// Waits until every embedded migration is applied, checking again with a
/// growing delay while the database is down or behind. The service does not
/// migrate (its role may not create tables): `pulsars-api migrate` does.
async fn wait_for_schema(pool: sqlx::PgPool) {
    let mut wait = std::time::Duration::from_secs(1);
    loop {
        match store::pending(&pool).await {
            Ok(pending) if pending.is_empty() => {
                tracing::info!("database schema up to date");
                return;
            }
            Ok(pending) => tracing::error!(pending = ?pending, retry_secs = wait.as_secs(),
                "migrations not applied: run `pulsars-api migrate`"),
            Err(error) => tracing::error!(error = %error, retry_secs = wait.as_secs(),
                "cannot check the database schema"),
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(std::time::Duration::from_secs(60));
    }
}

async fn run(config: Config) -> Result<(), String> {
    let metrics = telemetry::install_metrics_recorder()
        .map_err(|e| format!("cannot install the metrics recorder: {e}"))?;
    let db = db::connect_lazy(&config).map_err(|e| format!("invalid DATABASE_URL: {e}"))?;
    let redis = redis::connect(&config).map_err(|e| format!("invalid REDIS_URL: {e}"))?;
    // Loaded in the background: Laravel being down at boot must not stop the
    // API. `/healthz` answers 503 until the first key is loaded.
    let jwks = Jwks::new(&config)?;
    let jwks_refresh = jwks.spawn_refresh();
    // The monthly season close needs the `game.seasons` migration: it starts
    // once the migrations are applied (docs/contract/brains.md §3).
    let migrations = tokio::spawn({
        let db = db.clone();
        async move {
            wait_for_schema(db.clone()).await;
            // Runs forever; aborted at shutdown with this task.
            season::run_closer(db).await;
        }
    });
    let webhooks = match &config.webhook {
        Some(settings) => Some(webhook::spawn_worker(
            db.clone(),
            WebhookSettings::new(
                settings.url.clone(),
                settings.secret.clone(),
                settings.host.clone(),
            ),
        )?),
        None => {
            tracing::warn!(
                "LARAVEL_WEBHOOK_URL is not set: match results stay in the outbox, unsent"
            );
            None
        }
    };
    let game = Game::start(db.clone(), GameSettings::default());

    let public = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| format!("cannot listen on {}: {e}", config.bind_addr))?;
    let internal = TcpListener::bind(config.metrics_addr)
        .await
        .map_err(|e| format!("cannot listen on {}: {e}", config.metrics_addr))?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        environment = ?config.environment,
        bind_addr = %config.bind_addr,
        metrics_addr = %config.metrics_addr,
        "pulsars-api starting"
    );

    let grace = config.shutdown_grace;
    let state = AppState {
        config: Arc::new(config),
        db: db.clone(),
        redis: redis.clone(),
        jwks,
        game,
    };

    // The exporter needs periodic upkeep to drain histogram buffers.
    let upkeep_handle = metrics.clone();
    let upkeep = tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tick.tick().await;
            upkeep_handle.run_upkeep();
        }
    });

    let (stop_tx, stop_rx) = watch::channel(false);
    // The peer address is the fallback client IP in logs.
    let public_server = axum::serve(
        public,
        routes::router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(stopped(stop_rx.clone()));
    let internal_server = axum::serve(internal, routes::metrics_router(metrics))
        .with_graceful_shutdown(stopped(stop_rx.clone()));

    let servers = async {
        let (public_result, internal_result) = tokio::join!(public_server, internal_server);
        public_result.and(internal_result)
    };
    tokio::pin!(servers);

    let result = tokio::select! {
        result = &mut servers => result.map_err(|e| format!("server error: {e}")),
        () = shutdown_signal() => {
            tracing::info!(grace_secs = grace.as_secs(), "shutdown requested, draining requests");
            let _ = stop_tx.send(true);
            match tokio::time::timeout(grace, &mut servers).await {
                Ok(result) => result.map_err(|e| format!("server error: {e}")),
                Err(_) => {
                    tracing::warn!("grace period elapsed, closing remaining connections");
                    Ok(())
                }
            }
        }
    };

    upkeep.abort();
    jwks_refresh.abort();
    migrations.abort();
    if let Some(webhooks) = webhooks {
        webhooks.abort();
    }
    if redis.is_connected()
        && let Err(error) = redis.quit().await
    {
        tracing::warn!(error = %error, "redis did not close cleanly");
    }
    db.close().await;
    tracing::info!("pulsars-api stopped");
    result
}

async fn stopped(mut rx: watch::Receiver<bool>) {
    // An error means the sender is gone, which also means stop.
    let _ = rx.wait_for(|stop| *stop).await;
}

/// Resolves on SIGTERM (systemd, Docker) or Ctrl-C.
async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
