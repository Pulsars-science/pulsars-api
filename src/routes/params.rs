//! Query parameters of the content endpoints (`openapi.yaml`, components
//! `parameters`), validated by hand so every mistake is reported at once in
//! a single `invalid-parameter` problem.
//!
//! Unknown parameters are ignored (`v1` only grows by addition). A known
//! parameter given twice, or given empty, is an error: the API never has to
//! guess which value was meant.

use std::collections::BTreeMap;

use crate::error::{ApiError, ParameterError};

/// Default and maximum page sizes (`docs/contract/README.md`, pagination).
pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 100;

/// Bounds of `q`.
const SEARCH_MIN_CHARS: usize = 2;
const SEARCH_MAX_CHARS: usize = 100;

/// Longest cursor accepted.
const CURSOR_MAX_BYTES: usize = 512;

/// Content language (`?lang=fr|en`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// The source language.
    #[default]
    Fr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
        }
    }

    /// The language to look up in the translation views; `None` for the
    /// source language, whose text is the original.
    pub fn translation(self) -> Option<&'static str> {
        match self {
            Self::Fr => None,
            Self::En => Some("en"),
        }
    }
}

pub const LANGS: &[&str] = &["fr", "en"];
pub const DIFFICULTIES: &[&str] = &["debutant", "intermediaire", "avance"];
pub const LEVELS: &[&str] = &["college", "lycee", "superieur"];
pub const EXERCISE_TYPES: &[&str] = &["qcm", "reponse_courte", "reponse_libre"];
pub const EXAM_TYPES: &[&str] = &["brevet", "bac", "blanc", "concours"];
pub const EXAM_SUBJECTS: &[&str] = &[
    "mathematiques",
    "physique",
    "chimie",
    "informatique",
    "sciences-industrielles",
    "langues",
    "redaction",
];
pub const RESOURCE_MODES: &[&str] = &["host", "link"];

/// The decoded query string, plus the errors found so far.
#[derive(Debug, Default)]
pub struct Params {
    values: BTreeMap<String, String>,
    repeated: Vec<String>,
    errors: Vec<ParameterError>,
    /// Filters read so far, for the cursor fingerprint.
    filters: BTreeMap<&'static str, String>,
}

impl Params {
    pub fn parse(raw: Option<&str>) -> Self {
        let mut params = Self::default();
        for (name, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
            if params.values.contains_key(name.as_ref()) {
                params.repeated.push(name.into_owned());
            } else {
                params.values.insert(name.into_owned(), value.into_owned());
            }
        }
        params
    }

    fn error(&mut self, parameter: &str, detail: impl Into<String>) {
        self.errors.push(ParameterError {
            parameter: parameter.to_owned(),
            detail: detail.into(),
        });
    }

    /// The raw value of `name`, if given. Repeated and empty values are
    /// recorded as errors and read as absent.
    fn raw(&mut self, name: &str) -> Option<String> {
        if self.repeated.iter().any(|n| n == name) {
            self.error(name, "must not be repeated");
            return None;
        }
        let value = self.values.get(name)?.clone();
        if value.is_empty() {
            self.error(name, "must not be empty");
            return None;
        }
        Some(value)
    }

    fn remember(&mut self, name: &'static str, value: &str) {
        self.filters.insert(name, value.to_owned());
    }

    /// `lang`, `fr` when absent. Always part of the fingerprint, see
    /// [`Filters::canonical`].
    pub fn lang(&mut self) -> Lang {
        match self.pick("lang", LANGS) {
            Some("en") => Lang::En,
            _ => Lang::Fr,
        }
    }

    /// `limit`, between 1 and 100, 20 when absent.
    pub fn limit(&mut self) -> u32 {
        self.limit_or(DEFAULT_LIMIT)
    }

    /// `limit`, between 1 and 100, `default` when absent.
    pub fn limit_or(&mut self, default: u32) -> u32 {
        let Some(raw) = self.raw("limit") else {
            return default;
        };
        match raw.parse::<u32>() {
            Ok(n) if (1..=MAX_LIMIT).contains(&n) => n,
            _ => {
                self.error("limit", format!("must be between 1 and {MAX_LIMIT}"));
                default
            }
        }
    }

    /// `cursor`, still encoded. Never part of the fingerprint.
    pub fn cursor(&mut self) -> Option<String> {
        let raw = self.raw("cursor")?;
        if raw.len() > CURSOR_MAX_BYTES {
            self.error(
                "cursor",
                format!("must be at most {CURSOR_MAX_BYTES} characters"),
            );
            return None;
        }
        Some(raw)
    }

    /// A free string of at most `max_chars` characters.
    pub fn text(&mut self, name: &'static str, max_chars: usize) -> Option<String> {
        let raw = self.raw(name)?;
        if raw.chars().count() > max_chars {
            self.error(name, format!("must be at most {max_chars} characters"));
            return None;
        }
        self.remember(name, &raw);
        Some(raw)
    }

