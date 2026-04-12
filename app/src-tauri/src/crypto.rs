use aes_gcm_siv::{
    aead::{Aead, KeyInit, OsRng},
    Aes256GcmSiv, Nonce,
};
use bip39::Mnemonic;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
pub struct VaultKeys {
    pub vault_id: String,
    pub encryption_key: Vec<u8>,
}

/// Generate a new BIP-39 12-word mnemonic (128-bit entropy).
#[tauri::command]
pub fn generate_mnemonic() -> Result<String, String> {
    let mut entropy = [0u8; 16]; // 128 bits = 12 words
    OsRng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy).map_err(|e| e.to_string())?;
    Ok(mnemonic.to_string())
}

/// Derive vault_id and encryption_key from a BIP-39 mnemonic.
#[tauri::command]
pub fn derive_vault_keys(mnemonic: &str) -> Result<VaultKeys, String> {
    let mnemonic: Mnemonic = mnemonic.parse().map_err(|e: bip39::Error| e.to_string())?;
    let seed = mnemonic.to_seed("");

    let vault_id_raw = &seed[0..32];
    let encryption_key = seed[32..64].to_vec();

    let mut hasher = Sha256::new();
    hasher.update(vault_id_raw);
    let vault_id = hex::encode(hasher.finalize());

    Ok(VaultKeys {
        vault_id,
        encryption_key,
    })
}

/// Encrypt plaintext bytes with AES-256-GCM-SIV. Returns nonce || ciphertext.
#[tauri::command]
pub fn encrypt_blob(plaintext: Vec<u8>, key: Vec<u8>) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    let cipher = Aes256GcmSiv::new_from_slice(&key).map_err(|e| format!("Invalid key: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt nonce || ciphertext with AES-256-GCM-SIV.
#[tauri::command]
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
