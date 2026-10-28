use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::VaultKey;

type HmacSha256 = Hmac<Sha256>;

const OBJECT_KEY_BYTES: usize = 16;

#[must_use]
pub fn object_key(key: &VaultKey, relative_path: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key.as_bytes())
        .expect("HMAC accepts a key of any size");
    mac.update(relative_path.as_bytes());
    let digest = mac.finalize().into_bytes();
    hex::encode(&digest[..OBJECT_KEY_BYTES])
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_KEY: VaultKey = VaultKey::from_bytes([
        0x9a, 0x5a, 0xc4, 0x0b, 0x38, 0x9c, 0xd3, 0x70, 0xd0, 0x86, 0x20, 0x6d, 0xec, 0x8a, 0xa6,
        0xc4, 0x3d, 0xae, 0xa6, 0x69, 0x0f, 0x20, 0xad, 0x3d, 0x8d, 0x48, 0xb2, 0xd2, 0xce, 0x9e,
        0x38, 0xe4,
    ]);

    #[test]
    fn matches_the_release_object_keys() {
        let cases = [
            ("Notes/Plan.md", "29cb785e6d16ab5dfe0d85c3694d0168"),
            ("notes/2024/Plan.v2.md", "d032ab0037e9eb06e8beedd3c8ac2812"),
            ("a", "40cd9465838eaa0b8fa70aa2444b9ca2"),
            (
                "C:/Users/test/Vault/note.md",
                "0e33c905c29b722c053fd70b6339fc05",
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(object_key(&FIXTURE_KEY, path), expected, "{path}");
        }
    }

    #[test]
    fn is_256_bits_of_hex() {
        let key = crate::random_key();
        assert_eq!(object_key(&key, "note.md").len(), 2 * OBJECT_KEY_BYTES);
    }

    #[test]
    fn separates_paths_that_differ() {
        let first = object_key(&FIXTURE_KEY, "note.md");
        let second = object_key(&FIXTURE_KEY, "note.md ");
        assert_ne!(first, second);
    }
}
