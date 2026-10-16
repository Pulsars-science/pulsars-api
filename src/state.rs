//! State shared by every request handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::Jwks;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub redis: fred::prelude::Client,
    /// Laravel's public keys, to verify game tokens.
    pub jwks: Jwks,
}
