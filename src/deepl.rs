//! DeepL's `POST /v2/translate`, which translates forum posts (Laravel's
//! `TraducteurForum`).
//!
//! One request per post, two attempts at most, 250 ms apart. Each attempt
//! gets a few seconds, so both fit in the request timeout.
//!
//! [`Deepl`] translates the content for `pulsars-api content translate`
//! (Laravel's `TraducteurContenu` and `MarkdownProtege`), with the same
//! configuration and language map. Before a Markdown text goes to DeepL,
//! what a translator must not touch is put aside behind ASCII markers: code
//! blocks and inline code, images, and formulas in their four notations. A
//! fenced block keeps its prose annotations in parentheses: those are
//! translated with the text, in the same request, and put back in the block.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::DeeplConfig;

/// One attempt, sending included.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(4);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const ATTEMPTS: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_millis(250);
/// Largest answer read: two posts of 10 000 characters, translated.
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translated {
    pub text: String,
    /// What DeepL detected, lower case (`en`).
    pub source_lang: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No answer, or one that is not a success, after every attempt.
    #[error("DeepL did not answer properly: {0}")]
    Unavailable(String),
    /// A success whose body is not the expected list of translations.
    #[error("DeepL's answer is invalid: {0}")]
    Invalid(String),
}

#[derive(Serialize)]
struct Request<'a> {
    text: &'a [&'a str],
    target_lang: &'a str,
    preserve_formatting: bool,
}

#[derive(Deserialize)]
struct Response {
    translations: Vec<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    text: String,
    #[serde(default)]
    detected_source_language: Option<String>,
}

fn client() -> Result<&'static reqwest::Client, Error> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = crate::auth::jwks::http_client(false, ATTEMPT_TIMEOUT, CONNECT_TIMEOUT)
        .map_err(Error::Unavailable)?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Translates `texts` into DeepL's `target_lang`, one translation per text,
/// in order.
pub async fn translate(
    config: &DeeplConfig,
    texts: &[&str],
    target_lang: &str,
) -> Result<Vec<Translated>, Error> {
    let body = serde_json::to_vec(&Request {
        text: texts,
        target_lang,
        preserve_formatting: true,
    })
    .map_err(|e| Error::Invalid(e.to_string()))?;
    let url = format!("{}/v2/translate", config.url);
    let client = client()?;

    let mut last_error = String::new();
    for attempt in 1..=ATTEMPTS {
        if attempt > 1 {
            tokio::time::sleep(RETRY_DELAY).await;
        }
        let sent = client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .header(
                reqwest::header::AUTHORIZATION,
                format!("DeepL-Auth-Key {}", config.key.expose()),
            )
            .body(body.clone())
            .send()
            .await;
        let response = match sent {
            Ok(response) if response.status().is_success() => response,
            Ok(response) => {
                last_error = format!("status {}", response.status().as_u16());
                continue;
            }
            Err(error) => {
                // Never the URL's query or headers: only the kind of failure.
                last_error = if error.is_timeout() {
                    "timeout".to_owned()
                } else if error.is_connect() {
                    "connection failed".to_owned()
                } else {
                    "request failed".to_owned()
                };
                continue;
            }
        };
        let bytes = response
            .bytes()
            .await
            .map_err(|_| Error::Unavailable("body not received".to_owned()))?;
        return parse(&bytes, texts.len());
    }
    Err(Error::Unavailable(last_error))
}

fn parse(bytes: &[u8], expected: usize) -> Result<Vec<Translated>, Error> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(Error::Invalid("body too large".to_owned()));
    }
    let response: Response =
        serde_json::from_slice(bytes).map_err(|e| Error::Invalid(e.to_string()))?;
    if response.translations.len() != expected {
        return Err(Error::Invalid(format!(
            "{} translations for {expected} texts",
            response.translations.len()
        )));
    }
    Ok(response
        .translations
        .into_iter()
        .map(|t| Translated {
            text: t.text,
            source_lang: t
                .detected_source_language
                .map(|l| l.to_lowercase())
                .filter(|l| !l.is_empty()),
        })
        .collect())
}

// --- Content ----------------------------------------------------------------

/// Attempts of one content request: DeepL's free plan answers 429 when
/// calls come too fast.
const CONTENT_ATTEMPTS: u32 = 5;

