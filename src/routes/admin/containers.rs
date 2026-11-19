//! `GET /v1/admin/containers`: the health of the server's containers, from
//! the snapshot `pulsars-podman-releve` writes every minute
//! (`crate::podman`, `deploy/podman/README.md`). The API never talks to
//! Podman itself.

use axum::extract::State;
use axum::response::Response;
use chrono::Utc;

use super::ok;
use crate::auth::member::AdminMember;
use crate::error::ApiError;
use crate::podman;
use crate::state::AppState;

/// Always 200: a snapshot that is not configured, missing or unreadable is
/// `available: false` with its `reason`.
pub async fn containers(
    State(state): State<AppState>,
    _admin: AdminMember,
) -> Result<Response, ApiError> {
    let path = state.config.podman_snapshot_path.as_deref();
    Ok(ok(podman::read(path, Utc::now()).await))
}
