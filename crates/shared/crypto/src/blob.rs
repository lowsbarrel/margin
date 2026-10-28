use aes_gcm_siv::aead::{Aead, KeyInit};
use aes_gcm_siv::{Aes256GcmSiv, Nonce};
use rand::TryRng;
use rand::rngs::SysRng;

use crate::{CryptoError, VaultKey};

const NONCE_BYTES: usize = 12;

fn cipher(key: &VaultKey) -> Aes256GcmSiv {
    Aes256GcmSiv::new_from_slice(key.as_bytes()).expect("a vault key is an AES-256 key")
}

#[must_use]
pub fn encrypt(key: &VaultKey, plaintext: &[u8]) -> Vec<u8> {
    let mut nonce_bytes = [0_u8; NONCE_BYTES];
    SysRng
        .try_fill_bytes(&mut nonce_bytes)
        .expect("the OS random source is available");
    let cipher = cipher(key);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .expect("GCM-SIV encryption only fails when the plaintext is too long");

    let mut blob = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    blob
}

pub fn decrypt(key: &VaultKey, blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let Some((nonce_bytes, ciphertext)) = blob.split_at_checked(NONCE_BYTES) else {
        return Err(CryptoError::TruncatedBlob);
    };
    match cipher(key).decrypt(Nonce::from_slice(nonce_bytes), ciphertext) {
        Ok(plaintext) => Ok(plaintext),
        Err(_) => Err(CryptoError::NotAuthentic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_KEY: VaultKey = VaultKey::from_bytes([
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ]);
    const FIXTURE_NONCE: [u8; 12] = [160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171];
    const FIXTURE_PLAINTEXT: &[u8] = b"margin-fixture";
    const FIXTURE_BLOB: [u8; 42] = [
        160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 51, 81, 78, 50, 116, 89, 196,
        241, 83, 213, 180, 20, 24, 61, 229, 214, 135, 56, 19, 62, 154, 178, 223, 182, 74, 42, 237,
        103, 215, 194,
    ];

    #[test]
    fn decrypts_a_blob_written_by_the_release_format() {
        assert_eq!(FIXTURE_BLOB[..NONCE_BYTES], FIXTURE_NONCE);
        assert_eq!(
            decrypt(&FIXTURE_KEY, &FIXTURE_BLOB).expect("the fixture authenticates"),
            FIXTURE_PLAINTEXT
        );
    }

    #[test]
    fn round_trips_a_blob() {
        let blob = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        assert_eq!(
            decrypt(&FIXTURE_KEY, &blob).expect("a fresh blob authenticates"),
            FIXTURE_PLAINTEXT
        );
    }

    #[test]
    fn lays_out_a_nonce_plus_ciphertext_and_tag() {
        let blob = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        assert_eq!(blob.len(), NONCE_BYTES + FIXTURE_PLAINTEXT.len() + 16);
        assert_eq!(blob.len(), FIXTURE_BLOB.len());
    }

    #[test]
    fn never_reuses_a_nonce() {
        let first = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        let second = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        assert_ne!(first, second);
        assert_ne!(first[..NONCE_BYTES], second[..NONCE_BYTES]);
    }

    #[test]
    fn rejects_a_tampered_blob() {
        let mut blob = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert_eq!(decrypt(&FIXTURE_KEY, &blob), Err(CryptoError::NotAuthentic));
    }

    #[test]
    fn rejects_another_key() {
        let blob = encrypt(&FIXTURE_KEY, FIXTURE_PLAINTEXT);
        assert_eq!(
            decrypt(&crate::random_key(), &blob),
            Err(CryptoError::NotAuthentic)
        );
    }

    #[test]
    fn rejects_a_blob_shorter_than_a_nonce() {
        assert_eq!(
            decrypt(&FIXTURE_KEY, &[0; NONCE_BYTES - 1]),
            Err(CryptoError::TruncatedBlob)
        );
        assert_eq!(decrypt(&FIXTURE_KEY, &[]), Err(CryptoError::TruncatedBlob));
    }
}