/// Translates the content (courses, lessons, exercises, exam parts) for
/// `pulsars-api content translate`: longer requests, more patient retries,
/// French as the source as Laravel's `TraducteurContenu` sent it.
pub struct Deepl {
    config: DeeplConfig,
    http: reqwest::Client,
    /// Waited `attempt × retry_delay` between attempts (2 s in use).
    pub retry_delay: Duration,
}

impl Deepl {
    pub fn new(config: DeeplConfig) -> Result<Self, String> {
        let timeout = Duration::from_secs(60);
        Ok(Self {
            http: crate::auth::jwks::http_client(false, timeout, Duration::from_secs(10))?,
            config,
            retry_delay: Duration::from_secs(2),
        })
    }

    /// Translates `texts` from French into `lang`, in one request.
    pub async fn translate_texts(
        &self,
        texts: &[String],
        lang: &str,
    ) -> Result<Vec<String>, String> {
        let target = self
            .config
            .target(lang)
            .ok_or_else(|| format!("no DeepL language for `{lang}` (DEEPL_LANGUAGES)"))?;
        let body = json!({
            "text": texts,
            "target_lang": target,
            "source_lang": "FR",
            "preserve_formatting": true,
        })
        .to_string();
        let url = format!("{}/v2/translate", self.config.url);
        let mut attempt = 0;
        let (status, bytes) = loop {
            attempt += 1;
            let sent = self
                .http
                .post(&url)
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("DeepL-Auth-Key {}", self.config.key.expose()),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT, "application/json")
                .body(body.clone())
                .send()
                .await;
            let retry = match &sent {
                Err(_) => true,
                Ok(r) => r.status().as_u16() == 429 || r.status().is_server_error(),
            };
            if retry && attempt < CONTENT_ATTEMPTS {
                tokio::time::sleep(self.retry_delay * attempt).await;
                continue;
            }
            let response = sent.map_err(|e| format!("translation unavailable: {e}"))?;
            let status = response.status();
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
            break (status, bytes);
        };
        if !status.is_success() {
            return Err(format!(
                "translation unavailable (HTTP {}): {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes)
            ));
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "invalid answer from the translation service".to_owned())?;
        let translations: Vec<String> = value
            .get("translations")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|t| t.get("text")?.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if translations.len() != texts.len() {
            return Err("invalid answer from the translation service".to_owned());
        }
        Ok(translations)
    }

    /// Translates a Markdown text, its code, images and formulas untouched.
    pub async fn translate_markdown(&self, markdown: &str, lang: &str) -> Result<String, String> {
        let protected = protect(markdown);
        // The body and the annotations of the code blocks go together: one
        // round trip per field.
        let mut texts = vec![protected.text.clone()];
        texts.extend(protected.annotations.iter().cloned());
        let mut translated = self.translate_texts(&texts, lang).await?;
        let body = translated.remove(0);
        if !markers_intact(&body, protected.pieces.len()) {
            // A code block lost or duplicated: better the original than a
            // course without its examples.
            return Err("the translation altered the code blocks".to_owned());
        }
        let annotations: Vec<(String, String)> =
            protected.annotations.into_iter().zip(translated).collect();
        Ok(restore(&body, &protected.pieces, &annotations))
    }
}

/// A Markdown text with its protected pieces replaced by markers.
#[derive(Debug, PartialEq, Eq)]
pub struct Protected {
    pub text: String,
    pub pieces: Vec<String>,
    /// Prose annotations found in fenced blocks, each once.
    pub annotations: Vec<String>,
}

fn marker(index: usize) -> String {
    format!("ZQX{index}XQZ")
}

/// A pattern of `MarkdownProtege::MOTIFS`: the byte length of the match
/// starting at `i`, if any.
type Matcher = fn(&str, usize) -> Option<usize>;

/// ```` ```…``` ````, across lines.
fn fenced(s: &str, i: usize) -> Option<usize> {
    let rest = s.get(i..)?.strip_prefix("```")?;
    rest.find("```").map(|end| 3 + end + 3)
}

