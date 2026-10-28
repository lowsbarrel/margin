use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::locate::{extent, table_at};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gap {
    AboveTable,
    BelowTable,
}

fn gap_of(document: &Document, edit: &TextEdit) -> Option<Gap> {
    let line = document.line_of(edit.span.start);
    let span = document.line_span(line);
    if !document.line_text(line).trim().is_empty() || document.line_of(edit.span.end) != line {
        return None;
    }
    let text = document.text();
    let before = text.get(span.start..edit.span.start)?;
    let after = text.get(edit.span.end..span.end)?;
    let typed = format!("{before}{}{after}", edit.insert);
    let blocks = |segment: &str| {
        let segment = segment.trim();
        !segment.is_empty() && !segment.starts_with('|')
    };
    let below = document.line_span(line + 1);
    if line + 1 < document.line_count()
        && table_at(document, below.start).is_some_and(|table| table.span.start == below.start)
        && typed.rsplit('\n').next().is_some_and(blocks)
    {
        return Some(Gap::AboveTable);
    }
    let above = line.checked_sub(1).map(|prev| document.line_span(prev))?;
    (table_at(document, above.start).is_some_and(|table| extent(table).end == above.end)
        && typed.split('\n').next().is_some_and(blocks))
    .then_some(Gap::BelowTable)
}

fn shift(position: usize, points: &[(usize, Gap)]) -> usize {
    position
        + points
            .iter()
            .filter(|(point, gap)| match gap {
                Gap::AboveTable => position > *point,
                Gap::BelowTable => position >= *point,
            })
            .count()
}

#[must_use]
pub fn keep_table_gaps(document: &Document, selections: &[Selection], change: Change) -> Change {
    let mut edits = change.edits;
    edits.sort_by_key(|edit| edit.span.start);
    let mut points = Vec::new();
    let mut delta: isize = 0;
    let mut carets = Vec::new();
    for edit in &mut edits {
        let start = edit.span.start.checked_add_signed(delta).unwrap_or(0);
        let gap = gap_of(document, edit);
        if let Some(gap) = gap {
            let point = match gap {
                Gap::AboveTable => start + edit.insert.len(),
                Gap::BelowTable => start,
            };
            points.push((point, gap));
        }
        carets.push((edit.span, start + edit.insert.len()));
        delta += edit.insert.len().cast_signed() - edit.span.len().cast_signed();
        match gap {
            Some(Gap::AboveTable) => edit.insert.push('\n'),
            Some(Gap::BelowTable) => edit.insert.insert(0, '\n'),
            None => {}
        }
    }
    if points.is_empty() {
        return Change {
            edits,
            selections: change.selections,
        };
    }
    let mapped = change.selections.unwrap_or_else(|| {
        selections
            .iter()
            .map(|selection| {
                let map = |offset| map_through(offset, &carets);
                Selection::new(map(selection.anchor), map(selection.head))
            })
            .collect()
    });
    let selections = mapped
        .iter()
        .map(|selection| {
            Selection::new(
                shift(selection.anchor, &points),
                shift(selection.head, &points),
            )
        })
        .collect();
    Change::new(edits).with_selections(selections)
}

fn map_through(offset: usize, edits: &[(Span, usize)]) -> usize {
    let mut delta: isize = 0;
    for (span, end) in edits {
        if offset < span.start {
            break;
        }
        if offset <= span.end {
            return *end;
        }
        delta = (*end).cast_signed() - span.end.cast_signed();
    }
    offset.checked_add_signed(delta).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::applied;

    fn typed(source: &str, at: usize, text: &str) -> (String, usize) {
        let document = Document::parse(source);
        let change = Change::new(vec![TextEdit::new(Span::empty(at), text)]);
        let kept = keep_table_gaps(&document, &[Selection::cursor(at)], change);
        let caret = kept
            .selections
            .as_ref()
            .and_then(|list| list.first())
            .expect("caret")
            .head;
        (applied(source, &kept), caret)
    }

    #[test]
    fn typing_above_a_table_keeps_a_blank_line() {
        assert_eq!(
            typed("\n| a |\n| - |", 0, "x"),
            ("x\n\n| a |\n| - |".to_owned(), 1)
        );
    }

    #[test]
    fn typing_below_a_table_keeps_a_blank_line() {
        assert_eq!(
            typed("| a |\n| - |\n", 12, "x"),
            ("| a |\n| - |\n\nx".to_owned(), 14)
        );
    }

    #[test]
    fn typing_a_row_is_left_alone() {
        let source = "| a |\n| - |\n";
        let document = Document::parse(source);
        let change = Change::new(vec![TextEdit::new(Span::empty(12), "| b |")]);
        let kept = keep_table_gaps(&document, &[Selection::cursor(12)], change.clone());
        assert_eq!(kept, change);
    }
}
