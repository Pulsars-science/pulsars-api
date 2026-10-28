//! Private messages at rest: AES-256-GCM under the API's `MESSAGES_KEY`
//! (`config::MessagesKey`), as Laravel's `encrypted` cast did with its
//! `APP_KEY`. The database never holds a message in clear.
//!
//! A sealed body is `base64(nonce ‖ ciphertext ‖ tag)`, with a random 96-bit
//! nonce, stored in `site.direct_messages.contenu` next to the version of the
//! key (`key_version`). The conversation's id is the associated data: a body
//! copied into another conversation does not open.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::config::MessagesKey;

const NONCE_BYTES: usize = 12;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SealError {
    #[error("no randomness for a nonce")]
    Random,
    #[error("the message was sealed with key version {0}, which is not configured")]
    UnknownVersion(i16),
    /// Not base64, too short, or tampered with.
    #[error("the sealed message does not open")]
    Corrupt,
}

/// Seals a private message of conversation `conversation_id`. Returns the
/// stored text and the key version to store with it.
pub fn seal(
    key: &MessagesKey,
    conversation_id: i64,
    body: &str,
) -> Result<(String, i16), SealError> {
    let mut nonce = [0u8; NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|_| SealError::Random)?;
    let cipher = cipher(key);
    let sealed = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: body.as_bytes(),
                aad: &aad(conversation_id),
            },
        )
        .map_err(|_| SealError::Corrupt)?;
    let mut stored = Vec::with_capacity(NONCE_BYTES + sealed.len());
    stored.extend_from_slice(&nonce);
    stored.extend_from_slice(&sealed);
    Ok((STANDARD.encode(stored), key.version))
}

/// Opens what [`seal`] stored.
pub fn open(
    key: &MessagesKey,
    conversation_id: i64,
    stored: &str,
    version: i16,
) -> Result<String, SealError> {
    if version != key.version {
        return Err(SealError::UnknownVersion(version));
    }
    let bytes = STANDARD.decode(stored).map_err(|_| SealError::Corrupt)?;
    if bytes.len() < NONCE_BYTES {
        return Err(SealError::Corrupt);
    }
    let (nonce, sealed) = bytes.split_at(NONCE_BYTES);
    let nonce: [u8; NONCE_BYTES] = nonce.try_into().map_err(|_| SealError::Corrupt)?;
    let clear = cipher(key)
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: sealed,
                aad: &aad(conversation_id),
            },
        )
        .map_err(|_| SealError::Corrupt)?;
    String::from_utf8(clear).map_err(|_| SealError::Corrupt)
}

fn cipher(key: &MessagesKey) -> Aes256Gcm {
    Aes256Gcm::new(&key.key.into())
}

fn aad(conversation_id: i64) -> Vec<u8> {
    format!("pulsars:direct_messages:{conversation_id}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8, version: i16) -> MessagesKey {
        MessagesKey {
            version,
            key: [byte; 32],
        }
    }

    #[test]
    fn a_sealed_message_opens_with_its_key_only() {
        let k = key(7, 1);
        let secret = "Rendez-vous à 14h, code portail 4791.";
        let (stored, version) = seal(&k, 42, secret).unwrap();
        assert_eq!(version, 1);
        assert!(!stored.contains("4791") && !stored.contains("portail"));
        assert_eq!(open(&k, 42, &stored, 1).unwrap(), secret);

        // Another key, another conversation, another version: nothing.
        assert_eq!(open(&key(8, 1), 42, &stored, 1), Err(SealError::Corrupt));
        assert_eq!(open(&k, 43, &stored, 1), Err(SealError::Corrupt));
        assert_eq!(open(&k, 42, &stored, 2), Err(SealError::UnknownVersion(2)));
        assert_eq!(open(&k, 42, "not base64!", 1), Err(SealError::Corrupt));
        assert_eq!(open(&k, 42, "AAAA", 1), Err(SealError::Corrupt));
    }

    #[test]
    fn every_seal_uses_a_fresh_nonce() {
        let k = key(7, 1);
        let (a, _) = seal(&k, 1, "même texte").unwrap();
        let (b, _) = seal(&k, 1, "même texte").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn a_tampered_body_does_not_open() {
        let k = key(7, 1);
        let (stored, _) = seal(&k, 1, "bonjour").unwrap();
        let mut bytes = STANDARD.decode(&stored).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_eq!(
            open(&k, 1, &STANDARD.encode(bytes), 1),
            Err(SealError::Corrupt)
        );
    }
}
