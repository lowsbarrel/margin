use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use bip39::Mnemonic;
use rand::TryRng;
use rand::rngs::SysRng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::{InvokeBody, Request, Response};

#[derive(Serialize, specta::Type)]
pub struct VaultKeys {
    pub vault_id: String,
    pub encryption_key: Vec<u8>,
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

#[tauri::command]
#[specta::specta]
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

#[tauri::command]
pub fn encrypt_blob_cmd(request: Request) -> Result<Response, String> {
    let key = parse_key_header(&request)?;
    let plaintext = parse_body(&request)?;
    let result = encrypt_blob(plaintext, key)?;
    Ok(Response::new(result))
}

#[tauri::command]
pub fn decrypt_blob_cmd(request: Request) -> Result<Response, String> {
    let key = parse_key_header(&request)?;
    let ciphertext = parse_body(&request)?;
    let result = decrypt_blob(ciphertext, key)?;
    Ok(Response::new(result))
}

fn parse_key_header(request: &Request) -> Result<Vec<u8>, String> {
    let header = request
        .headers()
        .get("x-key")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing x-key header")?;
    header
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Invalid key header: {e}"))
}

fn parse_body(request: &Request) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(val) => {
            serde_json::from_value::<Vec<u8>>(val.clone()).map_err(|e| format!("Invalid body: {e}"))
        }
    }
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
