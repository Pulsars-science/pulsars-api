//! Redis client: rate limits, and matchmaking queues and pub/sub in later
//! phases.
//!
//! `fred` is used rather than `redis`: it reconnects on its own with a
//! configurable policy, has first-class pub/sub (needed to run several API
//! instances side by side), pooling and command timeouts, without extra
//! glue code.

use std::time::Duration;

use fred::prelude::{Builder, Client, ClientLike, Config as RedisConfig, ReconnectPolicy};

use crate::config::Config;
use crate::db::CheckError;

/// Builds the client and starts connecting in the background. A Redis that is
/// down at startup does not stop the API: content stays readable and
/// `/healthz` reports `degraded`.
pub fn connect(config: &Config) -> Result<Client, fred::error::Error> {
    let mut redis_config = RedisConfig::from_url(config.redis_url.expose())?;
    // With `fail_fast`, a Redis that is down at startup would never be
    // retried: the reconnect policy must apply to the first connection too.
    redis_config.fail_fast = false;
    let client = Builder::from_config(redis_config)
        .with_connection_config(|c| {
            c.connection_timeout = Duration::from_secs(2);
            c.internal_command_timeout = Duration::from_secs(2);
        })
        .with_performance_config(|p| {
            p.default_command_timeout = Duration::from_secs(2);
        })
        // Retry forever, from 100 ms up to 5 s between attempts.
        .set_policy(ReconnectPolicy::new_exponential(0, 100, 5_000, 2))
        .build()?;
    // The handle is not awaited: the connection task lives as long as the
    // client and is stopped by `quit()` on shutdown.
    let _connection_task = client.connect();
    Ok(client)
}

pub async fn ping(client: &Client, timeout: Duration) -> Result<(), CheckError> {
    if !client.is_connected() {
        return Err(CheckError::ConnectionRefused);
    }
    match tokio::time::timeout(timeout, client.ping::<String>(None)).await {
        Err(_) => Err(CheckError::Timeout),
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => match error.kind() {
            fred::error::ErrorKind::Auth => Err(CheckError::AuthFailed),
            fred::error::ErrorKind::Timeout => Err(CheckError::Timeout),
            _ => Err(CheckError::Other),
        },
    }
}
