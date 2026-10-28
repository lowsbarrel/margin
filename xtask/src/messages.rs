use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde_json::{Map, Value};

use crate::findings::Findings;
use crate::repository::{self, Repository};

const BASE_LOCALE: &str = "messages/en.json";
const SCHEMA_KEY: &str = "$schema";

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    let Some(base) = load(repository, BASE_LOCALE, findings) else {
        findings.add(BASE_LOCALE, "the English messages are missing");
        return;
    };
    let key_pattern = Regex::new("^[a-z][a-z0-9_]*$").expect("the key pattern is valid");
    let placeholder =
        Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}").expect("the placeholder pattern is valid");
    for key in base.keys() {
        if !key_pattern.is_match(key) {
            findings.add(BASE_LOCALE, format!("key `{key}` is not snake_case"));
        }
    }
    let locales = repository.files().filter(|file| {
        file.starts_with("messages/")
            && repository::has_extension(file, "json")
            && *file != BASE_LOCALE
    });
    for locale in locales {
        let Some(messages) = load(repository, locale, findings) else {
            continue;
        };
        compare(locale, &base, &messages, &placeholder, findings);
    }
}

fn compare(
    locale: &str,
    base: &BTreeMap<String, String>,
    messages: &BTreeMap<String, String>,
    placeholder: &Regex,
    findings: &mut Findings,
) {
    for (key, english) in base {
        let Some(translated) = messages.get(key) else {
            findings.add(locale, format!("missing `{key}`"));
            continue;
        };
        let expected = placeholders(placeholder, english);
        let found = placeholders(placeholder, translated);
        if expected != found {
            findings.add(
                locale,
                format!("`{key}` has placeholders {found:?}, English has {expected:?}"),
            );
        }
    }
    for key in messages.keys().filter(|key| !base.contains_key(*key)) {
        findings.add(
            locale,
            format!("`{key}` is not in English: remove it or add it to en.json"),
        );
    }
}

fn placeholders<'text>(pattern: &Regex, text: &'text str) -> BTreeSet<&'text str> {
    pattern
        .captures_iter(text)
        .filter_map(|captures| captures.get(1))
        .map(|name| name.as_str())
        .collect()
}

fn load(
    repository: &Repository,
    file: &str,
    findings: &mut Findings,
) -> Option<BTreeMap<String, String>> {
    let text = repository.read(file)?;
    let object: Map<String, Value> = match serde_json::from_str(&text) {
        Ok(object) => object,
        Err(error) => {
            findings.add(file, format!("invalid JSON object: {error}"));
            return None;
        }
    };
    let mut messages = BTreeMap::new();
    for (key, value) in object {
        if key == SCHEMA_KEY {
            continue;
        }
        match value {
            Value::String(text) if !text.trim().is_empty() => {
                messages.insert(key, text);
            }
            _ => findings.add(file, format!("`{key}` must be a non-empty string")),
        }
    }
    Some(messages)
}
