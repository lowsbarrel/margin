#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Query {
    #[default]
    Empty,
    Search(String),
    Tags {
        filter: String,
    },
    Ask(String),
}

impl Query {
    pub(crate) fn parse(raw: &str) -> Self {
        let leading = raw.trim_start();
        if let Some(rest) = leading.strip_prefix('#') {
            return Self::Tags {
                filter: rest.trim().to_lowercase(),
            };
        }
        if let Some(rest) = leading.strip_prefix('?') {
            return Self::Ask(rest.trim().to_owned());
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Self::Empty
        } else {
            Self::Search(trimmed.to_owned())
        }
    }

    pub(crate) fn search_text(&self) -> Option<&str> {
        match self {
            Self::Search(text) => Some(text),
            Self::Empty | Self::Tags { .. } | Self::Ask(_) => None,
        }
    }

    pub(crate) const fn is_tags(&self) -> bool {
        matches!(self, Self::Tags { .. })
    }

    pub(crate) const fn is_ask(&self) -> bool {
        matches!(self, Self::Ask(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_input_is_empty() {
        assert_eq!(Query::parse(""), Query::Empty);
        assert_eq!(Query::parse("   "), Query::Empty);
    }

    #[test]
    fn plain_text_searches_trimmed() {
        assert_eq!(
            Query::parse("  road map "),
            Query::Search("road map".to_owned())
        );
    }

    #[test]
    fn hash_prefix_browses_tags_with_a_lowercase_filter() {
        assert_eq!(
            Query::parse(" #Work "),
            Query::Tags {
                filter: "work".to_owned()
            }
        );
        assert_eq!(
            Query::parse("#"),
            Query::Tags {
                filter: String::new()
            }
        );
    }

    #[test]
    fn question_prefix_asks() {
        assert_eq!(
            Query::parse("? what did I decide "),
            Query::Ask("what did I decide".to_owned())
        );
        assert_eq!(Query::parse("?"), Query::Ask(String::new()));
    }

    #[test]
    fn prefixes_only_count_at_the_start() {
        assert_eq!(
            Query::parse("notes #tag"),
            Query::Search("notes #tag".to_owned())
        );
        assert_eq!(Query::parse("why?"), Query::Search("why?".to_owned()));
    }
}
