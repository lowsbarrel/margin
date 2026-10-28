use margin_ask::{ApiFormat, Effort, Error, LlmConfig, normalize_base_url};
use serde_json::json;

fn config(format: ApiFormat, base: &str) -> LlmConfig {
    LlmConfig {
        api_format: format,
        base_url: base.to_owned(),
        api_key: String::new(),
        model: "m".to_owned(),
        effort: None,
    }
}

#[test]
fn a_config_round_trips_through_the_settings_json() {
    let stored = json!({
        "api_format": "anthropic",
        "base_url": "https://api.anthropic.com/v1",
        "api_key": "sk-secret",
        "model": "claude",
        "effort": "high",
    });
    let parsed: LlmConfig =
        serde_json::from_value(stored.clone()).expect("the stored settings load");
    assert_eq!(parsed.api_format, ApiFormat::Anthropic);
    assert_eq!(parsed.model, "claude");
    assert_eq!(parsed.effort, Some(Effort::High));
    assert_eq!(
        serde_json::to_value(&parsed).expect("a config serializes"),
        stored
    );
}

#[test]
fn effort_is_optional_so_an_older_settings_file_still_loads() {
    let without = json!({
        "api_format": "openai",
        "base_url": "https://x.test/v1",
        "api_key": "sk",
        "model": "m",
    });
    let parsed: LlmConfig = serde_json::from_value(without).expect("an older settings file loads");
    assert_eq!(parsed.effort, None);

    let with = json!({
        "api_format": "anthropic",
        "model": "m",
        "effort": "high",
    });
    let parsed: LlmConfig = serde_json::from_value(with).expect("a minimal settings file loads");
    assert_eq!(parsed.effort, Some(Effort::High));
    assert_eq!(
        serde_json::to_value(parsed.effort).expect("the effort serializes"),
        "high"
    );
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
    let reachable = config(ApiFormat::Openai, "https://x.test/v1");
    assert_eq!(reachable.validate(), Ok(()));
    let unencrypted = config(ApiFormat::Openai, "file:///etc");
    assert_eq!(unencrypted.validate(), Err(Error::UnsupportedScheme));
    let schemeless = config(ApiFormat::Openai, "x.test/v1");
    assert_eq!(schemeless.validate(), Err(Error::UnsupportedScheme));
    let mut empty_model = config(ApiFormat::Openai, "https://x.test/v1");
    empty_model.model = "  ".to_owned();
    assert_eq!(empty_model.validate(), Err(Error::EmptyModel));
}

#[test]
fn listing_models_needs_an_endpoint_but_no_model_yet() {
    let mut unpicked = config(ApiFormat::Openai, "https://openrouter.ai/api/v1");
    unpicked.model = String::new();
    assert_eq!(unpicked.validate_endpoint(), Ok(()));
    unpicked.base_url = "file:///etc".to_owned();
    assert_eq!(unpicked.validate_endpoint(), Err(Error::UnsupportedScheme));
}

#[test]
fn debug_never_prints_the_key() {
    let mut config = config(ApiFormat::Openai, "https://x.test/v1");
    config.api_key = "sk-secret".to_owned();
    let shown = format!("{config:?}");
    assert!(!shown.contains("sk-secret"), "key leaked: {shown}");
}
