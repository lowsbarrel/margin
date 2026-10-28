use std::fmt;

use rand::TryRng;
use rand::rngs::SysRng;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VaultKey([u8; 32]);

impl VaultKey {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultKey(redacted)")
    }
}

#[must_use]
pub fn random_key() -> VaultKey {
    let mut bytes = [0_u8; 32];
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("the OS random source is available");
    VaultKey(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_key() {
        let key = VaultKey::from_bytes([7; 32]);
        assert_eq!(format!("{key:?}"), "VaultKey(redacted)");
    }

    #[test]
    fn random_keys_differ() {
        assert_ne!(random_key(), random_key());
    }
}
