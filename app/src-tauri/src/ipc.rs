use tauri::ipc::{InvokeBody, Request};

pub(crate) fn body(request: &Request) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(val) => {
            serde_json::from_value::<Vec<u8>>(val.clone()).map_err(|e| format!("Invalid body: {e}"))
        }
    }
}

pub(crate) fn raw_header(request: &Request, key: &str) -> Result<String, String> {
    request
        .headers()
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| format!("Missing {key} header"))
}

// Header values are Latin-1, so the frontend percent-encodes the ones that carry UTF-8.
pub(crate) fn header(request: &Request, key: &str) -> Result<String, String> {
    let raw = raw_header(request, key)?;
    percent_decode(&raw).ok_or_else(|| format!("Invalid {key} header"))
}

pub(crate) fn key_header(request: &Request) -> Result<Vec<u8>, String> {
    raw_header(request, "x-key")?
        .split(',')
        .map(|s| s.trim().parse::<u8>())
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Invalid key header: {e}"))
}

pub(crate) fn percent_decode(raw: &str) -> Option<String> {
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
        .ok()
}

pub(crate) fn percent_decode_lossy(raw: &str) -> String {
    percent_encoding::percent_decode_str(raw)
        .decode_utf8_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percent_encoded_header_decodes_to_its_text() {
        assert_eq!(
            percent_decode("Screenshot%202026.png").unwrap(),
            "Screenshot 2026.png"
        );
        assert_eq!(percent_decode("ni%C3%B1o.pdf").unwrap(), "niño.pdf");
        assert_eq!(percent_decode("plain.png").unwrap(), "plain.png");
        assert!(
            percent_decode("%FF").is_none(),
            "invalid UTF-8 is not a file name"
        );
    }
}
