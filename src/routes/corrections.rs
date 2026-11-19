//! What the exam page adds to the statement (`openapi.yaml`, tag `exams`),
//! as Laravel's `ExamenController::show`, `corrige` and `download`: the
//! model answer of each part, the correction PDF, and the counters.
//!
//! Laravel showed the corrections of a published exam to everyone, on its
//! page; so does this route, for published exams only (a draft is the
//! admin's, step E). The content routes still never carry them
//! (`db::sources`).

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::response::Response;
use serde::Serialize;

use super::auth::Caller;
use super::public::{served, translated};
use super::site::{self, not_found, ok};
use super::views;
use crate::auth::member::MaybeMember;
use crate::db::exams;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct Corrections {
    pub parts: Vec<PartCorrection>,
    pub has_correction_pdf: bool,
    pub views: i64,
    pub downloads: i64,
    pub downloaders: i64,
}

#[derive(Debug, Serialize)]
pub struct PartCorrection {
    pub part_id: i64,
    pub position: i32,
    pub lang: &'static str,
    pub correction_md: String,
}

/// `GET /v1/exams/{slug}/corrections?lang=`: the exam page calls it on
/// every display, and it counts the view (`CompteurVues`: not again for
/// the same member, or visitor fingerprint, within 20 seconds).
pub async fn corrections(
    State(state): State<AppState>,
    MaybeMember(member): MaybeMember,
    caller: Caller,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, ApiError> {
    let lang = site::lang(query.as_deref())?;
    let mut counters = exams::counters(&state.db, &slug).await?.ok_or_else(|| {
        not_found(
            format!("No published exam has the slug '{slug}'."),
            uri.path(),
        )
    })?;
    let who = views::reader(&state, member.as_ref().map(|m| m.id()), &caller).await?;
    if views::first_view(&state, "exams", counters.id, &who).await {
        exams::add_view(&state.db, counters.id).await?;
        counters.views += 1;
    }
    let parts = exams::corrections(&state.db, counters.id, lang.translation()).await?;
    Ok(ok(Corrections {
        parts: parts
            .into_iter()
            .map(|p| {
                let (correction_md, is_tr) = translated(p.correction_md, p.tr_correction_md);
                PartCorrection {
                    part_id: p.id,
                    position: p.position,
                    lang: served(lang, is_tr),
                    correction_md,
                }
            })
            .collect(),
        has_correction_pdf: counters.has_correction_pdf,
        views: counters.views,
        downloads: counters.downloads,
        downloaders: counters.downloaders,
    }))
}

/// Counts a download of the published exam `slug` (statement or
/// correction), unless a robot asks: every download, and the visitor once
/// among the downloaders.
pub(crate) async fn count_download(
    state: &AppState,
    slug: &str,
    member: Option<i64>,
    caller: &Caller,
) -> Result<(), ApiError> {
    if site::is_robot(caller.user_agent.as_deref()) {
        return Ok(());
    }
    let Some(id) = exams::published_id(&state.db, slug).await? else {
        return Ok(());
    };
    let hash =
        site::visitor_hash(state, caller.ip.as_deref(), caller.user_agent.as_deref()).await?;
    exams::add_download(&state.db, id, &hash, member).await?;
    Ok(())
}
