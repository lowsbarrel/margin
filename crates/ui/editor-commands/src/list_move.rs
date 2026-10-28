use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::list_tree::{self, Indent, ListItem};
use crate::mapping::{self, Assoc};
use crate::text::Line;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub pos: usize,
    pub lead: &'static str,
    pub tail: &'static str,
}

fn removal(document: &Document, from: usize, to: usize) -> (usize, usize) {
    let first = Line::at(document, from);
    let last = Line::at(document, to);
    let before = first
        .index
        .checked_sub(1)
        .map(|index| Line::numbered(document, index));
    if last.index + 1 == document.line_count() {
        return (before.map_or(from, |line| line.end), to);
    }
    let after = Line::numbered(document, last.index + 1);
    let blank_around = before.is_none_or(|line| line.is_blank()) && after.is_blank();
    if blank_around && after.index + 1 < document.line_count() {
        return (from, Line::numbered(document, after.index + 1).start);
    }
    (from, after.start)
}

pub(crate) fn relocate_item(
    document: &Document,
    selections: &[Selection],
    item: &ListItem,
    target: &Target,
    columns: usize,
    indent: Indent,
    token: Option<&str>,
) -> Option<Change> {
    let (cut_from, cut_to) = removal(document, item.from, item.to);
    if target.pos > cut_from && target.pos < cut_to {
        return None;
    }
    let releveled = list_tree::relevel(document, item, columns, indent, token);
    let edits = vec![
        TextEdit::new(Span::new(cut_from, cut_to), ""),
        TextEdit::new(
            Span::empty(target.pos),
            format!("{}{}{}", target.lead, releveled.text, target.tail),
        ),
    ];
    let start =
        |edits: &[TextEdit]| mapping::map_pos(edits, target.pos, Assoc::Before) + target.lead.len();
    list_tree::list_edit(
        document,
        edits,
        |edits| {
            vec![
                start(edits),
                mapping::map_pos(edits, cut_from, Assoc::After),
            ]
        },
        |edits| {
            let move_to = |pos: usize| {
                if pos >= item.from && pos <= item.to {
                    start(edits) + releveled.map(pos - item.from)
                } else {
                    mapping::map_pos(edits, pos, Assoc::Before)
                }
            };
            selections
                .iter()
                .map(|selection| Selection::new(move_to(selection.anchor), move_to(selection.head)))
                .collect()
        },
    )
}
