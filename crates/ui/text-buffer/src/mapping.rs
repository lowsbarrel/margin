use margin_markdown::TextEdit;

use crate::bias::Bias;
use crate::selection::Selection;

#[must_use]
pub fn map_offset(edits: &[TextEdit], offset: usize, bias: Bias) -> usize {
    let mut added = 0;
    let mut removed = 0;
    for edit in edits {
        let start = edit.span.start;
        let end = edit.span.end;
        if offset < start {
            break;
        }
        if offset < end || (offset == end && start == end && bias == Bias::Left) {
            let mapped = start + added - removed;
            return if offset == start || bias == Bias::Left {
                mapped
            } else {
                mapped + edit.insert.len()
            };
        }
        added += edit.insert.len();
        removed += end - start;
    }
    offset + added - removed
}

#[must_use]
pub(crate) fn map_selection(edits: &[TextEdit], selection: Selection) -> Selection {
    if selection.is_empty() {
        return Selection::cursor(map_offset(edits, selection.head, Bias::Right));
    }
    let from = map_offset(edits, selection.start(), Bias::Right);
    let to = map_offset(edits, selection.end(), Bias::Left).max(from);
    if selection.anchor > selection.head {
        Selection::new(to, from)
    } else {
        Selection::new(from, to)
    }
}

#[cfg(test)]
mod tests {
    use margin_markdown::Span;

    use super::*;

    fn edit(start: usize, end: usize, insert: &str) -> TextEdit {
        TextEdit::new(Span::new(start, end), insert)
    }

    #[test]
    fn offsets_before_and_after_edits_shift() {
        let edits = [edit(2, 4, "xyz"), edit(6, 6, "!!")];
        assert_eq!(map_offset(&edits, 1, Bias::Left), 1);
        assert_eq!(map_offset(&edits, 5, Bias::Left), 6);
        assert_eq!(map_offset(&edits, 9, Bias::Right), 12);
    }

    #[test]
    fn insertion_points_follow_the_bias() {
        let edits = [edit(3, 3, "ab")];
        assert_eq!(map_offset(&edits, 3, Bias::Left), 3);
        assert_eq!(map_offset(&edits, 3, Bias::Right), 5);
    }

    #[test]
    fn replaced_ranges_map_to_their_edges() {
        let edits = [edit(2, 6, "x")];
        assert_eq!(map_offset(&edits, 2, Bias::Right), 2);
        assert_eq!(map_offset(&edits, 6, Bias::Left), 3);
        assert_eq!(map_offset(&edits, 4, Bias::Left), 2);
        assert_eq!(map_offset(&edits, 4, Bias::Right), 3);
    }

    #[test]
    fn cursors_move_after_insertions_and_ranges_do_not_grow() {
        let edits = [edit(2, 2, "ab"), edit(5, 5, "cd")];
        assert_eq!(
            map_selection(&edits, Selection::cursor(2)),
            Selection::cursor(4)
        );
        assert_eq!(
            map_selection(&edits, Selection::new(5, 2)),
            Selection::new(7, 4)
        );
    }
}
