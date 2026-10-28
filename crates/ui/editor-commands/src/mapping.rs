use margin_markdown::{Span, TextEdit};
use margin_text_buffer::{Change, Selection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Assoc {
    Before,
    After,
}

pub(crate) fn sorted(mut edits: Vec<TextEdit>) -> Vec<TextEdit> {
    edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
    edits
}

pub(crate) fn map_pos(edits: &[TextEdit], pos: usize, assoc: Assoc) -> usize {
    let mut delta = 0_isize;
    for edit in edits {
        let Span { start, end } = edit.span;
        let inserted = edit.insert.len();
        if pos < start {
            break;
        }
        if start == end {
            if pos == start && assoc == Assoc::Before {
                break;
            }
        } else if pos < end {
            let base = start.saturating_add_signed(delta);
            return if pos == start || assoc == Assoc::Before {
                base
            } else {
                base + inserted
            };
        }
        delta += signed(inserted) - signed(end - start);
    }
    pos.saturating_add_signed(delta)
}

pub(crate) fn map_selection(edits: &[TextEdit], selection: Selection, assoc: Assoc) -> Selection {
    Selection::new(
        map_pos(edits, selection.anchor, assoc),
        map_pos(edits, selection.head, assoc),
    )
}

pub(crate) fn map_range(edits: &[TextEdit], selection: Selection) -> Selection {
    if selection.is_empty() {
        return Selection::cursor(map_pos(edits, selection.head, Assoc::Before));
    }
    let start = map_pos(edits, selection.start(), Assoc::After);
    let end = map_pos(edits, selection.end(), Assoc::Before).max(start);
    if selection.anchor <= selection.head {
        Selection::new(start, end)
    } else {
        Selection::new(end, start)
    }
}

pub(crate) fn apply(text: &str, edits: &[TextEdit]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        out.push_str(text.get(at..edit.span.start).unwrap_or_default());
        out.push_str(&edit.insert);
        at = edit.span.end.max(at);
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}

pub(crate) fn diff(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let room = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(room)
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    Some(TextEdit::new(
        Span::new(prefix, old.len() - suffix),
        new.get(prefix..new.len() - suffix).unwrap_or_default(),
    ))
}

pub(crate) struct RangeEdit {
    pub edits: Vec<TextEdit>,
    pub selection: Selection,
}

impl RangeEdit {
    pub(crate) fn new(edits: Vec<TextEdit>, selection: Selection) -> Self {
        Self { edits, selection }
    }

    pub(crate) fn replace(span: Span, insert: impl Into<String>, cursor: usize) -> Self {
        Self::new(vec![TextEdit::new(span, insert)], Selection::cursor(cursor))
    }
}

pub(crate) fn by_range(
    selections: &[Selection],
    mut each: impl FnMut(Selection) -> Option<RangeEdit>,
) -> Option<Change> {
    let mut edits = Vec::new();
    let mut moved = Vec::with_capacity(selections.len());
    let mut delta = 0_isize;
    for selection in selections {
        let range = each(*selection)?;
        let own: isize = range
            .edits
            .iter()
            .map(|edit| signed(edit.insert.len()) - signed(edit.span.len()))
            .sum();
        moved.push(Selection::new(
            range.selection.anchor.saturating_add_signed(delta),
            range.selection.head.saturating_add_signed(delta),
        ));
        delta += own;
        edits.extend(range.edits);
    }
    Some(Change::new(sorted(edits)).with_selections(moved))
}

pub(crate) fn rewrite(
    text: &str,
    first: &[TextEdit],
    second: &[TextEdit],
    selections: Vec<Selection>,
) -> Change {
    let middle = apply(text, first);
    let last = apply(&middle, second);
    let edits = diff(text, &last).into_iter().collect();
    let selections = selections
        .into_iter()
        .map(|selection| map_selection(second, selection, Assoc::After))
        .collect();
    Change::new(edits).with_selections(selections)
}

pub(crate) fn signed(value: usize) -> isize {
    isize::try_from(value).expect("a text offset always fits in isize")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: usize, end: usize, insert: &str) -> TextEdit {
        TextEdit::new(Span::new(start, end), insert)
    }

    #[test]
    fn maps_positions_like_codemirror() {
        let edits = [edit(2, 2, "ab"), edit(5, 7, "x")];
        assert_eq!(map_pos(&edits, 2, Assoc::Before), 2);
        assert_eq!(map_pos(&edits, 2, Assoc::After), 4);
        assert_eq!(map_pos(&edits, 5, Assoc::After), 7);
        assert_eq!(map_pos(&edits, 6, Assoc::After), 8);
        assert_eq!(map_pos(&edits, 6, Assoc::Before), 7);
        assert_eq!(map_pos(&edits, 7, Assoc::Before), 8);
        assert_eq!(map_pos(&edits, 9, Assoc::Before), 10);
    }

    #[test]
    fn diffs_to_a_minimal_edit() {
        assert_eq!(diff("1. a\n2. b", "1. a\n3. b"), Some(edit(5, 6, "3")));
        assert_eq!(diff("same", "same"), None);
        assert_eq!(diff("aé", "aè"), Some(edit(1, 3, "è")));
    }
}
