use s3::creds::Credentials;
use s3::{Bucket, Region};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Mutex;
use tauri::State;

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

// Hand-written so a `{:?}` on this, or on anything embedding it like AppSettings, cannot print the keys.
impl fmt::Debug for S3Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

pub struct S3State(pub Mutex<Option<CachedS3>>);

#[derive(Clone)]
pub struct CachedS3 {
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
    .map_err(|_| "Invalid S3 credentials".to_string())?;

    let mut bucket = Bucket::new(&config.bucket, region, credentials)
        .map_err(|e| format!("Bucket error: {e}"))?;
    bucket.set_path_style();

    Ok(bucket)
}

fn get_bucket(state: &State<'_, S3State>) -> Result<Box<Bucket>, String> {
    let s3 = state.0.lock().map_err(|e| e.to_string())?;
    let cached = s3.as_ref().ok_or("S3 not configured")?;
    Ok(cached.bucket.clone())
}

#[tauri::command]
#[specta::specta]
pub fn s3_configure(config: S3Config, state: State<'_, S3State>) -> Result<(), String> {
    let bucket = make_bucket(&config)?;
    let mut s3 = state.0.lock().map_err(|e| e.to_string())?;
    *s3 = Some(CachedS3 { bucket });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn s3_test_connection(state: State<'_, S3State>) -> Result<String, String> {
    let bucket = get_bucket(&state)?;
    let results = bucket
        .list("".to_string(), Some("/".to_string()))
        .await
        .map_err(|e| format!("Connection failed: {e}"))?;

    Ok(format!("Connected. {} prefixes found.", results.len()))
}
