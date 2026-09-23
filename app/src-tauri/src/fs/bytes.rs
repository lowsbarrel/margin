use super::{VaultPathState, atomic_write, ensure_in_vault, vault_root};
use std::fs;
use std::path::Path;
use tauri::ipc::{InvokeBody, Request, Response};

#[tauri::command]
pub fn read_file_bytes(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<Response, String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let bytes = fs::read(&p).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(Response::new(bytes))
}

// Header values are Latin-1, so the frontend percent-encodes what it puts here.
pub(crate) fn header(request: &Request, key: &str) -> Result<String, String> {
    let raw = request
        .headers()
        .get(key)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| format!("Missing {key} header"))?;
    decode_header(raw).ok_or_else(|| format!("Invalid {key} header"))
}

pub(crate) fn decode_header(raw: &str) -> Option<String> {
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
        .ok()
}

pub(crate) fn request_body(request: &Request) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(val) => {
            serde_json::from_value::<Vec<u8>>(val.clone()).map_err(|e| format!("Invalid body: {e}"))
        }
    }
}

#[tauri::command]
pub fn write_file_bytes(
    request: Request,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let path = header(&request, "x-path")?;
    let data = request_body(&request)?;
    let p = ensure_in_vault(&path, &vault_path_state)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(&p, &data)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &p);
    Ok(())
}

// The destination came from the native save dialog and is outside the vault by design.
#[tauri::command]
pub fn save_file_bytes(request: Request) -> Result<(), String> {
    let path = request
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing x-path header".to_string())?;
    let data = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(val) => serde_json::from_value::<Vec<u8>>(val.clone())
            .map_err(|e| format!("Invalid body: {e}"))?,
    };
    let dest = Path::new(path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(dest, &data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percent_encoded_header_decodes_to_its_text() {
        assert_eq!(
            decode_header("Screenshot%202026.png").unwrap(),
            "Screenshot 2026.png"
        );
        assert_eq!(decode_header("ni%C3%B1o.pdf").unwrap(), "niño.pdf");
        assert_eq!(decode_header("plain.png").unwrap(), "plain.png");
        assert!(
            decode_header("%FF").is_none(),
            "invalid UTF-8 is not a file name"
        );
    }
}
