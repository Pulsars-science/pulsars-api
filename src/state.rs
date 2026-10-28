//! State shared by every request handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::game::Game;
use crate::mail::Mailer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub redis: fred::prelude::Client,
    /// The game server (matchmaking, rooms, matches).
    pub game: Game,
    /// Account mails (verification, password reset).
    pub mailer: Mailer,
}
