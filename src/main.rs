//! Entry point: configuration, listeners and graceful shutdown.

use std::process::ExitCode;
use std::sync::Arc;

use fred::prelude::ClientLike as _;
use pulsars_api::config::Config;
use pulsars_api::state::AppState;
use pulsars_api::{db, redis, routes, telemetry};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// `EX_CONFIG` from sysexits.h: systemd and operators can tell a
/// configuration error from a crash.
const EXIT_CONFIG: u8 = 78;

#[tokio::main]
async fn main() -> ExitCode {
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

    match run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "fatal error");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> Result<(), String> {
    let metrics = telemetry::install_metrics_recorder()
        .map_err(|e| format!("cannot install the metrics recorder: {e}"))?;
    let db = db::connect_lazy(&config).map_err(|e| format!("invalid DATABASE_URL: {e}"))?;
    let redis = redis::connect(&config).map_err(|e| format!("invalid REDIS_URL: {e}"))?;

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
    let public_server =
        axum::serve(public, routes::router(state)).with_graceful_shutdown(stopped(stop_rx.clone()));
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
