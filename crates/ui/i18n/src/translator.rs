use gpui_kit::{App, Global, SharedString};

use crate::catalog::Catalog;
use crate::locale::Locale;

#[derive(Debug)]
pub struct Translator {
    locale: Locale,
    catalog: Catalog,
    fallback: Catalog,
}

impl Global for Translator {}

impl Translator {
    #[must_use]
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            catalog: Catalog::load(locale),
            fallback: Catalog::load(Locale::English),
        }
    }

    pub fn init(locale: Option<Locale>, cx: &mut App) {
        cx.set_global(Self::new(locale.unwrap_or_else(Locale::from_system)));
    }

    pub fn set_locale(locale: Locale, cx: &mut App) {
        cx.set_global(Self::new(locale));
        cx.refresh_windows();
    }

    #[must_use]
    pub fn locale(cx: &App) -> Locale {
        cx.global::<Self>().locale
    }

    #[must_use]
    pub fn message<'catalog>(&'catalog self, key: &'catalog str) -> &'catalog str {
        self.catalog
            .get(key)
            .or_else(|| self.fallback.get(key))
            .unwrap_or(key)
    }

    #[must_use]
    pub fn format(&self, key: &str, arguments: &[(&str, &str)]) -> String {
        let mut text = self.message(key).to_owned();
        for (name, value) in arguments {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    #[must_use]
    pub fn format_count(&self, key: &str, count: u64, arguments: &[(&str, &str)]) -> String {
        let form = if self.locale.is_singular(count) {
            "one"
        } else {
            "other"
        };
        let count_text = count.to_string();
        let mut all_arguments = vec![("count", count_text.as_str())];
        all_arguments.extend_from_slice(arguments);
        self.format(&format!("{key}_{form}"), &all_arguments)
    }
}

#[must_use]
pub fn t(cx: &App, key: &str) -> SharedString {
    SharedString::from(cx.global::<Translator>().message(key).to_owned())
}

#[must_use]
pub fn t_with(cx: &App, key: &str, arguments: &[(&str, &str)]) -> SharedString {
    SharedString::from(cx.global::<Translator>().format(key, arguments))
}

#[must_use]
pub fn t_count(cx: &App, key: &str, count: u64, arguments: &[(&str, &str)]) -> SharedString {
    SharedString::from(
        cx.global::<Translator>()
            .format_count(key, count, arguments),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english_then_to_the_key() {
        let translator = Translator::new(Locale::Italian);
        assert_eq!(translator.message("trash_title"), "Cestino");
        assert_eq!(translator.message("no_such_key"), "no_such_key");
    }

    #[test]
    fn substitutes_named_placeholders() {
        let translator = Translator::new(Locale::English);
        assert_eq!(
            translator.format("toast_sync_failed", &[("error", "timeout")]),
            "Sync failed: timeout"
        );
    }
}
