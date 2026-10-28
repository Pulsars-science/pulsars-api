//! The member extractors (`src/auth/member.rs`), through `GET /v1/auth/session`
//! and the shared harness (`tests/common`).

mod common;

use axum::http::StatusCode;
use common::{Call, Env};

#[tokio::test]
async fn a_planted_member_has_a_live_session() {
    let Some(env) = Env::new().await else {
        return;
    };
    let ada = env.member().verified().role("moderateur").create().await;
    let (status, body) = Call::get("/v1/auth/session")
        .member(&ada)
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["user"]["pseudo"], ada.pseudo.as_str());

    let (status, _) = Call::get("/v1/auth/session")
        .session(&"x".repeat(43))
        .send(&env.app)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
