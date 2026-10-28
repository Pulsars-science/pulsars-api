//! Game token verification, exactly as `docs/contract/auth-jwt.md` §5 lists
//! it: same checks, same order, same `reason` values.
//!
//! Written by hand rather than with a generic JWT library: the contract
//! allows a single algorithm and key type, and the verifying key is never
//! chosen from what the token says about itself (`alg` is only compared to
//! `EdDSA`). Verification is split in two so the key lookup by `kid` stays
//! out of this module:
//!
//! 1. [`bearer`] and [`decode`]: header, structure, `alg`, `kid` (steps 1-4
//!    up to the lookup);
//! 2. [`Unverified::verify`]: signature, then claims (steps 5-10).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::{Map, Value};

/// Leeway on time claims, in seconds.
pub const LEEWAY_SECS: i64 = 30;

/// Tokens are under 600 bytes (contract §1). Anything much larger is not
/// worth decoding.
const MAX_TOKEN_BYTES: usize = 4096;

/// Why a token was refused: the `reason` of the 401 problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Missing,
    Malformed,
    BadAlgorithm,
    UnknownKid,
    BadSignature,
    BadIssuer,
    BadAudience,
    Expired,
    NotYetValid,
}

impl Reason {
    /// Stable value of the problem's `reason` field.
    pub fn code(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Malformed => "malformed",
            Self::BadAlgorithm => "bad_algorithm",
            Self::UnknownKid => "unknown_kid",
            Self::BadSignature => "bad_signature",
            Self::BadIssuer => "bad_issuer",
            Self::BadAudience => "bad_audience",
            Self::Expired => "expired",
            Self::NotYetValid => "not_yet_valid",
        }
    }

    /// The problem's `detail`. Never echoes anything from the token.
    pub fn detail(self) -> &'static str {
        match self {
            Self::Missing => "No bearer token was provided.",
            Self::Malformed => "The token is malformed.",
            Self::BadAlgorithm => "The token is not signed with EdDSA.",
            Self::UnknownKid => "The token was signed with an unknown key.",
            Self::BadSignature => "The token signature is invalid.",
            Self::BadIssuer => "The token was issued by an unexpected issuer.",
            Self::BadAudience => "The token is not meant for this API.",
            Self::Expired => "The token has expired.",
            Self::NotYetValid => "The token is not valid yet.",
        }
    }

    /// `error_description` of the `WWW-Authenticate` header (RFC 6750). Plain
    /// ASCII without quotes, as the header syntax requires.
    pub fn description(self) -> &'static str {
        match self {
            Self::Missing => "token missing",
            Self::Malformed => "token malformed",
            Self::BadAlgorithm => "unsupported algorithm",
            Self::UnknownKid => "unknown signing key",
            Self::BadSignature => "invalid signature",
            Self::BadIssuer => "invalid issuer",
            Self::BadAudience => "invalid audience",
            Self::Expired => "token expired",
            Self::NotYetValid => "token not yet valid",
        }
    }
}

/// What a valid token must carry.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    pub issuer: &'a str,
    pub audience: &'a str,
}

/// The claims of a verified token (contract §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    /// `site.users.id`, as a string.
    pub sub: String,
    /// Unique token id (UUIDv7).
    pub jti: String,
    /// Expiry, Unix seconds.
    pub exp: i64,
    /// The member's pseudo.
    pub name: Option<String>,
    /// `fr`, `en` or absent.
    pub locale: Option<String>,
    /// Permission roles only: `moderateur`, `admin`, `super-admin`.
    pub roles: Vec<String>,
}

/// Step 1: the token of an `Authorization: Bearer <token>` header.
pub fn bearer(header: Option<&[u8]>) -> Result<&str, Reason> {
    let header = header.ok_or(Reason::Missing)?;
    let header = std::str::from_utf8(header).map_err(|_| Reason::Malformed)?;
    let (scheme, token) = header.split_once(' ').ok_or(Reason::Malformed)?;
    // The scheme is case-insensitive (RFC 9110 §11.1); the token is not.
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(Reason::Malformed);
    }
    if !token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(Reason::Malformed);
    }
    Ok(token)
}

