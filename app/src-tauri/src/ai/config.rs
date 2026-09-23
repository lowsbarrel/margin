//! Which provider answers `?` questions, and where.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The provider's wire protocol. Every OpenAI-compatible and Anthropic-compatible
/// endpoint speaks one of these two, so the format — not the vendor — picks the
/// request shape, the auth header and the SSE event vocabulary.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ApiFormat {
    Openai,
    Anthropic,
}

impl ApiFormat {
    fn default_base_url(self) -> &'static str {
        match self {
            ApiFormat::Openai => "https://api.openai.com/v1",
            ApiFormat::Anthropic => "https://api.anthropic.com",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct LlmConfig {
    pub api_format: ApiFormat,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
}

/// Hand-written Debug that redacts the key, mirroring [`crate::s3::S3Config`]:
/// `AppSettings` derives Debug, so without this any `{:?}` of the settings would
/// print the API key in plaintext.
impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("api_format", &self.api_format)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .finish()
    }
}

/// Canonical base for both the models list and the chat endpoint.
///
/// Trailing slashes are dropped so `{base}/chat/completions` never doubles up;
/// an Anthropic base gets `/v1` appended because the Messages API lives under it,
/// while a base that already spells `/v1` is left alone (both
/// `https://api.anthropic.com` and `https://api.anthropic.com/v1` are what people
/// paste). An empty value falls back to the format's default host.
pub fn normalize_base_url(format: ApiFormat, raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let base = if trimmed.is_empty() {
        format.default_base_url()
    } else {
        trimmed
    };
    match format {
        ApiFormat::Openai => base.to_string(),
        ApiFormat::Anthropic => {
            if base.ends_with("/v1") {
                base.to_string()
            } else {
                format!("{base}/v1")
            }
        }
    }
}

/// Reject a config the user could only have typed by hand wrongly. Returns the
/// normalized base URL so callers never re-derive it.
pub fn validate(config: &LlmConfig) -> Result<String, String> {
    if config.model.trim().is_empty() {
        return Err("Model must not be empty".into());
    }
    let base = normalize_base_url(config.api_format, &config.base_url);
    let parsed = reqwest::Url::parse(&base).map_err(|e| format!("Invalid base URL: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Base URL must be http or https".into());
    }
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(format: ApiFormat, base: &str) -> LlmConfig {
        LlmConfig {
            api_format: format,
            base_url: base.to_string(),
            api_key: String::new(),
            model: "m".to_string(),
        }
    }

    #[test]
    fn openai_base_keeps_the_version_path_it_was_given() {
        assert_eq!(
            normalize_base_url(ApiFormat::Openai, "https://api.openai.com/v1/"),
            "https://api.openai.com/v1"
        );
        // A local server whose OpenAI-compatible routes live under /v1.
        assert_eq!(
            normalize_base_url(ApiFormat::Openai, " http://localhost:11434/v1 "),
            "http://localhost:11434/v1"
        );
    }

    #[test]
    fn anthropic_base_gains_v1_exactly_once() {
        assert_eq!(
            normalize_base_url(ApiFormat::Anthropic, "https://api.anthropic.com"),
            "https://api.anthropic.com/v1"
        );
        assert_eq!(
            normalize_base_url(ApiFormat::Anthropic, "https://api.anthropic.com/v1/"),
            "https://api.anthropic.com/v1"
        );
    }

    #[test]
    fn empty_base_falls_back_to_the_provider_default() {
        assert_eq!(
            normalize_base_url(ApiFormat::Openai, "   "),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base_url(ApiFormat::Anthropic, ""),
            "https://api.anthropic.com/v1"
        );
    }

    #[test]
    fn validate_rejects_an_empty_model_or_a_non_http_scheme() {
        assert!(validate(&config(ApiFormat::Openai, "https://x.test/v1")).is_ok());
        assert!(validate(&config(ApiFormat::Openai, "file:///etc")).is_err());
        let mut empty_model = config(ApiFormat::Openai, "https://x.test/v1");
        empty_model.model = "  ".into();
        assert!(validate(&empty_model).is_err());
    }

    #[test]
    fn debug_never_prints_the_key() {
        let mut c = config(ApiFormat::Openai, "https://x.test/v1");
        c.api_key = "sk-secret".into();
        let shown = format!("{c:?}");
        assert!(!shown.contains("sk-secret"), "key leaked: {shown}");
    }
}