    /// One of `allowed`.
    pub fn one_of(&mut self, name: &'static str, allowed: &[&'static str]) -> Option<&'static str> {
        let value = self.pick(name, allowed)?;
        self.remember(name, value);
        Some(value)
    }

    fn pick(&mut self, name: &'static str, allowed: &[&'static str]) -> Option<&'static str> {
        let raw = self.raw(name)?;
        match allowed.iter().find(|a| **a == raw) {
            Some(value) => Some(value),
            None => {
                self.error(name, format!("must be one of: {}", allowed.join(", ")));
                None
            }
        }
    }

    /// A string matching `^[A-Z]{len}$` (country codes).
    pub fn upper_code(&mut self, name: &'static str, len: usize) -> Option<String> {
        let raw = self.raw(name)?;
        if raw.len() != len || !raw.bytes().all(|b| b.is_ascii_uppercase()) {
            self.error(name, format!("must be {len} uppercase letters"));
            return None;
        }
        self.remember(name, &raw);
        Some(raw)
    }

    /// `q`, as a case-insensitive `LIKE` pattern matching any substring.
    pub fn search(&mut self) -> Option<String> {
        let raw = self.raw("q")?;
        let chars = raw.chars().count();
        if !(SEARCH_MIN_CHARS..=SEARCH_MAX_CHARS).contains(&chars) {
            self.error(
                "q",
                format!("must be between {SEARCH_MIN_CHARS} and {SEARCH_MAX_CHARS} characters"),
            );
            return None;
        }
        self.remember("q", &raw);
        Some(like_pattern(&raw))
    }

    /// Records that `name` needs `other`.
    pub fn require(&mut self, name: &str, given: bool, other: &str, other_given: bool) {
        if given && !other_given {
            self.error(name, format!("requires `{other}`"));
        }
    }

    /// Fails with every error found, in the order the parameters were read.
    pub fn finish(self) -> Result<Filters, ApiError> {
        if self.errors.is_empty() {
            Ok(Filters(self.filters))
        } else {
            Err(ApiError::InvalidParameter(self.errors))
        }
    }
}

/// The filters a list was requested with, for the cursor fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filters(BTreeMap<&'static str, String>);

impl Filters {
    /// A canonical text of the endpoint, the language and the filters.
    pub fn canonical(&self, endpoint: &str, lang: Lang) -> String {
        let mut out = format!("{endpoint}\u{0}lang={}", lang.code());
        for (name, value) in &self.0 {
            out.push('\u{0}');
            out.push_str(name);
            out.push('=');
            out.push_str(value);
        }
        out
    }
}

/// `%value%`, with the `LIKE` metacharacters of `value` escaped (backslash is
/// PostgreSQL's default `LIKE` escape character).
fn like_pattern(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('%');
    for c in value.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errors(result: Result<Filters, ApiError>) -> Vec<(String, String)> {
        match result {
            Err(ApiError::InvalidParameter(errors)) => errors
                .into_iter()
                .map(|e| (e.parameter, e.detail))
                .collect(),
            other => panic!("expected invalid-parameter, got {other:?}"),
        }
    }

    #[test]
    fn defaults_when_absent() {
        let mut params = Params::parse(None);
        assert_eq!(params.lang(), Lang::Fr);
        assert_eq!(params.limit(), 20);
        assert_eq!(params.cursor(), None);
        assert_eq!(params.search(), None);
        assert!(params.finish().is_ok());
    }

    #[test]
    fn reads_valid_values() {
        let mut params = Params::parse(Some(
            "lang=en&limit=100&difficulty=debutant&q=f%C3%A9e&unknown=1",
        ));
        assert_eq!(params.lang(), Lang::En);
        assert_eq!(params.limit(), 100);
        assert_eq!(params.one_of("difficulty", DIFFICULTIES), Some("debutant"));
        assert_eq!(params.search().as_deref(), Some("%fée%"));
        assert!(params.finish().is_ok());
    }

    #[test]
    fn reports_every_error_at_once() {
        let mut params = Params::parse(Some("lang=de&limit=0&difficulty=easy&q=x&country=fr&tag="));
        params.lang();
        params.limit();
        params.one_of("difficulty", DIFFICULTIES);
        params.search();
        params.upper_code("country", 2);
        params.text("tag", 50);
        let errors = errors(params.finish());
        let names: Vec<&str> = errors.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            names,
            vec!["lang", "limit", "difficulty", "q", "country", "tag"]
        );
        assert_eq!(errors[1].1, "must be between 1 and 100");
    }

    #[test]
    fn limit_must_be_a_small_integer() {
        for bad in ["101", "-1", "1.5", "abc", "99999999999"] {
            let mut params = Params::parse(Some(&format!("limit={bad}")));
            params.limit();
            assert!(params.finish().is_err(), "{bad}");
        }
    }

    #[test]
    fn repeated_parameters_are_refused() {
        let mut params = Params::parse(Some("lang=fr&lang=en"));
        params.lang();
        assert_eq!(
            errors(params.finish()),
            vec![("lang".to_owned(), "must not be repeated".to_owned())]
        );
    }

    #[test]
    fn length_limits_count_characters() {
        let mut params = Params::parse(Some(&format!("tag={}", "é".repeat(50))));
        assert!(params.text("tag", 50).is_some());
        let mut params = Params::parse(Some(&format!("tag={}", "é".repeat(51))));
        assert!(params.text("tag", 50).is_none());
        let mut params = Params::parse(Some(&format!("q={}", "a".repeat(101))));
        assert!(params.search().is_none());
    }

    #[test]
    fn like_metacharacters_are_escaped() {
        assert_eq!(like_pattern("50%_a\\b"), "%50\\%\\_a\\\\b%");
    }

    #[test]
    fn the_fingerprint_text_covers_language_and_filters() {
        let mut params = Params::parse(Some("subject=chimie&limit=5&cursor=abc"));
        params.text("subject", 100);
        params.limit();
        params.cursor();
        let filters = params.finish().expect("valid");
        assert_eq!(
            filters.canonical("courses", Lang::En),
            "courses\u{0}lang=en\u{0}subject=chimie"
        );
    }
}