/// A token whose structure, `alg` and `kid` are valid, and whose signature
/// is not checked yet.
#[derive(Debug)]
pub struct Unverified<'a> {
    kid: String,
    /// `<header>.<claims>`, exactly as received: what was signed.
    signing_input: &'a [u8],
    signature: Vec<u8>,
    claims: Map<String, Value>,
}

/// Steps 2-4 (before the key lookup): structure, `alg`, presence of `kid`.
pub fn decode(token: &str) -> Result<Unverified<'_>, Reason> {
    if token.len() > MAX_TOKEN_BYTES {
        return Err(Reason::Malformed);
    }
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Reason::Malformed);
    };
    let header = json_object(header)?;
    let claims = json_object(claims)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| Reason::Malformed)?;
    // Extensions marked critical must be understood (RFC 7515 §4.1.11); this
    // verifier understands none.
    if header.contains_key("crit") {
        return Err(Reason::Malformed);
    }

    if header.get("alg").and_then(Value::as_str) != Some("EdDSA") {
        return Err(Reason::BadAlgorithm);
    }
    let kid = match header.get("kid").and_then(Value::as_str) {
        Some(kid) if !kid.is_empty() => kid.to_owned(),
        _ => return Err(Reason::UnknownKid),
    };

    let signing_input_len = token.len() - signature_len(token);
    Ok(Unverified {
        kid,
        signing_input: &token.as_bytes()[..signing_input_len],
        signature,
        claims,
    })
}

/// Length of `.<signature>` at the end of a three-part token.
fn signature_len(token: &str) -> usize {
    token.rfind('.').map_or(0, |dot| token.len() - dot)
}

fn json_object(part: &str) -> Result<Map<String, Value>, Reason> {
    let bytes = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| Reason::Malformed)?;
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(Reason::Malformed),
    }
}

impl Unverified<'_> {
    /// The `kid` of the key that signed the token.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// Steps 5-10, with the key of [`Self::kid`]. `now` is in Unix seconds.
    pub fn verify(
        self,
        key: &VerifyingKey,
        expected: Expected<'_>,
        now: i64,
    ) -> Result<Claims, Reason> {
        // 5. Signature. `verify_strict` also refuses small-order keys and
        // non-canonical signatures.
        let signature = Signature::from_slice(&self.signature).map_err(|_| Reason::BadSignature)?;
        key.verify_strict(self.signing_input, &signature)
            .map_err(|_| Reason::BadSignature)?;

        let claims = &self.claims;

        // 6. Issuer.
        if claims.get("iss").and_then(Value::as_str) != Some(expected.issuer) {
            return Err(Reason::BadIssuer);
        }

        // 7. Audience: a string, or an array containing it.
        let audience_ok = match claims.get("aud") {
            Some(Value::String(aud)) => aud == expected.audience,
            Some(Value::Array(auds)) => auds.iter().any(|a| a.as_str() == Some(expected.audience)),
            _ => false,
        };
        if !audience_ok {
            return Err(Reason::BadAudience);
        }

        // 8. Expiry. Without `exp` a token would never expire: refused.
        let exp = numeric_date(claims, "exp")?;
        if exp <= now - LEEWAY_SECS {
            return Err(Reason::Expired);
        }

        // 9. Not before, issued at. The API always sets both.
        let nbf = numeric_date(claims, "nbf")?;
        let iat = numeric_date(claims, "iat")?;
        if nbf > now + LEEWAY_SECS || iat > now + LEEWAY_SECS {
            return Err(Reason::NotYetValid);
        }

        // 10. Subject and token id.
        let sub = non_empty_string(claims, "sub")?;
        let jti = non_empty_string(claims, "jti")?;

        // Informative claims: absent is fine, a wrong type is not.
        let name = optional_string(claims, "name")?;
        let locale = optional_string(claims, "locale")?;
        let roles = match claims.get("roles") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|r| r.as_str().map(str::to_owned).ok_or(Reason::Malformed))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(Reason::Malformed),
        };

        Ok(Claims {
            sub,
            jti,
            exp,
            name,
            locale,
            roles,
        })
    }
}

