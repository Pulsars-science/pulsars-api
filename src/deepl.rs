//! DeepL's `POST /v2/translate`, which translates forum posts (Laravel's
//! `TraducteurForum`).
//!
//! One request per post, two attempts at most, 250 ms apart. Each attempt
//! gets a few seconds, so both fit in the request timeout.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

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
}