/// `` `…` `` on one line, not empty.
fn inline_code(s: &str, i: usize) -> Option<usize> {
    let rest = s.get(i..)?.strip_prefix('`')?;
    let end = rest.find(['`', '\n'])?;
    (end > 0 && rest[end..].starts_with('`')).then_some(1 + end + 1)
}

/// `![alt](url)`.
fn image(s: &str, i: usize) -> Option<usize> {
    let rest = s.get(i..)?.strip_prefix("![")?;
    let close = rest.find(']')?;
    let after = rest[close + 1..].strip_prefix('(')?;
    let end = after.find(')')?;
    Some(2 + close + 2 + end + 1)
}

/// `open…close` with at least one character inside, the shortest one.
fn delimited(s: &str, i: usize, open: &str, close: &str) -> Option<usize> {
    let rest = s.get(i..)?.strip_prefix(open)?;
    let from = rest.chars().next()?.len_utf8();
    let end = from + rest.get(from..)?.find(close)?;
    Some(open.len() + end + close.len())
}

fn display_dollars(s: &str, i: usize) -> Option<usize> {
    delimited(s, i, "$$", "$$")
}

fn display_brackets(s: &str, i: usize) -> Option<usize> {
    delimited(s, i, "\\[", "\\]")
}

fn inline_parens(s: &str, i: usize) -> Option<usize> {
    delimited(s, i, "\\(", "\\)")
}

/// `$…$`: no space or `$` after the opening one, no space before the
/// closing one, no digit after it (`\$(?![\s$])[^$]*?(?<!\s)\$(?!\d)`).
fn inline_dollars(s: &str, i: usize) -> Option<usize> {
    let rest = s.get(i..)?.strip_prefix('$')?;
    let first = rest.chars().next()?;
    if first.is_whitespace() || first == '$' {
        return None;
    }
    let end = rest.find('$')?;
    let last = rest[..end].chars().next_back()?;
    if last.is_whitespace() {
        return None;
    }
    if rest[end + 1..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(1 + end + 1)
}

/// The patterns, in Laravel's order: `$$` before `$`.
const PATTERNS: &[Matcher] = &[
    fenced,
    inline_code,
    image,
    display_dollars,
    display_brackets,
    inline_dollars,
    inline_parens,
];

/// Replaces every match of `pattern`, left to right, by a marker.
fn replace_all(text: &str, pattern: Matcher, pieces: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(len) = pattern(text, i) {
            pieces.push(text[i..i + len].to_owned());
            out.push_str(&marker(pieces.len() - 1));
            i += len;
        } else {
            let c = text[i..].chars().next().unwrap_or_default();
            out.push(c);
            i += c.len_utf8().max(1);
        }
    }
    out
}

/// Parentheses with no parenthesis inside and a word of three letters or
/// more: prose, not an expression like `(2 + 3)`. Their inner text and
/// byte range.
fn annotations_in(block: &str) -> Vec<(usize, usize, String)> {
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(open) = block[i..].find('(').map(|o| i + o) {
        let inner_start = open + 1;
        let Some(next) = block[inner_start..]
            .find(['(', ')'])
            .map(|n| inner_start + n)
        else {
            break;
        };
        if block[next..].starts_with(')') {
            let inner = &block[inner_start..next];
            let mut run = 0;
            let wordy = inner.chars().any(|c| {
                run = if c.is_alphabetic() { run + 1 } else { 0 };
                run >= 3
            });
            if wordy {
                found.push((open, next + 1, inner.to_owned()));
                i = next + 1;
                continue;
            }
        }
        i = next;
    }
    found
}

pub fn protect(markdown: &str) -> Protected {
    let mut pieces = Vec::new();
    let mut text = markdown.to_owned();
    for pattern in PATTERNS {
        text = replace_all(&text, *pattern, &mut pieces);
    }
    let mut annotations: Vec<String> = Vec::new();
    for piece in pieces.iter().filter(|p| p.starts_with("```")) {
        for (_, _, inner) in annotations_in(piece) {
            if !annotations.contains(&inner) {
                annotations.push(inner);
            }
        }
    }
    Protected {
        text,
        pieces,
        annotations,
    }
}

/// Engines "improve" typography: in a calculation, a long dash is a mistake.
fn normalise(text: &str) -> String {
    text.replace(['–', '—', '−'], "-").replace('’', "'")
}

