use std::collections::HashMap;

use crate::locale::Locale;

const ENGLISH: &str = include_str!("../../../../messages/en.json");
const ITALIAN: &str = include_str!("../../../../messages/it.json");
const SCHEMA_KEY: &str = "$schema";

#[derive(Debug)]
pub(crate) struct Catalog {
    messages: HashMap<String, String>,
}

impl Catalog {
    pub(crate) fn load(locale: Locale) -> Self {
        let source = match locale {
            Locale::English => ENGLISH,
            Locale::Italian => ITALIAN,
        };
        let mut messages: HashMap<String, String> = serde_json::from_str(source)
            .expect("messages are checked by cargo xtask check-messages");
        messages.remove(SCHEMA_KEY);
        Self { messages }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_loads_without_the_schema_key() {
        for locale in Locale::ALL {
            let catalog = Catalog::load(locale);
            assert!(catalog.get(SCHEMA_KEY).is_none());
            assert!(
                catalog.get("app_name").is_some(),
                "{locale:?} lacks app_name"
            );
        }
    }
}
