use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::list_tree::{self, Indent};
use crate::mapping::{self, Assoc, RangeEdit};
use crate::text::{self, Line, TAB_SIZE};

fn unit_style(document: &Document) -> (&'static str, usize, Indent) {
    match list_tree::detect_indent_unit(document.text()) {
        "\t" => ("\t", TAB_SIZE, Indent::Tabs),
        unit => (unit, unit.len(), Indent::Spaces),
    }
}

fn by_selected_line(
    document: &Document,
    selections: &[Selection],
    mut each: impl FnMut(Line<'_>, &mut Vec<TextEdit>),
) -> Change {
    let mut at_line: Option<usize> = None;
    mapping::by_range(selections, |range| {
        let mut edits = Vec::new();
        for index in text::selected_lines(document, range) {
            let line = Line::numbered(document, index);
            let fresh = at_line.is_none_or(|seen| index > seen);
            if fresh && (range.is_empty() || range.end() > line.start) {
                each(line, &mut edits);
                at_line = Some(index);
            }
        }
        let edits = mapping::sorted(edits);
        let selection = mapping::map_selection(&edits, range, Assoc::After);
        Some(RangeEdit::new(edits, selection))
    })
    .unwrap_or_default()
}

pub(crate) fn indent_more(document: &Document, selections: &[Selection]) -> Change {
    let (unit, _, _) = unit_style(document);
    by_selected_line(document, selections, |line, edits| {
        edits.push(TextEdit::new(Span::empty(line.start), unit));
    })
}

pub(crate) fn indent_less(document: &Document, selections: &[Selection]) -> Change {
    let (_, width, style) = unit_style(document);
    by_selected_line(document, selections, |line, edits| {
        let space = line.text.len() - line.text.trim_start().len();
        if space == 0 {
            return;
        }
        let current = line.text.get(..space).unwrap_or_default();
        let wanted = list_tree::indent_text(text::columns(current).saturating_sub(width), style);
        let keep = current
            .bytes()
            .zip(wanted.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        edits.push(TextEdit::new(
            Span::new(line.start + keep, line.start + space),
            wanted.get(keep..).unwrap_or_default(),
        ));
    })
}
