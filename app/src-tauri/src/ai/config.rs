use serde::{Deserialize, Serialize};
use std::fmt;

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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
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
    #[serde(default)]
    pub effort: Option<Effort>,
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("api_format", &self.api_format)
            .field("base_url", &self.base_url)
            // AppSettings derives Debug, so the key must be redacted or {:?} prints it in plaintext.
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("effort", &self.effort)
            .finish()
    }
}

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

pub fn validate(config: &LlmConfig) -> Result<String, String> {
    if config.model.trim().is_empty() {
        return Err("Model must not be empty".into());
    }
    validate_endpoint(config)
}

pub fn validate_endpoint(config: &LlmConfig) -> Result<String, String> {
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
            effort: None,
        }
    }

    #[test]
    fn effort_is_optional_so_an_older_settings_file_still_loads() {
        let without = serde_json::json!({
            "api_format": "openai",
            "base_url": "https://x.test/v1",
            "api_key": "sk",
            "model": "m",
        });
        let parsed: LlmConfig = serde_json::from_value(without).unwrap();
        assert_eq!(parsed.effort, None);

        let with = serde_json::json!({
            "api_format": "anthropic",
            "model": "m",
            "effort": "high",
        });
        let parsed: LlmConfig = serde_json::from_value(with).unwrap();
        assert_eq!(parsed.effort, Some(Effort::High));
        assert_eq!(serde_json::to_value(parsed.effort).unwrap(), "high");
    }

    #[test]
    fn openai_base_keeps_the_version_path_it_was_given() {
        assert_eq!(
            normalize_base_url(ApiFormat::Openai, "https://api.openai.com/v1/"),
            "https://api.openai.com/v1"
        );
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
    fn listing_models_needs_an_endpoint_but_no_model_yet() {
        let mut unpicked = config(ApiFormat::Openai, "https://openrouter.ai/api/v1");
        unpicked.model = String::new();
        assert_eq!(
            validate_endpoint(&unpicked).as_deref(),
            Ok("https://openrouter.ai/api/v1")
        );
        unpicked.base_url = "file:///etc".into();
        assert!(validate_endpoint(&unpicked).is_err());
    }

    #[test]
    fn debug_never_prints_the_key() {
        let mut c = config(ApiFormat::Openai, "https://x.test/v1");
        c.api_key = "sk-secret".into();
        let shown = format!("{c:?}");
        assert!(!shown.contains("sk-secret"), "key leaked: {shown}");
    }
}
