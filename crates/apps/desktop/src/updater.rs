use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use gpui_kit::App;
use margin_kernel::{BoxFuture, StoreError};
use margin_workbench::{AvailableUpdate, UpdateChannel};
use minisign_verify::{PublicKey, Signature};
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use tokio::task;

use crate::update_install;

const ENDPOINT: &str = "https://github.com/lowsbarrel/margin/releases/latest/download/latest.json";
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM1Qzk0MEFFM0FCNEMyQkUKUldTK3dyUTZya0RKeFdNN09QL05wMDk0VFZnOHVONit6S1NlR0doRmZLRlNDME9OK3krOE0yMTgK";
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    platforms: HashMap<String, Artifact>,
}

#[derive(Clone, Debug, Deserialize)]
struct Artifact {
    url: String,
    signature: String,
}

impl Manifest {
    fn parse(bytes: &[u8]) -> Result<Self, StoreError> {
        serde_json::from_slice(bytes)
            .map_err(|error| StoreError::InvalidData(format!("update manifest: {error}")))
    }

    fn update_for(
        &self,
        current: &str,
        platform: &str,
    ) -> Result<Option<(AvailableUpdate, Artifact)>, StoreError> {
        let Some(artifact) = self.platforms.get(platform) else {
            return Ok(None);
        };
        if parse_version(&self.version)? <= parse_version(current)? {
            return Ok(None);
        }
        let update = AvailableUpdate {
            version: self.version.trim_start_matches('v').to_owned(),
            notes: self.notes.clone().filter(|notes| !notes.trim().is_empty()),
        };
        Ok(Some((update, artifact.clone())))
    }
}

fn parse_version(version: &str) -> Result<Version, StoreError> {
    Version::parse(version.trim().trim_start_matches('v'))
        .map_err(|error| StoreError::InvalidData(format!("version {version}: {error}")))
}

fn platform_key(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "macos" => "darwin",
        "linux" | "windows" => os,
        _ => return None,
    };
    matches!(arch, "x86_64" | "aarch64").then(|| format!("{os}-{arch}"))
}

fn invalid_signature(error: impl Display) -> StoreError {
    StoreError::InvalidData(format!("update signature: {error}"))
}

fn verify(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), StoreError> {
    let decode = |encoded: &str| {
        STANDARD
            .decode(encoded.trim())
            .map_err(invalid_signature)
            .and_then(|raw| String::from_utf8(raw).map_err(invalid_signature))
    };
    let public_key = PublicKey::decode(&decode(public_key)?).map_err(invalid_signature)?;
    let signature = Signature::decode(&decode(signature)?).map_err(invalid_signature)?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(invalid_signature)
}

fn unavailable(error: &reqwest::Error) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

#[derive(Clone, Debug)]
pub(crate) struct GithubUpdater {
    client: Client,
}

impl GithubUpdater {
    async fn latest(client: &Client) -> Result<Option<(AvailableUpdate, Artifact)>, StoreError> {
        let Some(platform) = platform_key(env::consts::OS, env::consts::ARCH) else {
            return Ok(None);
        };
        let response = client
            .get(ENDPOINT)
            .timeout(CHECK_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| unavailable(&error))?;
        let body = response
            .bytes()
            .await
            .map_err(|error| unavailable(&error))?;
        Manifest::parse(&body)?.update_for(env!("CARGO_PKG_VERSION"), &platform)
    }
}

impl UpdateChannel for GithubUpdater {
    fn check(&self) -> BoxFuture<'static, Result<Option<AvailableUpdate>, StoreError>> {
        let client = self.client.clone();
        Box::pin(async move { Ok(Self::latest(&client).await?.map(|(update, _)| update)) })
    }

    fn install(&self, update: AvailableUpdate) -> BoxFuture<'static, Result<(), StoreError>> {
        let client = self.client.clone();
        Box::pin(async move {
            let Some((latest, artifact)) = Self::latest(&client).await? else {
                return Err(StoreError::NotFound(format!("update {}", update.version)));
            };
            let bytes = client
                .get(&artifact.url)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|error| unavailable(&error))?
                .bytes()
                .await
                .map_err(|error| unavailable(&error))?;
            verify(&bytes, &artifact.signature, PUBLIC_KEY)?;
            task::spawn_blocking(move || update_install::install(&bytes))
                .await
                .map_err(|error| StoreError::Other(error.to_string()))?
                .map_err(|error| StoreError::Other(format!("update {}: {error}", latest.version)))
        })
    }
}

