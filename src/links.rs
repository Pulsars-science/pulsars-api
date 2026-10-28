//! Links to pages of the Laravel site (`web_url`, `profile_url`, download
//! links). Every response object points back to its canonical page on
//! `https://pulsars.science` (`docs/contract/README.md`).

use crate::config::SITE_URL;

/// Percent-encodes everything but RFC 3986 unreserved characters (and `/`
/// when `keep_slashes`, for a path).
pub fn encode(value: &str, keep_slashes: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (keep_slashes && byte == b'/');
        if keep {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The link of an address verification mail:
/// `https://pulsars.science/email/verifier/{token}`.
pub fn email_verification(token: &str) -> String {
    format!("{SITE_URL}/email/verifier/{}", encode(token, false))
}

/// The link of a password reset mail, as Laravel's `ResetPassword` builds
/// it: `https://pulsars.science/reset-password/{token}?email={email}`.
pub fn password_reset(token: &str, email: &str) -> String {
    format!(
        "{SITE_URL}/reset-password/{}?email={}",
        encode(token, false),
        encode(email, false)
    )
}

/// `https://pulsars.science/membres/{pseudo}`.
pub fn member(pseudo: &str) -> String {
    format!("{SITE_URL}/membres/{}", encode(pseudo, false))
}

/// `https://pulsars.science/actualite/{news}`.
pub fn news(slug: &str) -> String {
    format!("{SITE_URL}/actualite/{}", encode(slug, false))
}

/// `https://pulsars.science/cours/{course}`.
pub fn course(slug: &str) -> String {
    format!("{SITE_URL}/cours/{}", encode(slug, false))
}

/// `https://pulsars.science/cours/{course}/lecon/{lesson}`.
pub fn lesson(course: &str, lesson: &str) -> String {
    format!("{}/lecon/{}", self::course(course), encode(lesson, false))
}

/// `https://pulsars.science/cours/{course}/exercice/{id}`.
pub fn exercise(course: &str, id: i64) -> String {
    format!("{}/exercice/{id}", self::course(course))
}

/// `https://pulsars.science/examens/{slug}`.
pub fn exam(slug: &str) -> String {
    format!("{SITE_URL}/examens/{}", encode(slug, false))
}

/// The statement PDF Laravel serves for a hosted exam.
pub fn exam_pdf(slug: &str) -> String {
    format!("{}/telecharger", exam(slug))
}

/// `https://pulsars.science/ressources/{slug}`.
pub fn resource(slug: &str) -> String {
    format!("{SITE_URL}/ressources/{}", encode(slug, false))
}

/// The file Laravel serves for a hosted resource.
pub fn resource_download(slug: &str) -> String {
    format!("{}/telecharger", resource(slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_percent_encoded() {
        assert_eq!(encode("jikuji", false), "jikuji");
        assert_eq!(encode("a b/c?é", false), "a%20b%2Fc%3F%C3%A9");
        assert_eq!(encode("avatars/x y.webp", true), "avatars/x%20y.webp");
    }

    #[test]
    fn site_links_match_laravel_routes() {
        assert_eq!(
            lesson("les-bases-du-calcul", "priorites-operatoires"),
            "https://pulsars.science/cours/les-bases-du-calcul/lecon/priorites-operatoires"
        );
        assert_eq!(
            exercise("les-bases-du-calcul", 3),
            "https://pulsars.science/cours/les-bases-du-calcul/exercice/3"
        );
        assert_eq!(
            exam_pdf("bac-2026"),
            "https://pulsars.science/examens/bac-2026/telecharger"
        );
        assert_eq!(
            resource_download("misselbooks-abstract-algebra"),
            "https://pulsars.science/ressources/misselbooks-abstract-algebra/telecharger"
        );
        assert_eq!(member("a b"), "https://pulsars.science/membres/a%20b");
    }
}
