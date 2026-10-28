//! Password hashes: argon2id in the PHC string format
//! (`$argon2id$v=19$m=19456,t=2,p=1$…`), with the parameters OWASP
//! recommends. `ARGON2_MEMORY_KIB` lowers the memory for the tests only.

use std::sync::OnceLock;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::ApiError;

/// Memory cost, in KiB, when `ARGON2_MEMORY_KIB` is not set.
pub const DEFAULT_MEMORY_KIB: u32 = 19_456;
const ITERATIONS: u32 = 2;
const LANES: u32 = 1;

fn hasher(memory_kib: u32) -> Result<Argon2<'static>, ApiError> {
    let params = Params::new(memory_kib, ITERATIONS, LANES, None)
        .map_err(|e| ApiError::Internal(format!("argon2 parameters: {e}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn hash_now(memory_kib: u32, password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    hasher(memory_kib)?
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(format!("password hashing failed: {e}")))
}

/// The hash of `password`, computed off the async threads.
pub async fn hash(memory_kib: u32, password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || hash_now(memory_kib, &password))
        .await
        .map_err(|e| ApiError::Internal(format!("password hashing panicked: {e}")))?
}

/// Whether `password` matches `hash`. The parameters come from the hash
/// itself, so older hashes still verify after a change of parameters.
pub async fn verify(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

/// A hash to check against when there is no account, so that an unknown
/// login takes as long as a wrong password.
pub fn dummy(memory_kib: u32) -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| hash_now(memory_kib, "not a password of anyone").unwrap_or_default())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[tokio::test]
    async fn hashes_are_argon2id_and_verify() {
        let h = hash(64, "secret-pass".to_owned()).await.unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=64,t=2,p=1$"), "{h}");
        assert!(verify("secret-pass".to_owned(), h.clone()).await);
        assert!(!verify("secret-pasS".to_owned(), h).await);
        assert!(!verify("x".to_owned(), "not a hash".to_owned()).await);
    }
}
