use margin_markdown::Span;

pub const MATCH_LIMIT: usize = 5000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaseSensitivity {
    #[default]
    Insensitive,
    Sensitive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindQuery {
    pub text: String,
    pub case: CaseSensitivity,
}

#[must_use]
pub fn find_matches(text: &str, query: &FindQuery) -> Vec<Span> {
    matches(text, query, MATCH_LIMIT)
}

pub(crate) fn matches(text: &str, query: &FindQuery, limit: usize) -> Vec<Span> {
    if query.text.is_empty() {
        return Vec::new();
    }
    match query.case {
        CaseSensitivity::Sensitive => text
            .match_indices(query.text.as_str())
            .take(limit)
            .map(|(start, found)| Span::new(start, start + found.len()))
            .collect(),
        CaseSensitivity::Insensitive => folded_matches(text, &query.text, limit),
    }
}

fn folded_matches(text: &str, needle: &str, limit: usize) -> Vec<Span> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    let mut found = Vec::new();
    let mut start = 0;
    while found.len() < limit
        && let Some(rest) = text.get(start..)
        && let Some(first) = rest.chars().next()
    {
        match folded_len(rest, &needle) {
            Some(len) => {
                found.push(Span::new(start, start + len));
                start += len;
            }
            None => start += first.len_utf8(),
        }
    }
    found
}

fn folded_len(rest: &str, needle: &[char]) -> Option<usize> {
    let mut index = 0;
    for (offset, character) in rest.char_indices() {
        for lower in character.to_lowercase() {
            if needle.get(index) != Some(&lower) {
                return None;
            }
            index += 1;
        }
        if index == needle.len() {
            return Some(offset + character.len_utf8());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str, case: CaseSensitivity) -> FindQuery {
        FindQuery {
            text: text.to_owned(),
            case,
        }
    }

    fn spans(found: &[Span]) -> Vec<(usize, usize)> {
        found.iter().map(|span| (span.start, span.end)).collect()
    }

    #[test]
    fn empty_query_matches_nothing() {
        assert!(find_matches("abc", &query("", CaseSensitivity::Insensitive)).is_empty());
    }

    #[test]
    fn insensitive_ignores_case() {
        let found = find_matches("Foo foo FOO", &query("foo", CaseSensitivity::Insensitive));
        assert_eq!(spans(&found), [(0, 3), (4, 7), (8, 11)]);
    }

    #[test]
    fn sensitive_respects_case() {
        let found = find_matches("Foo foo FOO", &query("foo", CaseSensitivity::Sensitive));
        assert_eq!(spans(&found), [(4, 7)]);
    }

    #[test]
    fn matches_do_not_overlap() {
        let found = find_matches("aaaa", &query("aa", CaseSensitivity::Insensitive));
        assert_eq!(spans(&found), [(0, 2), (2, 4)]);
    }

    #[test]
    fn insensitive_handles_multibyte_text() {
        let found = find_matches("Èra èRA", &query("ÈRA", CaseSensitivity::Insensitive));
        assert_eq!(spans(&found), [(0, 4), (5, 9)]);
    }

    #[test]
    fn query_is_literal() {
        let found = find_matches("a.b axb", &query("a.b", CaseSensitivity::Sensitive));
        assert_eq!(spans(&found), [(0, 3)]);
    }

    #[test]
    fn counting_stops_at_the_limit() {
        let text = "a".repeat(MATCH_LIMIT + 10);
        let found = find_matches(&text, &query("a", CaseSensitivity::Insensitive));
        assert_eq!(found.len(), MATCH_LIMIT);
    }
}
