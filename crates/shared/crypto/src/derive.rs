use std::str::FromStr;

use bip39::Mnemonic;
use margin_kernel::Secret;
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::{Digest, Sha256};

use crate::{CryptoError, DerivedVault, VaultId, VaultKey};

const MNEMONIC_ENTROPY_BYTES: usize = 16;
const SEED_BYTES: usize = 64;
const HALF_SEED_BYTES: usize = 32;

#[must_use]
pub fn generate_mnemonic() -> Secret<String> {
    let mut entropy = [0_u8; MNEMONIC_ENTROPY_BYTES];
    SysRng
        .try_fill_bytes(&mut entropy)
        .expect("the OS random source is available");
    let mnemonic =
        Mnemonic::from_entropy(&entropy).expect("16 bytes of entropy are a valid length");
    Secret::new(mnemonic.to_string())
}

pub fn derive_vault(mnemonic: &str) -> Result<DerivedVault, CryptoError> {
    let mnemonic = Mnemonic::from_str(mnemonic)
        .map_err(|error| CryptoError::InvalidMnemonic(error.to_string()))?;
    let seed: [u8; SEED_BYTES] = mnemonic.to_seed("");
    let (identifier_material, key_material) = seed.split_at(HALF_SEED_BYTES);

    let mut hasher = Sha256::new();
    hasher.update(identifier_material);
    let id = VaultId::from_hex(hex::encode(hasher.finalize()));

    let mut key = [0_u8; HALF_SEED_BYTES];
    key.copy_from_slice(key_material);
    Ok(DerivedVault {
        id,
        key: VaultKey::from_bytes(key),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const FIXTURE_VAULT_ID: &str =
        "209947b258e347c7f68ea97be6a866a7771d2363696ff1fa585d6d28ae45e1af";
    const FIXTURE_KEY: [u8; 32] = [
        0x9a, 0x5a, 0xc4, 0x0b, 0x38, 0x9c, 0xd3, 0x70, 0xd0, 0x86, 0x20, 0x6d, 0xec, 0x8a, 0xa6,
        0xc4, 0x3d, 0xae, 0xa6, 0x69, 0x0f, 0x20, 0xad, 0x3d, 0x8d, 0x48, 0xb2, 0xd2, 0xce, 0x9e,
        0x38, 0xe4,
    ];

    #[test]
    fn derives_the_release_vault_id_and_key() {
        let vault = derive_vault(FIXTURE_MNEMONIC).expect("the fixture mnemonic is valid");
        assert_eq!(vault.id.as_str(), FIXTURE_VAULT_ID);
        assert_eq!(vault.key.as_bytes(), &FIXTURE_KEY);
    }

    #[test]
    fn derives_again_from_a_generated_mnemonic() {
        let mnemonic = generate_mnemonic();
        let vault = derive_vault(mnemonic.expose()).expect("a generated mnemonic is valid");
        assert_eq!(vault.id.as_str().len(), 64);
        assert_ne!(vault.key.as_bytes(), &FIXTURE_KEY);
    }

    #[test]
    fn rejects_a_phrase_that_is_not_a_mnemonic() {
        let error = derive_vault("this is not a mnemonic")
            .expect_err("words outside the list are rejected");
        assert!(matches!(error, CryptoError::InvalidMnemonic(_)));
    }

    #[test]
    fn generated_mnemonics_do_not_repeat() {
        let first = generate_mnemonic();
        let second = generate_mnemonic();
        assert_eq!(first.expose().split_whitespace().count(), 12);
        assert_ne!(first.expose(), second.expose());
    }

    #[test]
    fn debug_never_prints_a_mnemonic() {
        assert_eq!(format!("{:?}", generate_mnemonic()), "Secret(redacted)");
    }
}
