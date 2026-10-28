//! `GET /v1/exams`, `/v1/exams/facets` and `/v1/exams/{slug}` (`openapi.yaml`,
//! tag `exams`).
//!
//! Statements only, never the model answer or the correction PDF. Exam
//! titles and descriptions are not translatable in Laravel: an exam's `lang`
//! is always `fr`; only its parts follow `?lang`.

use axum::extract::{OriginalUri, Path, RawQuery, State};
use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::{Number, Value};

use super::courses::timestamp;
use super::cursor;
use super::params::{EXAM_SUBJECTS, EXAM_TYPES, Lang, Params};
use super::public::{self, Data, Pagination, PublicResult, served, translated};
use crate::db::exams::{self, ExamFilters, ExamRow, PartRow};
use crate::links;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ExamSummary {
    pub id: i64,
    pub slug: String,
    pub lang: &'static str,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub competition: Option<String>,
    pub subject: String,
    pub country: String,
    pub level: String,
    pub tracks: Vec<String>,
    pub session: Option<String>,
    pub duration_minutes: Option<i32>,
    pub total_points: Option<i32>,
    pub part_count: i32,
    pub pdf_url: Option<String>,
    pub official_url: Option<String>,
    pub source_url: Option<String>,
    pub published_at: String,
    pub description: Option<String>,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct ExamDetail {
    #[serde(flatten)]
    pub summary: ExamSummary,
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
pub struct Part {
    pub id: i64,
    pub position: i32,
    pub lang: &'static str,
    pub title: String,
    pub points: Option<Number>,
    pub statement_md: String,
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
    let kind = params.one_of("type", EXAM_TYPES);
    let subject = params.one_of("subject", EXAM_SUBJECTS);
    let level = params.text("level", 50);
    let session = params.text("session", 80);
    let country = params.upper_code("country", 2);
    let competition = params.text("competition", 255);
    let search = params.search();
    let page = params.page(cursor.is_some());
    let filters = params.finish()?;

    let pagination = Pagination::new("exams", lang, &filters, limit);
    let after = match cursor.filter(|_| page.is_none()) {
        Some(c) => Some(cursor::time_id_key(&cursor::decode(
            &c,
            &pagination.fingerprint,
        )?)?),
        None => None,
    };
    let exam_filters = ExamFilters {
        kind,
        subject,
        level: level.as_deref(),
        session: session.as_deref(),
        country: country.as_deref(),
        competition: competition.as_deref(),
        search: search.as_deref(),
    };
    let offset = page.map_or(0, |page| pagination.offset(page));
    let rows = exams::list(&state.db, &exam_filters, after, offset, pagination.fetch()).await?;
    let body = match page {
        Some(_) => {
            let total = match rows.first() {
                Some(row) => row.total,
                // Past the last page: count from the first one.
                None => exams::list(&state.db, &exam_filters, None, 0, 1)
                    .await?
                    .first()
                    .map_or(0, |row| row.total),
            };
            pagination.numbered(rows, total, to_summary)
        }
        None => pagination.page(
            rows,
            |row| cursor::time_id(row.published_at, row.id),
            to_summary,
        ),
    };
    public::json(&headers, lang, &body)
}

/// `{"data": {"levels": [...], "countries": [...]}}`: the values the list's
/// `level` and `country` filters can take.
#[derive(Debug, Serialize)]
pub struct Facets {
    pub levels: Vec<String>,
    pub countries: Vec<String>,
}

pub async fn facets(
    State(state): State<AppState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> PublicResult {
    let mut params = Params::parse(query.as_deref());
    let lang = params.lang();
    params.finish()?;
    let (levels, countries) = exams::facets(&state.db).await?;
    let body = Data {
        data: Facets { levels, countries },
    };
    public::json(&headers, lang, &body)
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

    let exam = exams::find(&state.db, &slug).await?.ok_or_else(|| {
        public::not_found(
            format!("No published exam has the slug '{slug}'."),
            uri.path(),
        )
    })?;
    let parts = exams::parts(&state.db, lang.translation(), exam.id).await?;
    let body = Data {
        data: ExamDetail {
            summary: to_summary(exam),
            parts: parts.into_iter().map(|p| to_part(p, lang)).collect(),
        },
    };
    public::json(&headers, lang, &body)
}

fn to_summary(row: ExamRow) -> ExamSummary {
    let tracks = match row.tracks.0 {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|t| t.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    ExamSummary {
        id: row.id,
        pdf_url: row.has_pdf.then(|| links::exam_pdf(&row.slug)),
        web_url: links::exam(&row.slug),
        slug: row.slug,
        lang: "fr",
        title: row.title,
        kind: row.kind,
        competition: row.competition,
        subject: row.subject,
        country: row.country,
        level: row.level,
        tracks,
        session: row.session,
        duration_minutes: row.duration_minutes,
        total_points: row.total_points,
        part_count: row.part_count,
        official_url: row.official_url,
        source_url: row.source_url,
        published_at: timestamp(row.published_at),
        description: row.description,
    }
}

fn to_part(row: PartRow, lang: Lang) -> Part {
    let (title, is_tr) = translated(row.title, row.tr_title);
    Part {
        id: row.id,
        position: row.position,
        lang: served(lang, is_tr),
        title,
        points: row.points.and_then(points),
        statement_md: translated(row.statement_md, row.tr_statement_md).0,
    }
}

/// Whole points as an integer (`5`, not `5.0`), others as a decimal.
fn points(value: f64) -> Option<Number> {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        Some(Number::from(value as i64))
    } else {
        Number::from_f64(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_points_are_integers() {
        assert_eq!(points(5.0).map(|n| n.to_string()).as_deref(), Some("5"));
        assert_eq!(points(2.5).map(|n| n.to_string()).as_deref(), Some("2.5"));
        assert_eq!(points(f64::NAN), None);
    }
}
