use std::cmp::Reverse;
use std::ops::Range;

pub(crate) fn match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let mut terms: Vec<&str> = query.split_whitespace().collect();
    terms.sort_by_key(|term| Reverse(term.chars().count()));
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let rest = &text[start..];
        match terms.iter().find_map(|term| matched_len(rest, term)) {
            Some(length) => {
                ranges.push(start..start + length);
                start += length;
            }
            None => start += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    ranges
}

fn matched_len(text: &str, term: &str) -> Option<usize> {
    let mut length = 0;
    let mut characters = text.chars();
    for wanted in term.chars() {
        let found = characters.next()?;
        if !found.to_lowercase().eq(wanted.to_lowercase()) {
            return None;
        }
        length += found.len_utf8();
    }
    Some(length)
}

pub(crate) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches<'text>(text: &'text str, query: &str) -> Vec<&'text str> {
        match_ranges(text, query)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn marks_every_term_case_insensitively() {
        assert_eq!(
            matches("Plan the Roadmap, then plan again", "plan roadmap"),
            ["Plan", "Roadmap", "plan"]
        );
    }

    #[test]
    fn prefers_the_longest_term() {
        assert_eq!(matches("planning", "plan planning"), ["planning"]);
    }

    #[test]
    fn handles_multibyte_text() {
        assert_eq!(matches("…Città e città", "CITTÀ"), ["Città", "città"]);
    }

    #[test]
    fn blank_query_marks_nothing() {
        assert!(match_ranges("anything", "  ").is_empty());
    }

    #[test]
    fn snippets_collapse_to_one_line() {
        assert_eq!(
            one_line("# Ideas\n\n#work  #ideas\tA "),
            "# Ideas #work #ideas A"
        );
    }
}
