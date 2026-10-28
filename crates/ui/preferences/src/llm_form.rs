use margin_ask::LlmConfig;

use crate::field::FieldError;

pub(crate) fn llm_setting(config: LlmConfig) -> Result<Option<LlmConfig>, FieldError> {
    if config.model.is_empty() {
        return Ok(None);
    }
    match config.validate_endpoint() {
        Ok(()) => Ok(Some(config)),
        Err(_) => Err(FieldError::NotHttpUrl),
    }
}

#[cfg(test)]
mod tests {
    use margin_ask::{ApiFormat, Effort};

    use super::*;

    fn config(base_url: &str, model: &str) -> LlmConfig {
        LlmConfig {
            api_format: ApiFormat::Anthropic,
            base_url: base_url.to_owned(),
            api_key: String::new(),
            model: model.to_owned(),
            effort: Some(Effort::High),
        }
    }

    #[test]
    fn no_model_means_ai_is_off() {
        assert_eq!(llm_setting(config("https://api.test", "")), Ok(None));
    }

    #[test]
    fn an_empty_base_url_uses_the_provider_default() {
        let llm = config("", "claude");
        assert_eq!(llm_setting(llm.clone()), Ok(Some(llm)));
    }

    #[test]
    fn the_base_url_must_be_http() {
        assert_eq!(
            llm_setting(config("file:///etc", "claude")),
            Err(FieldError::NotHttpUrl)
        );
    }
}
