use std::time::{SystemTime, UNIX_EPOCH};

use margin_kernel::{BoxFuture, StoreError};
use margin_sync::BlobStore;
use reqwest::header::CONTENT_LENGTH;
use reqwest::{Client, Method, Response, StatusCode};

use crate::config::S3Config;
use crate::sigv4::{
    Credentials, SignableRequest, amz_dates, authorization, canonical_query, sha256_hex, uri_encode,
};

#[derive(Clone, Debug)]
pub struct S3BlobStore {
    client: Client,
    config: S3Config,
}

impl S3BlobStore {
    #[must_use]
    pub fn new(config: S3Config) -> Self {
        let client = Client::builder()
            .build()
            .expect("the HTTP client builds with the bundled TLS backend");
        Self { client, config }
    }

    #[must_use]
    pub fn config(&self) -> &S3Config {
        &self.config
    }

    fn host(&self) -> String {
        let endpoint = self
            .config
            .endpoint
            .split_once("://")
            .map_or(self.config.endpoint.as_str(), |(_, rest)| rest);
        endpoint.split('/').next().unwrap_or_default().to_owned()
    }

    fn bucket_path(&self) -> String {
        format!("/{}", self.config.bucket)
    }

    fn object_path(&self, key: &str) -> String {
        format!("{}/{}", self.bucket_path(), uri_encode(key, false))
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Vec<u8>,
    ) -> reqwest::RequestBuilder {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let (date, timestamp) = amz_dates(now);
        let payload_hash = sha256_hex(&body);
        let headers = vec![
            ("host".to_owned(), self.host()),
            ("x-amz-content-sha256".to_owned(), payload_hash.clone()),
            ("x-amz-date".to_owned(), timestamp.clone()),
        ];
        let signable = SignableRequest {
            method: method.as_str(),
            uri: path,
            query,
            headers: &headers,
            payload_hash: &payload_hash,
            timestamp: &timestamp,
            date: &date,
        };
        let credentials = Credentials {
            access_key: &self.config.access_key,
            secret_key: &self.config.secret_key,
            region: &self.config.region,
        };
        let value = authorization(&credentials, &signable);
        let mut url = format!("{}{path}", self.config.endpoint.trim_end_matches('/'));
        if !query.is_empty() {
            url.push('?');
            url.push_str(&canonical_query(query));
        }
        self.client
            .request(method, url)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", timestamp)
            .header("authorization", value)
            .body(body)
    }

    async fn list_page(
        &self,
        prefix: &str,
        token: Option<&str>,
    ) -> Result<(Vec<String>, Option<String>), StoreError> {
        let mut query = vec![
            ("list-type".to_owned(), "2".to_owned()),
            ("prefix".to_owned(), prefix.to_owned()),
        ];
        if let Some(token) = token {
            query.push(("continuation-token".to_owned(), token.to_owned()));
        }
        let path = self.bucket_path();
        let response = self
            .request(Method::GET, &path, &query, Vec::new())
            .send()
            .await
            .map_err(|error| unavailable(&error))?;
        let body = check(response, "list")?
            .text()
            .await
            .map_err(|error| unavailable(&error))?;
        Ok((keys_in(&body), next_token(&body)))
    }
}

impl BlobStore for S3BlobStore {
    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let path = self.object_path(key);
            let response = self
                .request(Method::PUT, &path, &[], body)
                .send()
                .await
                .map_err(|error| unavailable(&error))?;
            check(response, "put")?;
            Ok(())
        })
    }

    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Vec<u8>, StoreError>> {
        Box::pin(async move {
            let path = self.object_path(key);
            let response = self
                .request(Method::GET, &path, &[], Vec::new())
                .send()
                .await
                .map_err(|error| unavailable(&error))?;
            if response.status() == StatusCode::NOT_FOUND {
                return Err(StoreError::NotFound(key.to_owned()));
            }
            let bytes = check(response, "get")?
                .bytes()
                .await
                .map_err(|error| unavailable(&error))?;
            Ok(bytes.to_vec())
        })
    }

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<u64>, StoreError>> {
        Box::pin(async move {
            let path = self.object_path(key);
            let response = self
                .request(Method::HEAD, &path, &[], Vec::new())
                .send()
                .await
                .map_err(|error| unavailable(&error))?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            Ok(check(response, "head")?
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok()))
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let path = self.object_path(key);
            let response = self
                .request(Method::DELETE, &path, &[], Vec::new())
                .send()
                .await
                .map_err(|error| unavailable(&error))?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(());
            }
            check(response, "delete")?;
            Ok(())
        })
    }

    fn list_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> BoxFuture<'a, Result<Vec<String>, StoreError>> {
        Box::pin(async move {
            let mut keys = Vec::new();
            let mut token: Option<String> = None;
            loop {
                let (page, next) = self.list_page(prefix, token.as_deref()).await?;
                keys.extend(page);
                match next {
                    Some(value) => token = Some(value),
                    None => break,
                }
            }
            Ok(keys)
        })
    }
}

fn unavailable(error: &reqwest::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

fn check(response: Response, action: &str) -> Result<Response, StoreError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = format!("{action} failed with status {status}");
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        Err(StoreError::Unavailable(message))
    } else {
        Err(StoreError::Other(message))
    }
}

fn keys_in(body: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = body;
    while let Some((_, after)) = rest.split_once("<Key>") {
        match after.split_once("</Key>") {
            Some((key, remaining)) => {
                keys.push(key.to_owned());
                rest = remaining;
            }
            None => break,
        }
    }
    keys
}

fn next_token(body: &str) -> Option<String> {
    if !body.contains("<IsTruncated>true</IsTruncated>") {
        return None;
    }
    let (_, after) = body.split_once("<NextContinuationToken>")?;
    let (token, _) = after.split_once("</NextContinuationToken>")?;
    Some(token.to_owned())
}
