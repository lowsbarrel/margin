use s3::creds::Credentials;
use s3::{Bucket, Region};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::State;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

pub struct S3State(pub Mutex<Option<CachedS3>>);

/// Cached S3 config + pre-built bucket to avoid recreating it on every command
#[derive(Clone)]
pub struct CachedS3 {
    pub config: S3Config,
    pub bucket: Box<Bucket>,
}

fn make_bucket(config: &S3Config) -> Result<Box<Bucket>, String> {
    let region = Region::Custom {
        region: config.region.clone(),
        endpoint: config.endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.access_key),
        Some(&config.secret_key),
        None,
        None,
        None,
    )
    .map_err(|e| format!("Invalid credentials: {e}"))?;

    let mut bucket = Bucket::new(&config.bucket, region, credentials)
        .map_err(|e| format!("Bucket error: {e}"))?;
    bucket.set_path_style(); // Needed for MinIO, R2, etc.

    Ok(bucket)
}

fn get_bucket(state: &State<'_, S3State>) -> Result<Box<Bucket>, String> {
    let s3 = state.0.lock().map_err(|e| e.to_string())?;
    let cached = s3.as_ref().ok_or("S3 not configured")?;
    Ok(cached.bucket.clone())
}

#[tauri::command]
pub fn s3_configure(config: S3Config, state: State<'_, S3State>) -> Result<(), String> {
    let bucket = make_bucket(&config)?;
    let mut s3 = state.0.lock().map_err(|e| e.to_string())?;
    *s3 = Some(CachedS3 { config, bucket });
    Ok(())
}

#[tauri::command]
pub fn s3_get_config(state: State<'_, S3State>) -> Result<Option<S3Config>, String> {
    let s3 = state.0.lock().map_err(|e| e.to_string())?;
    Ok(s3.as_ref().map(|c| c.config.clone()))
}

#[tauri::command]
pub async fn s3_test_connection(state: State<'_, S3State>) -> Result<String, String> {
    let bucket = get_bucket(&state)?;
    let results = bucket
        .list("".to_string(), Some("/".to_string()))
        .await
        .map_err(|e| format!("Connection failed: {e}"))?;

    Ok(format!("Connected. {} prefixes found.", results.len()))
}

/// 5 MB — minimum part size for S3 multipart uploads
const MULTIPART_THRESHOLD: usize = 5 * 1024 * 1024;
const PART_SIZE: usize = 5 * 1024 * 1024;
/// Maximum retries per multipart chunk upload
const MULTIPART_MAX_RETRIES: u32 = 3;

#[tauri::command]
pub async fn s3_upload(request: Request<'_>, state: State<'_, S3State>) -> Result<(), String> {
    let key: String = request
        .headers()
        .get("x-key")
        .and_then(|v: &tauri::http::HeaderValue| v.to_str().ok())
        .ok_or("Missing x-key header")?
        .to_string();
    let data = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(val) => serde_json::from_value::<Vec<u8>>(val.clone())
            .map_err(|e| format!("Invalid body: {e}"))?,
    };
    let bucket = get_bucket(&state)?;

    if data.len() >= MULTIPART_THRESHOLD {
        // ── Multipart upload for large files ─────────────────────────
        let init = bucket
            .initiate_multipart_upload(&key, "application/octet-stream")
            .await
            .map_err(|e| format!("Multipart init failed: {e}"))?;

        let upload_id = init.upload_id;

        let mut tasks = tokio::task::JoinSet::new();

        for (i, chunk) in data.chunks(PART_SIZE).enumerate() {
            let part_number = (i as u32) + 1;
            let chunk_data = chunk.to_vec();
            let bucket = bucket.clone();
            let key = key.clone();
            let upload_id = upload_id.clone();

            tasks.spawn(async move {
                let mut last_err = String::new();
                for attempt in 0..=MULTIPART_MAX_RETRIES {
                    match bucket
                        .put_multipart_chunk(
                            chunk_data.clone(),
                            &key,
                            part_number,
                            &upload_id,
                            "application/octet-stream",
                        )
                        .await
                    {
                        Ok(part) => return Ok((part_number, part)),
                        Err(e) => {
                            last_err = format!("{e}");
                            if attempt < MULTIPART_MAX_RETRIES {
                                let delay = std::time::Duration::from_millis(100 << attempt);
                                tokio::time::sleep(delay).await;
                            }
                        }
                    }
                }
                Err(format!(
                    "Multipart chunk {part_number} failed after {} retries: {last_err}",
                    MULTIPART_MAX_RETRIES
                ))
            });
        }

        let mut indexed_parts: Vec<(u32, _)> = Vec::new();
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(part)) => indexed_parts.push(part),
                Ok(Err(e)) => {
                    let _ = bucket.abort_upload(&key, &upload_id).await;
                    return Err(e);
                }
                Err(e) => {
                    let _ = bucket.abort_upload(&key, &upload_id).await;
                    return Err(format!("Task panicked: {e}"));
                }
            }
        }

        // Sort by part number for correct multipart completion
        indexed_parts.sort_by_key(|(num, _)| *num);
        let parts: Vec<_> = indexed_parts.into_iter().map(|(_, part)| part).collect();

        bucket
            .complete_multipart_upload(&key, &upload_id, parts)
            .await
            .map_err(|e| format!("Multipart complete failed: {e}"))?;
    } else {
        // ── Simple upload for small files ────────────────────────────
        let _ = bucket
            .put_object(key.as_str(), &data)
            .await
            .map_err(|e| format!("Upload failed: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn s3_download(key: String, state: State<'_, S3State>) -> Result<Response, String> {
    let bucket = get_bucket(&state)?;
    let response = bucket
        .get_object(&key)
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    Ok(Response::new(response.to_vec()))
}

#[tauri::command]
pub async fn s3_list(prefix: String, state: State<'_, S3State>) -> Result<Vec<String>, String> {
    let bucket = get_bucket(&state)?;
    let results = bucket
        .list(prefix, None)
        .await
        .map_err(|e| format!("List failed: {e}"))?;

    let keys: Vec<String> = results
        .into_iter()
        .flat_map(|r| r.contents)
        .map(|obj| obj.key)
        .collect();

    Ok(keys)
}

#[tauri::command]
pub async fn s3_delete(key: String, state: State<'_, S3State>) -> Result<(), String> {
    let bucket = get_bucket(&state)?;
    bucket
        .delete_object(&key)
        .await
        .map_err(|e| format!("Delete failed: {e}"))?;

    Ok(())
}
