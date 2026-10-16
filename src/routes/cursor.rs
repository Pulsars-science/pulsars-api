//! Keyset pagination cursors (`docs/contract/README.md`, pagination).
//!
//! A cursor is base64url (no padding) of `{"k": [...], "f": "..."}`: `k` is
//! the sort key of the last row returned, ending with its `id`; `f` is a
//! fingerprint of the endpoint, language and filters it was issued for. A
//! cursor reused with other filters, or for another endpoint, is refused
//! with `400 invalid-cursor`. Cursors are not signed: any key a client forges
//! only moves the starting point of a public list.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::ApiError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    k: Vec<Value>,
    f: String,
}

/// Fingerprint of a canonical filter text (see `params::Filters`).
pub fn fingerprint(canonical: &str) -> String {
    let digest = Sha256::digest(canonical.as_bytes());
    digest[..6].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn encode(key: Vec<Value>, fingerprint: &str) -> String {
    let raw = Raw {
        k: key,
        f: fingerprint.to_owned(),
    };
    // Serializing a `Vec<Value>` and a string cannot fail.
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&raw).unwrap_or_default())
}

/// The sort key of `cursor`, if it was issued for `fingerprint`.
pub fn decode(cursor: &str, fingerprint: &str) -> Result<Vec<Value>, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ApiError::InvalidCursor)?;
    let raw: Raw = serde_json::from_slice(&bytes).map_err(|_| ApiError::InvalidCursor)?;
    if raw.f != fingerprint {
        return Err(ApiError::InvalidCursor);
    }
    Ok(raw.k)
}

/// A `[id]` key.
pub fn id_key(key: &[Value]) -> Result<i64, ApiError> {
    match key {
        [id] => id.as_i64().ok_or(ApiError::InvalidCursor),
        _ => Err(ApiError::InvalidCursor),
    }
}

/// A `[text, id]` key.
pub fn text_id_key(key: &[Value]) -> Result<(String, i64), ApiError> {
    match key {
        [Value::String(text), id] => {
            Ok((text.clone(), id.as_i64().ok_or(ApiError::InvalidCursor)?))
        }
        _ => Err(ApiError::InvalidCursor),
    }
}

/// A `[microseconds since the epoch, id]` key.
pub fn time_id_key(key: &[Value]) -> Result<(DateTime<Utc>, i64), ApiError> {
    match key {
        [micros, id] => {
            let at = micros
                .as_i64()
                .and_then(DateTime::from_timestamp_micros)
                .ok_or(ApiError::InvalidCursor)?;
            Ok((at, id.as_i64().ok_or(ApiError::InvalidCursor)?))
        }
        _ => Err(ApiError::InvalidCursor),
    }
}

/// The key of a row sorted by time then id.
pub fn time_id(at: DateTime<Utc>, id: i64) -> Vec<Value> {
    vec![Value::from(at.timestamp_micros()), Value::from(id)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_with_the_same_filters() {
        let f = fingerprint("courses\u{0}lang=fr");
        let cursor = encode(vec![json!("Les bases du calcul"), json!(1)], &f);
        assert!(!cursor.contains('='), "no padding");
        let key = decode(&cursor, &f).expect("valid");
        assert_eq!(
            text_id_key(&key).expect("shape"),
            ("Les bases du calcul".to_owned(), 1)
        );
    }

    #[test]
    fn refuses_other_filters() {
        let cursor = encode(vec![json!(1)], &fingerprint("exercises\u{0}lang=fr"));
        assert!(matches!(
            decode(&cursor, &fingerprint("exercises\u{0}lang=en")),
            Err(ApiError::InvalidCursor)
        ));
    }

    #[test]
    fn refuses_garbage() {
        let f = fingerprint("x");
        for bad in [
            "!!!",
            "e30",                          // {}
            &URL_SAFE_NO_PAD.encode("[1]"), // not an object
            &URL_SAFE_NO_PAD.encode(r#"{"k":1,"f":"x"}"#),
        ] {
            assert!(decode(bad, &f).is_err(), "{bad}");
        }
        assert!(id_key(&[json!("1")]).is_err());
        assert!(id_key(&[json!(1), json!(2)]).is_err());
        assert!(text_id_key(&[json!(1), json!(1)]).is_err());
        assert!(time_id_key(&[json!(i64::MAX), json!(1)]).is_err());
    }

    #[test]
    fn time_keys_keep_microseconds() {
        let at = DateTime::from_timestamp_micros(1_786_613_524_000_123).expect("valid");
        let key = time_id(at, 7);
        assert_eq!(time_id_key(&key).expect("shape"), (at, 7));
    }
}
