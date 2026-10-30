//! Pulsars API: the back end of pulsars.science (content, members, forum,
//! social, administration) and its game server.
//!
//! See `docs/architecture.md` and `docs/contract/` for what this service
//! does and the rules it follows.

pub mod auth;
pub mod cli;
pub mod config;
pub mod content;
pub mod crypto;
pub mod db;
pub mod deepl;
pub mod error;
pub mod extract;
pub mod flash;
pub mod game;
pub mod http_client;
pub mod http_daily;
pub mod links;
pub mod mail;
pub mod middleware;
pub mod podman;
pub mod rate_limit;
pub mod redis;
pub mod routes;
pub mod state;
pub mod telemetry;
pub mod umami;