pub(crate) fn updater() -> Arc<dyn UpdateChannel> {
    let client = Client::builder()
        .user_agent(concat!("Margin/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("the HTTP client builds with the bundled TLS backend");
    Arc::new(GithubUpdater { client })
}

pub(crate) fn relaunch(cx: &mut App) {
    if cfg!(target_os = "windows") {
        cx.quit();
        return;
    }
    if let Some(appimage) = env::var_os("APPIMAGE") {
        cx.set_restart_path(appimage.into());
    }
    cx.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDdBMzc3M0MwRkRGREYwQ0EKUldUSzhQMzl3SE0zZXNSMjJUNG1UcEEwaFJQTUxvTmdSRTJBdHUvd0RoTk54d05nMXdjK1ZUM1cK";
    const TEST_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIGNhcmdvLXBhY2thZ2VyIHNlY3JldCBrZXkKUlVUSzhQMzl3SE0zZXJWZnlWaGIyNzBSUkZoVXB1L21DUnhXcU9MWjZkckpIdVorZ2YrQkovNm16SmtmMmRUaDI1ejBmbC9KKytNVytnVHFZTDhlWWUrQ3M2OFpjL3pXYlE0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkzMTg4OTI0CWZpbGU6cGF5bG9hZC5iaW4KVDJYSzF2RnNRaGFIOFhFTXU2NURsRW1qWnVnb1NKOWZoK0dNNnZJYXRqb0FVc29pUDJKT2hmSldVbWN1Q01heHp3RUxkekg4Y0t2S0lBV003S3FlQVE9PQo=";
    const PAYLOAD: &[u8] = b"margin update payload";

    const MANIFEST: &str = r#"{
        "version": "v0.3.0",
        "notes": "Faster sync",
        "pub_date": "2026-10-01T10:00:00Z",
        "platforms": {
            "darwin-aarch64": { "signature": "c2ln", "url": "https://example.com/Margin_aarch64.app.tar.gz" },
            "windows-x86_64": { "signature": "c2ln", "url": "https://example.com/Margin_0.3.0_x64-setup.exe" }
        }
    }"#;

    #[test]
    fn platform_keys_follow_tauri_names() {
        assert_eq!(
            platform_key("macos", "aarch64").as_deref(),
            Some("darwin-aarch64")
        );
        assert_eq!(
            platform_key("macos", "x86_64").as_deref(),
            Some("darwin-x86_64")
        );
        assert_eq!(
            platform_key("linux", "aarch64").as_deref(),
            Some("linux-aarch64")
        );
        assert_eq!(
            platform_key("windows", "x86_64").as_deref(),
            Some("windows-x86_64")
        );
        assert_eq!(platform_key("freebsd", "x86_64"), None);
        assert_eq!(platform_key("linux", "riscv64"), None);
    }

    #[test]
    fn a_newer_release_for_this_platform_is_an_update() {
        let manifest = Manifest::parse(MANIFEST.as_bytes()).expect("the manifest parses");
        let (update, artifact) = manifest
            .update_for("0.2.0", "darwin-aarch64")
            .expect("versions parse")
            .expect("0.3.0 is newer than 0.2.0");
        assert_eq!(update.version, "0.3.0");
        assert_eq!(update.notes.as_deref(), Some("Faster sync"));
        assert_eq!(
            artifact.url,
            "https://example.com/Margin_aarch64.app.tar.gz"
        );
    }

    #[test]
    fn same_older_or_missing_platform_is_no_update() {
        let manifest = Manifest::parse(MANIFEST.as_bytes()).expect("the manifest parses");
        for (current, platform) in [
            ("0.3.0", "darwin-aarch64"),
            ("0.4.0-beta.1", "darwin-aarch64"),
            ("0.2.0", "linux-x86_64"),
        ] {
            let update = manifest
                .update_for(current, platform)
                .expect("versions parse");
            assert!(update.is_none(), "{current} on {platform}");
        }
    }

    #[test]
    fn prereleases_order_before_their_release() {
        let manifest = Manifest::parse(MANIFEST.as_bytes()).expect("the manifest parses");
        let update = manifest
            .update_for("0.3.0-rc.1", "windows-x86_64")
            .expect("versions parse");
        assert!(update.is_some());
    }

    #[test]
    fn malformed_manifests_and_versions_are_invalid_data() {
        assert!(matches!(
            Manifest::parse(b"{\"version\": 1}"),
            Err(StoreError::InvalidData(_))
        ));
        let manifest = Manifest::parse(MANIFEST.as_bytes()).expect("the manifest parses");
        assert!(matches!(
            manifest.update_for("not a version", "darwin-aarch64"),
            Err(StoreError::InvalidData(_))
        ));
    }

    #[test]
    fn signatures_verify_against_the_signing_key_only() {
        verify(PAYLOAD, TEST_SIGNATURE, TEST_PUBLIC_KEY)
            .expect("the payload is signed by the test key");
        assert!(verify(b"margin update payloaD", TEST_SIGNATURE, TEST_PUBLIC_KEY).is_err());
        assert!(verify(PAYLOAD, TEST_SIGNATURE, PUBLIC_KEY).is_err());
        assert!(verify(PAYLOAD, "not base64", TEST_PUBLIC_KEY).is_err());
    }

    #[test]
    fn the_release_public_key_decodes() {
        let text = String::from_utf8(STANDARD.decode(PUBLIC_KEY).expect("base64")).expect("utf-8");
        PublicKey::decode(&text).expect("the embedded key is a minisign public key");
    }
}
