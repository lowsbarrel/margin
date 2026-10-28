use margin_markdown::{Document, Span};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, RangeEdit};
use crate::markup_context::{self, Context};
use crate::text::{self, Line};

fn replace_marker(
    document: &Document,
    inner: &Context<'_>,
    line: &Line<'_>,
    start: usize,
) -> Option<RangeEdit> {
    let item = inner.item?;
    let list = inner.list?;
    let first_item =
        markup_context::node_start(document, list) >= markup_context::node_start(document, item);
    let marker = text::slice(line.text, inner.from, inner.to);
    if first_item || marker.trim().is_empty() {
        return None;
    }
    let width = text::count_column(line.text, inner.to) - text::count_column(line.text, inner.from);
    let mut insert = inner.blank(Some(width), true);
    if start == line.start {
        insert = markup_context::normalize_indent(document, &insert);
    }
    let cursor = start + insert.len();
    Some(RangeEdit::replace(
        Span::new(start, line.start + inner.to),
        insert,
        cursor,
    ))
}

fn delete_range(document: &Document, range: Selection) -> Option<RangeEdit> {
    if !range.is_empty() {
        return None;
    }
    let pos = range.head;
    let line = Line::at(document, pos);
    let column = pos - line.start;
    let contexts = markup_context::contexts(document, pos);
    let inner = contexts.last()?;
    let space_end = inner.to - inner.space_after.len() + usize::from(!inner.space_after.is_empty());
    if column > space_end && text::slice(line.text, space_end, column).trim().is_empty() {
        let at = line.start + space_end;
        return Some(RangeEdit::replace(Span::new(at, pos), "", at));
    }
    if column != space_end {
        return None;
    }
    let on_item_line = inner
        .item
        .is_none_or(|item| line.start <= markup_context::node_start(document, item));
    if !on_item_line && !text::slice(line.text, 0, inner.to).trim().is_empty() {
        return None;
    }
    let start = line.start + inner.from;
    if let Some(replaced) = replace_marker(document, inner, &line, start) {
        return Some(replaced);
    }
    (start < pos).then(|| RangeEdit::replace(Span::new(start, pos), "", start))
}

#[must_use]
pub fn delete_markup_backward(document: &Document, selections: &[Selection]) -> Option<Change> {
    mapping::by_range(selections, |range| delete_range(document, range))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    fn backspace(source: &str) -> Option<String> {
        run(source, delete_markup_backward)
    }

    #[test]
    fn removes_the_marker_of_a_first_item() {
        assert_eq!(backspace("- |").as_deref(), Some("|"));
        assert_eq!(backspace("> |").as_deref(), Some("|"));
        assert_eq!(backspace("1. |").as_deref(), Some("|"));
    }

    #[test]
    fn turns_a_later_marker_into_continuation_space() {
        assert_eq!(backspace("- a\n- |").as_deref(), Some("- a\n  |"));
    }

    #[test]
    fn trims_extra_space_after_the_marker() {
        assert_eq!(backspace("-   |").as_deref(), Some("- |"));
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        assert_eq!(backspace("- a|"), None);
        assert_eq!(backspace("plain|"), None);
    }
}
