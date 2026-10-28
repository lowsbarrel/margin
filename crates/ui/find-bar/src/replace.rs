use margin_markdown::{Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::navigation::next_match;
use crate::query::{FindQuery, matches};

#[must_use]
pub fn replace_current(
    text: &str,
    query: &FindQuery,
    selection: Selection,
    replacement: &str,
) -> Option<Change> {
    let found = matches(text, query, usize::MAX);
    let selected = selection.span();
    let target = if found.contains(&selected) {
        selected
    } else {
        *found.get(next_match(&found, selection)?)?
    };
    let following = *found.get(next_match(
        &found,
        Selection::new(target.start, target.end),
    )?)?;
    let inserted = target.start + replacement.len();
    let after = if following == target {
        Selection::new(inserted, target.start)
    } else if following.start >= target.end {
        let shift = |offset: usize| offset - target.end + inserted;
        Selection::new(shift(following.start), shift(following.end))
    } else {
        Selection::new(following.start, following.end)
    };
    Some(Change::new(vec![TextEdit::new(target, replacement)]).with_selections(vec![after]))
}

#[must_use]
pub fn replace_all(text: &str, query: &FindQuery, replacement: &str) -> Option<Change> {
    let edits: Vec<TextEdit> = matches(text, query, usize::MAX)
        .into_iter()
        .map(|span: Span| TextEdit::new(span, replacement))
        .collect();
    (!edits.is_empty()).then(|| Change::new(edits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{CaseSensitivity, MATCH_LIMIT};

    fn query(text: &str) -> FindQuery {
        FindQuery {
            text: text.to_owned(),
            case: CaseSensitivity::Insensitive,
        }
    }

    fn edits(change: &Change) -> Vec<(usize, usize, &str)> {
        change
            .edits
            .iter()
            .map(|edit| (edit.span.start, edit.span.end, edit.insert.as_str()))
            .collect()
    }

    #[test]
    fn replaces_the_selected_match_and_selects_the_next() {
        let change = replace_current("foo bar foo", &query("foo"), Selection::new(0, 3), "quux")
            .expect("a match is selected");
        assert_eq!(edits(&change), [(0, 3, "quux")]);
        assert_eq!(change.selections, Some(vec![Selection::new(9, 12)]));
    }

    #[test]
    fn replaces_the_next_match_when_the_selection_is_not_one() {
        let change = replace_current("foo bar foo", &query("FOO"), Selection::cursor(1), "x")
            .expect("a later match exists");
        assert_eq!(edits(&change), [(8, 11, "x")]);
        assert_eq!(change.selections, Some(vec![Selection::new(0, 3)]));
    }

    #[test]
    fn the_only_match_leaves_the_replacement_selected() {
        let change = replace_current("a foo", &query("foo"), Selection::new(2, 5), "bar")
            .expect("a match is selected");
        assert_eq!(edits(&change), [(2, 5, "bar")]);
        assert_eq!(change.selections, Some(vec![Selection::new(5, 2)]));
    }

    #[test]
    fn nothing_to_replace_is_none() {
        assert_eq!(
            replace_current("abc", &query("x"), Selection::cursor(0), "y"),
            None
        );
        assert_eq!(
            replace_current("abc", &query(""), Selection::cursor(0), "y"),
            None
        );
        assert_eq!(replace_all("abc", &query("x"), "y"), None);
    }

    #[test]
    fn replace_all_replaces_every_match_and_keeps_selections() {
        let change = replace_all("Foo foo", &query("foo"), "$1").expect("two matches");
        assert_eq!(edits(&change), [(0, 3, "$1"), (4, 7, "$1")]);
        assert_eq!(change.selections, None);
    }

    #[test]
    fn replace_all_is_not_capped() {
        let text = "a".repeat(MATCH_LIMIT + 1);
        let change = replace_all(&text, &query("a"), "b").expect("matches");
        assert_eq!(change.edits.len(), MATCH_LIMIT + 1);
    }
}
