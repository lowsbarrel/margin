use margin_markdown::{Span, TextEdit};

#[must_use]
pub(crate) fn sorted_and_validated(text: &str, mut edits: Vec<TextEdit>) -> Vec<TextEdit> {
    edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
    let mut previous_end = 0;
    for edit in &edits {
        let Span { start, end } = edit.span;
        assert!(
            start <= end && end <= text.len(),
            "a change edit must lie inside the text"
        );
        assert!(
            text.is_char_boundary(start) && text.is_char_boundary(end),
            "a change edit must not split a character"
        );
        assert!(start >= previous_end, "change edits must not overlap");
        previous_end = end;
    }
    edits
}

#[must_use]
pub(crate) fn composed(text: &str, edits: Vec<TextEdit>) -> Option<TextEdit> {
    if edits.len() <= 1 {
        return edits.into_iter().next();
    }
    let start = edits.first().map(|edit| edit.span.start)?;
    let end = edits.last().map(|edit| edit.span.end)?;
    let removed: usize = edits.iter().map(|edit| edit.span.len()).sum();
    let inserted: usize = edits.iter().map(|edit| edit.insert.len()).sum();
    let mut insert = String::with_capacity(end - start - removed + inserted);
    let mut cursor = start;
    for edit in &edits {
        insert.push_str(&text[cursor..edit.span.start]);
        insert.push_str(&edit.insert);
        cursor = edit.span.end;
    }
    Some(TextEdit::new(Span::new(start, end), insert))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: usize, end: usize, insert: &str) -> TextEdit {
        TextEdit::new(Span::new(start, end), insert)
    }

    #[test]
    fn edits_are_sorted_by_position() {
        let sorted = sorted_and_validated("abcdef", vec![edit(4, 5, "x"), edit(0, 1, "y")]);
        assert_eq!(sorted, [edit(0, 1, "y"), edit(4, 5, "x")]);
    }

    #[test]
    fn several_edits_compose_into_one_region() {
        let text = "one two three";
        let edits = vec![edit(0, 3, "1"), edit(4, 7, "2"), edit(13, 13, "!")];
        let single = composed(text, edits.clone()).expect("edits are not empty");
        assert_eq!(single, edit(0, 13, "1 2 three!"));
        let mut expected = text.to_owned();
        for change in edits.iter().rev() {
            expected = change.applied(&expected);
        }
        assert_eq!(single.applied(text), expected);
    }

    #[test]
    fn no_edits_compose_to_nothing() {
        assert_eq!(composed("abc", Vec::new()), None);
    }

    #[test]
    #[should_panic(expected = "change edits must not overlap")]
    fn overlapping_edits_are_a_bug() {
        drop(sorted_and_validated(
            "abcdef",
            vec![edit(0, 3, ""), edit(2, 4, "")],
        ));
    }

    #[test]
    #[should_panic(expected = "a change edit must not split a character")]
    fn edits_inside_a_character_are_a_bug() {
        drop(sorted_and_validated("é", vec![edit(1, 2, "")]));
    }

    #[test]
    #[should_panic(expected = "a change edit must lie inside the text")]
    fn edits_past_the_end_are_a_bug() {
        drop(sorted_and_validated("abc", vec![edit(2, 9, "")]));
    }
}
