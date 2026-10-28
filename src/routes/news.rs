//! `GET /v1/news` and `/v1/news/{slug}` (`openapi.yaml`, tag `news`): the
//! announcements of the team, as the site's Actualité page shows them.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::HeaderMap;
use serde::Serialize;

use super::courses::timestamp;
use super::cursor;
use super::params::{Lang, Params};
use super::public::{self, Data, Pagination, PublicResult, served, translated};
use crate::db::news::{self, NewsDetailRow, NewsRow};
use crate::links;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct News {
    pub id: i64,
    pub slug: String,
    pub lang: &'static str,
    pub title: String,
    pub summary: Option<String>,
    /// Pseudo of the team member who wrote it.
    pub author: Option<String>,
    pub published_at: String,
    pub updated_at: String,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct NewsDetail {
    #[serde(flatten)]
    pub news: News,
    pub content_md: String,
}

pub async fn list(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    let limit = params.limit();
    let cursor = params.cursor();
    let search = params.search();
    let filters = params.finish()?;

    let pagination = Pagination::new("news", lang, &filters, limit);
    let after = match cursor {
        Some(c) => Some(cursor::time_id_key(&cursor::decode(
            &c,
            &pagination.fingerprint,
        )?)?),
        None => None,
    };
    let rows = news::list(
        &state.db,
        lang.translation(),
        search.as_deref(),
        after,
        pagination.fetch(),
    )
    .await?;
    let page = pagination.page(
        rows,
        |row| cursor::time_id(row.published_at, row.id),
        |row| to_news(row, lang),
    );
    public::json(&headers, lang, &page)
}

pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;

    let row = news::find(&state.db, lang.translation(), &slug)
        .await?
        .ok_or_else(|| {
            public::not_found(
                format!("No published news item has the slug '{slug}'."),
                uri.path(),
            )
        })?;
    public::json(
        &headers,
        lang,
        &Data {
            data: to_detail(row, lang),
        },
    )
}

fn to_news(row: NewsRow, lang: Lang) -> News {
    let (title, is_tr) = translated(row.title, row.tr_title);
    let summary = match row.summary {
        Some(original) => Some(translated(original, row.tr_summary).0),
        None => row.tr_summary.filter(|s| !s.is_empty()),
    };
    News {
        id: row.id,
        web_url: links::news(&row.slug),
        slug: row.slug,
        lang: served(lang, is_tr),
        title,
        summary,
        author: row.author_pseudo,
        published_at: timestamp(row.published_at),
        updated_at: timestamp(row.updated_at.unwrap_or(row.published_at)),
    }
}

fn to_detail(row: NewsDetailRow, lang: Lang) -> NewsDetail {
    NewsDetail {
        content_md: translated(row.content_md, row.tr_content_md).0,
        news: to_news(row.news, lang),
    }
}
