use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Locale {
    #[default]
    English,
    Italian,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::English, Self::Italian];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Italian => "it",
        }
    }

    #[must_use]
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Italian => "Italiano",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let language = tag.split(['-', '_', '.']).next().unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|locale| locale.code().eq_ignore_ascii_case(language))
    }

    #[must_use]
    pub fn from_system() -> Self {
        sys_locale::get_locales()
            .find_map(|tag| Self::from_tag(&tag))
            .unwrap_or_default()
    }

    #[must_use]
    pub const fn is_singular(self, count: u64) -> bool {
        match self {
            Self::English | Self::Italian => count == 1,
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.native_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_language_tags_by_their_language() {
        assert_eq!(Locale::from_tag("it-IT"), Some(Locale::Italian));
        assert_eq!(Locale::from_tag("it_CH.UTF-8"), Some(Locale::Italian));
        assert_eq!(Locale::from_tag("EN"), Some(Locale::English));
        assert_eq!(Locale::from_tag("de-DE"), None);
        assert_eq!(Locale::from_tag(""), None);
    }

    #[test]
    fn plural_rules_select_one_only_for_exactly_one() {
        for locale in Locale::ALL {
            assert!(locale.is_singular(1));
            assert!(!locale.is_singular(0));
            assert!(!locale.is_singular(2));
        }
    }
}
