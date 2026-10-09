use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use bip39::Mnemonic;
use rand::TryRng;
use rand::rngs::SysRng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use tauri::State;

// The derived vault key lives only here: no command returns it and the webview never holds it.
pub struct VaultKeyState(pub Mutex<Option<([u8; 32], String)>>);

#[derive(Serialize, specta::Type)]
pub struct VaultId {
    pub vault_id: String,
}

#[tauri::command]
#[specta::specta]
pub fn generate_mnemonic() -> Result<String, String> {
    let mut entropy = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut entropy)
        .map_err(|e| format!("OS random source failed: {e}"))?;
    let mnemonic = Mnemonic::from_entropy(&entropy).map_err(|e| e.to_string())?;
    Ok(mnemonic.to_string())
}

pub(crate) fn unlock_mnemonic(mnemonic: &str, key_state: &VaultKeyState) -> Result<String, String> {
    let mnemonic: Mnemonic = mnemonic.parse().map_err(|e: bip39::Error| e.to_string())?;
    let seed = mnemonic.to_seed("");

    let mut key = [0u8; 32];
    key.copy_from_slice(&seed[32..64]);

    let mut hasher = Sha256::new();
    hasher.update(&seed[0..32]);
    let vault_id = hex::encode(hasher.finalize());

    *key_state.0.lock().map_err(|e| e.to_string())? = Some((key, vault_id.clone()));
    Ok(vault_id)
}

#[tauri::command]
#[specta::specta]
pub fn derive_vault_keys(
    mnemonic: &str,
    key_state: State<'_, VaultKeyState>,
) -> Result<VaultId, String> {
    Ok(VaultId {
        vault_id: unlock_mnemonic(mnemonic, &key_state)?,
    })
}

pub(crate) fn current_key(key_state: &State<'_, VaultKeyState>) -> Result<[u8; 32], String> {
    key_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|(key, _)| *key)
        .ok_or_else(|| "Vault is locked".to_string())
}

pub(crate) fn current_key_for_prefix(
    key_state: &State<'_, VaultKeyState>,
    s3_prefix: &str,
) -> Result<[u8; 32], String> {
    let guard = key_state.0.lock().map_err(|e| e.to_string())?;
    let (key, vault_id) = guard
        .as_ref()
        .ok_or_else(|| "Vault is locked".to_string())?;
    if s3_prefix != format!("{vault_id}/") {
        return Err("Vault changed mid-sync".to_string());
    }
    Ok(*key)
}

pub fn encrypt_blob(plaintext: Vec<u8>, key: Vec<u8>) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    let cipher = Aes256GcmSiv::new_from_slice(&key).map_err(|e| format!("Invalid key: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    SysRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| format!("OS random source failed: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

pub fn decrypt_blob(ciphertext: Vec<u8>, key: Vec<u8>) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".into());
    }
    let cipher = Aes256GcmSiv::new_from_slice(&key).map_err(|e| format!("Invalid key: {e}"))?;

    let nonce = Nonce::from_slice(&ciphertext[..12]);
    let plaintext = cipher
        .decrypt(nonce, &ciphertext[12..])
        .map_err(|e| format!("Decryption failed: {e}"))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics_do_not_reuse_entropy() {
        let first = generate_mnemonic().unwrap();
        let second = generate_mnemonic().unwrap();
        assert_eq!(first.split_whitespace().count(), 12);
        assert_ne!(first, second);
    }

    // A reused nonce is the one failure GCM-SIV cannot detect on its own.
    #[test]
    fn blobs_do_not_reuse_a_nonce() {
        let key = vec![7u8; 32];
        let first = encrypt_blob(b"hello".to_vec(), key.clone()).unwrap();
        let second = encrypt_blob(b"hello".to_vec(), key.clone()).unwrap();
        assert_ne!(first, second);
        assert_eq!(decrypt_blob(first, key.clone()).unwrap(), b"hello");
        assert_eq!(decrypt_blob(second, key).unwrap(), b"hello");
    }
}