/// Puts the pieces back, with the translated annotations in the blocks.
pub fn restore(text: &str, pieces: &[String], annotations: &[(String, String)]) -> String {
    let mut out = text.to_owned();
    for (i, piece) in pieces.iter().enumerate() {
        let mut piece = piece.clone();
        if !annotations.is_empty() && piece.starts_with("```") {
            let mut rebuilt = String::with_capacity(piece.len());
            let mut last = 0;
            for (start, end, inner) in annotations_in(&piece) {
                rebuilt.push_str(&piece[last..start]);
                match annotations.iter().find(|(original, _)| *original == inner) {
                    Some((_, translated)) => {
                        rebuilt.push('(');
                        rebuilt.push_str(&normalise(translated));
                        rebuilt.push(')');
                    }
                    None => rebuilt.push_str(&piece[start..end]),
                }
                last = end;
            }
            rebuilt.push_str(&piece[last..]);
            piece = rebuilt;
        }
        out = out.replace(&marker(i), &piece);
    }
    out
}

/// Whether every marker is still there, exactly once.
pub fn markers_intact(text: &str, count: usize) -> bool {
    (0..count).all(|i| text.matches(&marker(i)).count() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_one_translation_per_text() {
        let body = br#"{"translations":[
            {"detected_source_language":"EN","text":"Titre"},
            {"detected_source_language":"","text":"Corps"}]}"#;
        let got = parse(body, 2).unwrap();
        assert_eq!(got[0].text, "Titre");
        assert_eq!(got[0].source_lang.as_deref(), Some("en"));
        assert_eq!(got[1].source_lang, None);
        assert!(matches!(parse(body, 1), Err(Error::Invalid(_))));
        assert!(matches!(parse(b"{}", 1), Err(Error::Invalid(_))));
        assert!(matches!(parse(b"<html>", 1), Err(Error::Invalid(_))));
    }

    #[test]
    fn the_request_is_what_laravel_sent() {
        let json = serde_json::to_value(Request {
            text: &["a", "b"],
            target_lang: "EN-GB",
            preserve_formatting: true,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"text": ["a", "b"], "target_lang": "EN-GB", "preserve_formatting": true})
        );
    }

    #[test]
    fn code_images_and_formulas_are_put_aside() {
        let p = protect(
            "Calcule $x^2$ puis `code` et ![fig](a.png).\n$$\\frac{a}{b}$$ et \\(y\\) et \\[z\\].\n```\n12 : 3 x 2 = 8   (et non 12 : 6 = 2)\n(2 + 3)\n```\nFin.",
        );
        assert_eq!(
            p.pieces,
            [
                "```\n12 : 3 x 2 = 8   (et non 12 : 6 = 2)\n(2 + 3)\n```",
                "`code`",
                "![fig](a.png)",
                "$$\\frac{a}{b}$$",
                "\\[z\\]",
                "$x^2$",
                "\\(y\\)",
            ]
        );
        assert_eq!(
            p.text,
            "Calcule ZQX5XQZ puis ZQX1XQZ et ZQX2XQZ.\nZQX3XQZ et ZQX6XQZ et ZQX4XQZ.\nZQX0XQZ\nFin."
        );
        assert_eq!(p.annotations, ["et non 12 : 6 = 2"]);
        let back = restore(
            &p.text,
            &p.pieces,
            &[("et non 12 : 6 = 2".to_owned(), "not 12 ÷ 6 – 2".to_owned())],
        );
        assert!(back.contains("(not 12 ÷ 6 - 2)\n(2 + 3)"));
        assert!(back.starts_with("Calcule $x^2$ puis `code`"));
        assert!(markers_intact(&p.text, p.pieces.len()));
        assert!(!markers_intact("ZQX0XQZ ZQX0XQZ", 1));
    }

    #[test]
    fn prices_are_not_formulas() {
        // `$5 et $6`: a digit follows the closing dollar.
        assert!(protect("de 5$ à 6$7").pieces.is_empty());
        assert!(protect("un $ seul et $ autre").pieces.is_empty());
        assert_eq!(protect("$a$ b").pieces, ["$a$"]);
        assert!(protect("``").pieces.is_empty());
    }
}
