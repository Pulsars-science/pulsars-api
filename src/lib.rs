//! Pulsars API: public read-only content API and game server.
//!
//! See `docs/architecture.md` and `docs/contract/` for what this service
//! does and the rules it follows.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod flash;
pub mod links;
pub mod middleware;
pub mod rate_limit;
pub mod redis;
pub mod routes;
pub mod state;
pub mod telemetry;
