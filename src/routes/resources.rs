//! `GET /v1/resources` and `/v1/resources/{slug}` (`openapi.yaml`, tag
//! `resources`): free documents with their licence and attribution.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::HeaderMap;
use serde::Serialize;

use super::courses::timestamp;
use super::cursor;
use super::params::{Params, RESOURCE_MODES};
use super::public::{self, Data, Pagination, PublicResult, served, translated};
use crate::db::resources::{self, ResourceFilters, ResourceRow};
use crate::links;
use crate::state::AppState;

/// Licence key whose name is the uploader's free text.
const CUSTOM_LICENSE: &str = "autre";

#[derive(Debug, Serialize)]
pub struct Resource {
    pub id: i64,
    pub slug: String,
    pub lang: &'static str,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub doc_lang: Option<String>,
    pub mode: String,
    pub attribution: Option<String>,
    pub license: Option<License>,
    pub download_url: Option<String>,
    pub source_url: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct License {
    pub key: String,
    pub name: String,
    pub url: Option<String>,
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
    let category = params.text("category", 120);
    let doc_lang = params.text("doc_lang", 8);
    let mode = params.one_of("mode", RESOURCE_MODES);
    // `varchar(60)` in Laravel.
    let license = params.text("license", 60);
    let search = params.search();
    let filters = params.finish()?;

    let pagination = Pagination::new("resources", lang, &filters, limit);
    let after = match cursor {
        Some(c) => Some(cursor::time_id_key(&cursor::decode(
            &c,
            &pagination.fingerprint,
        )?)?),
        None => None,
    };
    let rows = resources::list(
        &state.db,
        lang.translation(),
        &ResourceFilters {
            category: category.as_deref(),
            doc_lang: doc_lang.as_deref(),
            mode,
            license: license.as_deref(),
            search: search.as_deref(),
        },
        after,
        pagination.fetch(),
    )
    .await?;
    let page = pagination.page(
        rows,
        |row| cursor::time_id(row.created_at, row.id),
        to_resource,
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

    let row = resources::find(&state.db, lang.translation(), &slug)
        .await?
        .ok_or_else(|| {
            public::not_found(
                format!("No published resource has the slug '{slug}'."),
                uri.path(),
            )
        })?;
    public::json(
        &headers,
        lang,
        &Data {
            data: to_resource(row),
        },
    )
}

fn to_resource(row: ResourceRow) -> Resource {
    let (title, is_tr) = translated(row.title, row.tr_title);
    let description = match row.description {
        Some(original) => Some(translated(original, row.tr_description).0),
        None => row.tr_description.filter(|d| !d.is_empty()),
    };
    let category = match row.category {
        Some(original) => Some(translated(original, row.tr_category).0),
        None => row.tr_category.filter(|c| !c.is_empty()),
    };
    let license = row.license_key.map(|key| {
        let name = match (key.as_str(), row.license_custom, row.license_name) {
            (CUSTOM_LICENSE, Some(custom), _) if !custom.trim().is_empty() => custom,
            (_, _, Some(name)) => name,
            // A key the copied licence list does not know: show the key.
            _ => key.clone(),
        };
        License {
            key,
            name,
            url: row.license_url,
        }
    });
    let hosted = row.mode == "host";
    Resource {
        id: row.id,
        web_url: links::resource(&row.slug),
        download_url: hosted.then(|| links::resource_download(&row.slug)),
        slug: row.slug,
        lang: served(is_tr),
        title,
        description,
        category,
        doc_lang: row.doc_lang,
        mode: row.mode,
        attribution: row.attribution,
        license,
        source_url: row.source_url,
        size_bytes: if hosted { row.size_bytes } else { None },
        created_at: timestamp(row.created_at),
        updated_at: timestamp(row.updated_at.unwrap_or(row.created_at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: Option<&str>, custom: Option<&str>, name: Option<&str>) -> ResourceRow {
        ResourceRow {
            id: 1,
            slug: "x".into(),
            title: "X".into(),
            description: None,
            category: None,
            doc_lang: None,
            mode: "link".into(),
            attribution: None,
            license_key: key.map(Into::into),
            license_custom: custom.map(Into::into),
            license_name: name.map(Into::into),
            license_url: None,
            source_url: Some("https://example.org/x.pdf".into()),
            size_bytes: Some(10),
            created_at: chrono::DateTime::UNIX_EPOCH,
            updated_at: None,
            tr_title: None,
            tr_description: None,
            tr_category: None,
        }
    }

    #[test]
    fn custom_licences_show_the_uploader_text() {
        let r = to_resource(row(
            Some("autre"),
            Some("Licence Art Libre 1.3"),
            Some("Autre (à préciser)"),
        ));
        assert_eq!(
            r.license.map(|l| l.name).as_deref(),
            Some("Licence Art Libre 1.3")
        );
        let r = to_resource(row(Some("autre"), None, Some("Autre (à préciser)")));
        assert_eq!(
            r.license.map(|l| l.name).as_deref(),
            Some("Autre (à préciser)")
        );
        let r = to_resource(row(Some("unknown-key"), None, None));
        assert_eq!(r.license.map(|l| l.name).as_deref(), Some("unknown-key"));
        assert!(to_resource(row(None, None, None)).license.is_none());
    }

    #[test]
    fn links_have_no_download_or_size() {
        let r = to_resource(row(None, None, None));
        assert_eq!(r.download_url, None);
        assert_eq!(r.size_bytes, None);
        assert_eq!(r.source_url.as_deref(), Some("https://example.org/x.pdf"));
    }
}