/// A NumericDate (RFC 7519 §2): seconds, possibly fractional. Rounded down,
/// which errs towards "expired" and "not yet valid" by under a second.
fn numeric_date(claims: &Map<String, Value>, name: &str) -> Result<i64, Reason> {
    let value = claims.get(name).ok_or(Reason::Malformed)?;
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    match value.as_f64() {
        // Beyond ±2^53 seconds the value is meaningless anyway.
        Some(f) if f.is_finite() && f.abs() < 9.0e15 => Ok(f.floor() as i64),
        _ => Err(Reason::Malformed),
    }
}

fn non_empty_string(claims: &Map<String, Value>, name: &str) -> Result<String, Reason> {
    match claims.get(name).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => Ok(s.to_owned()),
        _ => Err(Reason::Malformed),
    }
}

fn optional_string(claims: &Map<String, Value>, name: &str) -> Result<Option<String>, Reason> {
    match claims.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(Reason::Malformed),
    }
}

/// Test helpers to mint tokens, shared with the integration tests.
#[cfg(any(test, feature = "test-util"))]
pub mod testing {
    use base64::Engine as _;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer as _, SigningKey};
    use serde_json::{Value, json};

    use crate::config::{API_AUDIENCE, API_ISSUER, TokenSigner};

    /// The `kid` of [`signer`].
    pub const KID: &str = "2026-10-ed25519-1";

    /// A fresh random signing key. The seed comes from the OS RNG through
    /// UUIDv4, which avoids a direct dependency on `rand`.
    pub fn signing_key() -> SigningKey {
        let mut seed = [0u8; 32];
        seed[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        seed[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        SigningKey::from_bytes(&seed)
    }

    /// A signing key for a test application, under [`KID`]: set it as
    /// `config.token_signer`, then mint tokens with [`token`] or [`sign`]
    /// and its `key`.
    pub fn signer() -> TokenSigner {
        TokenSigner {
            kid: KID.to_owned(),
            key: signing_key(),
            ttl_secs: 300,
        }
    }

    /// Valid claims for user `sub`, issued at `now`.
    pub fn claims(sub: &str, now: i64) -> Value {
        json!({
            "iss": API_ISSUER,
            "aud": API_AUDIENCE,
            "sub": sub,
            "iat": now,
            "nbf": now,
            "exp": now + 300,
            "jti": uuid::Uuid::now_v7().to_string(),
            "name": "jikuji",
            "locale": "fr",
            "roles": ["super-admin"],
        })
    }

    /// Signs `header` and `claims` as given, without any check.
    pub fn sign(key: &SigningKey, header: &Value, claims: &Value) -> String {
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = key.sign(input.as_bytes());
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }

    /// A valid token for `sub`, signed by `key` under `kid`.
    pub fn token(key: &SigningKey, kid: &str, sub: &str, now: i64) -> String {
        sign(
            key,
            &json!({"alg": "EdDSA", "typ": "JWT", "kid": kid}),
            &claims(sub, now),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{claims, sign, signing_key, token};
    use super::*;
    use ed25519_dalek::SigningKey;
    use serde_json::json;

    const KID: &str = "2026-10-ed25519-1";
    const NOW: i64 = 1_792_059_151;
    const EXPECTED: Expected<'static> = crate::auth::EXPECTED;

    fn header() -> Value {
        json!({"alg": "EdDSA", "typ": "JWT", "kid": KID})
    }

    /// Full verification with a single known key, like the extractor does.
    fn check(key: &SigningKey, token: &str, now: i64) -> Result<Claims, Reason> {
        let unverified = decode(token)?;
        if unverified.kid() != KID {
            return Err(Reason::UnknownKid);
        }
        unverified.verify(&key.verifying_key(), EXPECTED, now)
    }

    fn with_claims(key: &SigningKey, edit: impl FnOnce(&mut Map<String, Value>)) -> String {
        let mut c = claims("42", NOW);
        if let Value::Object(map) = &mut c {
            edit(map);
        }
        sign(key, &header(), &c)
    }

    #[test]
    fn accepts_a_valid_token() {
        let key = signing_key();
        let claims = check(&key, &token(&key, KID, "42", NOW), NOW).expect("valid");
        assert_eq!(claims.sub, "42");
        assert_eq!(claims.exp, NOW + 300);
        assert_eq!(claims.name.as_deref(), Some("jikuji"));
        assert_eq!(claims.locale.as_deref(), Some("fr"));
        assert_eq!(claims.roles, vec!["super-admin"]);
    }

    #[test]
    fn missing_header() {
        assert_eq!(bearer(None), Err(Reason::Missing));
    }

    #[test]
    fn malformed_header() {
        for bad in [
            &b"Basic dXNlcjpwYXNz"[..],
            b"Bearer",
            b"Bearer ",
            b"Bearer  abc",
            b"Bearer abc def",
            b"Bearer a\"b",
            b"Bearerabc",
            b"Bearer \xff",
        ] {
            assert_eq!(bearer(Some(bad)), Err(Reason::Malformed), "{bad:?}");
        }
        assert_eq!(bearer(Some(b"bearer a.b.c")), Ok("a.b.c"));
        assert_eq!(bearer(Some(b"Bearer a-b_c.d.e")), Ok("a-b_c.d.e"));
    }

    #[test]
    fn malformed_structure() {
        let key = signing_key();
        let good = token(&key, KID, "42", NOW);
        let (head, rest) = good.split_once('.').expect("three parts");
        for bad in [
            String::new(),
            "abc".to_owned(),
            "a.b".to_owned(),
            format!("{good}.extra"),
            format!("{head}.%%%.sig"),
            format!("!!.{rest}"),
            // Padding is not base64url without padding.
            format!("{head}=.{rest}"),
            // Valid base64url, but not a JSON object.
            format!("{}.{rest}", URL_SAFE_NO_PAD.encode("[1,2]")),
            format!("{head}.{}.sig", URL_SAFE_NO_PAD.encode("\"claims\"")),
            "a".repeat(MAX_TOKEN_BYTES + 1),
        ] {
            assert_eq!(
                decode(&bad).map(|_| ()),
                Err(Reason::Malformed),
                "{bad:.40}"
            );
        }
    }

    #[test]
    fn critical_extensions_are_refused() {
        let key = signing_key();
        let t = sign(
            &key,
            &json!({"alg": "EdDSA", "kid": KID, "crit": ["exp"]}),
            &claims("42", NOW),
        );
        assert_eq!(check(&key, &t, NOW), Err(Reason::Malformed));
    }

    #[test]
    fn bad_algorithm() {
        let key = signing_key();
        for alg in [
            json!("none"),
            json!("HS256"),
            json!("RS256"),
            json!("ES256"),
            json!("eddsa"),
            json!("Ed25519"),
            Value::Null,
        ] {
            let t = sign(&key, &json!({"alg": alg, "kid": KID}), &claims("42", NOW));
            assert_eq!(check(&key, &t, NOW), Err(Reason::BadAlgorithm), "{alg}");
        }
        let no_alg = sign(&key, &json!({"kid": KID}), &claims("42", NOW));
        assert_eq!(check(&key, &no_alg, NOW), Err(Reason::BadAlgorithm));
    }

    #[test]
    fn alg_none_without_signature_is_refused() {
        let key = signing_key();
        let t = format!(
            "{}.{}.",
            URL_SAFE_NO_PAD.encode(json!({"alg": "none", "kid": KID}).to_string()),
            URL_SAFE_NO_PAD.encode(claims("42", NOW).to_string())
        );
        assert_eq!(check(&key, &t, NOW), Err(Reason::BadAlgorithm));
    }

    #[test]
    fn unknown_kid() {
        let key = signing_key();
        assert_eq!(
            check(&key, &token(&key, "2020-01-ed25519-9", "42", NOW), NOW),
            Err(Reason::UnknownKid)
        );
        for header in [
            json!({"alg": "EdDSA"}),
            json!({"alg": "EdDSA", "kid": ""}),
            json!({"alg": "EdDSA", "kid": 1}),
        ] {
            let t = sign(&key, &header, &claims("42", NOW));
            assert_eq!(decode(&t).map(|_| ()), Err(Reason::UnknownKid), "{header}");
        }
    }

    #[test]
    fn bad_signature() {
        let key = signing_key();
        let other = signing_key();
        // Signed by another key under the same kid.
        assert_eq!(
            check(&key, &token(&other, KID, "42", NOW), NOW),
            Err(Reason::BadSignature)
        );

        // Claims changed after signing.
        let good = token(&key, KID, "42", NOW);
        let parts: Vec<&str> = good.split('.').collect();
        let forged = format!(
            "{}.{}.{}",
            parts[0],
            URL_SAFE_NO_PAD.encode(claims("1", NOW).to_string()),
            parts[2]
        );
        assert_eq!(check(&key, &forged, NOW), Err(Reason::BadSignature));

        // Truncated or empty signature.
        let truncated = format!("{}.{}.{}", parts[0], parts[1], &parts[2][..20]);
        assert_eq!(check(&key, &truncated, NOW), Err(Reason::BadSignature));
        let empty = format!("{}.{}.", parts[0], parts[1]);
        assert_eq!(check(&key, &empty, NOW), Err(Reason::BadSignature));
    }

    #[test]
    fn bad_issuer() {
        let key = signing_key();
        for iss in [
            json!("http://api.pulsars.science"),
            json!("https://api.pulsars.science/"),
            // The old site's issuer.
            json!("https://pulsars.science"),
            json!("http://localhost"),
            json!(["https://api.pulsars.science"]),
        ] {
            let t = with_claims(&key, |c| {
                c.insert("iss".into(), iss.clone());
            });
            assert_eq!(check(&key, &t, NOW), Err(Reason::BadIssuer), "{iss}");
        }
        let t = with_claims(&key, |c| {
            c.remove("iss");
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::BadIssuer));
    }

    #[test]
    fn audience_may_be_a_string_or_an_array() {
        let key = signing_key();
        let t = with_claims(&key, |c| {
            c.insert(
                "aud".into(),
                json!(["other.example", "api.pulsars.science"]),
            );
        });
        assert!(check(&key, &t, NOW).is_ok());
    }

    #[test]
    fn bad_audience() {
        let key = signing_key();
        for aud in [
            json!("other.example"),
            json!(["other.example"]),
            json!([]),
            json!("API.pulsars.science"),
            json!(1),
        ] {
            let t = with_claims(&key, |c| {
                c.insert("aud".into(), aud.clone());
            });
            assert_eq!(check(&key, &t, NOW), Err(Reason::BadAudience), "{aud}");
        }
        let t = with_claims(&key, |c| {
            c.remove("aud");
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::BadAudience));
    }

    #[test]
    fn expired_with_30_seconds_of_leeway() {
        let key = signing_key();
        let t = token(&key, KID, "42", NOW);
        let exp = NOW + 300;
        assert!(check(&key, &t, exp + 29).is_ok());
        assert_eq!(check(&key, &t, exp + 30), Err(Reason::Expired));
        assert_eq!(check(&key, &t, exp + 3600), Err(Reason::Expired));
    }

    #[test]
    fn not_yet_valid_with_30_seconds_of_leeway() {
        let key = signing_key();
        let t = token(&key, KID, "42", NOW);
        assert!(check(&key, &t, NOW - 30).is_ok());
        assert_eq!(check(&key, &t, NOW - 31), Err(Reason::NotYetValid));

        // `iat` in the future is refused even when `nbf` is fine.
        let t = with_claims(&key, |c| {
            c.insert("iat".into(), json!(NOW + 120));
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::NotYetValid));
    }

    #[test]
    fn time_claims_are_required_numbers() {
        let key = signing_key();
        for name in ["exp", "nbf", "iat"] {
            let t = with_claims(&key, |c| {
                c.remove(name);
            });
            assert_eq!(check(&key, &t, NOW), Err(Reason::Malformed), "no {name}");
            let t = with_claims(&key, |c| {
                c.insert(name.into(), json!("1792059151"));
            });
            assert_eq!(
                check(&key, &t, NOW),
                Err(Reason::Malformed),
                "string {name}"
            );
        }
        // Fractional NumericDates are allowed.
        let t = with_claims(&key, |c| {
            c.insert("exp".into(), json!((NOW + 300) as f64 + 0.5));
        });
        assert!(check(&key, &t, NOW).is_ok());
    }

    #[test]
    fn sub_and_jti_must_be_strings() {
        let key = signing_key();
        for (name, value) in [
            ("sub", json!(42)),
            ("sub", json!("")),
            ("jti", json!(null)),
            ("jti", json!(["x"])),
        ] {
            let t = with_claims(&key, |c| {
                c.insert(name.into(), value.clone());
            });
            assert_eq!(
                check(&key, &t, NOW),
                Err(Reason::Malformed),
                "{name}={value}"
            );
        }
        let t = with_claims(&key, |c| {
            c.remove("jti");
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::Malformed));
    }

    #[test]
    fn checks_run_in_contract_order() {
        let key = signing_key();
        // Expired, wrong audience and wrong issuer: the issuer is reported.
        let t = with_claims(&key, |c| {
            c.insert("iss".into(), json!("https://evil.example"));
            c.insert("aud".into(), json!("evil.example"));
            c.insert("exp".into(), json!(NOW - 3600));
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::BadIssuer));
        // A bad signature wins over every claim problem.
        let other = signing_key();
        let t = sign(&other, &header(), &json!({"exp": 0}));
        assert_eq!(check(&key, &t, NOW), Err(Reason::BadSignature));
    }

    #[test]
    fn optional_claims_are_typed() {
        let key = signing_key();
        let t = with_claims(&key, |c| {
            c.remove("name");
            c.insert("locale".into(), Value::Null);
            c.remove("roles");
        });
        let claims = check(&key, &t, NOW).expect("valid");
        assert_eq!(claims.name, None);
        assert_eq!(claims.locale, None);
        assert!(claims.roles.is_empty());

        let t = with_claims(&key, |c| {
            c.insert("roles".into(), json!(["admin", 3]));
        });
        assert_eq!(check(&key, &t, NOW), Err(Reason::Malformed));
    }

    #[test]
    fn every_reason_has_a_distinct_code() {
        let all = [
            Reason::Missing,
            Reason::Malformed,
            Reason::BadAlgorithm,
            Reason::UnknownKid,
            Reason::BadSignature,
            Reason::BadIssuer,
            Reason::BadAudience,
            Reason::Expired,
            Reason::NotYetValid,
        ];
        let codes: std::collections::HashSet<_> = all.iter().map(|r| r.code()).collect();
        assert_eq!(codes.len(), all.len());
        for reason in all {
            assert!(!reason.description().contains('"'));
        }
    }
}
