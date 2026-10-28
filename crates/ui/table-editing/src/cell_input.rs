use std::borrow::Cow;

use margin_markdown::{CellPlace, Document, TableModel, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::locate::{Direction, cell_at, primary, table_at};
use crate::nav::{horizontal_cell, next_cell};
use crate::write::{render, rows_of};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeTyped {
    Moved(Change),
    Swallowed,
    ClosedRow,
}

#[must_use]
pub fn filter_cell_input(text: &str) -> Cow<'_, str> {
    if !text.contains(['\n', '|']) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 2);
    let mut slashes = 0;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => out.push(' '),
            '|' if slashes % 2 == 0 => out.push_str("\\|"),
            _ => out.push(ch),
        }
        slashes = if ch == '\\' { slashes + 1 } else { 0 };
    }
    Cow::Owned(out)
}

#[must_use]
pub fn type_pipe(document: &Document, selections: &[Selection]) -> Option<PipeTyped> {
    let range = primary(selections)?;
    if !range.is_empty() {
        return None;
    }
    let at = cell_at(document, range.head)?;
    let text = document.text();
    let after = text.get(range.head.min(at.cell.end)..at.cell.end)?;
    if !after.trim().is_empty() || text.get(..range.head)?.ends_with('\\') {
        return None;
    }
    let table = table_at(document, at.table.start)?;
    if at.column == 0 && at.cell.is_empty() {
        return Some(PipeTyped::Swallowed);
    }
    if at.column + 1 < table.cols {
        return next_cell(document, selections).map(PipeTyped::Moved);
    }
    Some(PipeTyped::ClosedRow)
}

#[must_use]
pub fn insert_break(document: &Document, selections: &[Selection]) -> Option<Change> {
    cell_at(document, primary(selections)?.head)?;
    let mut edits = Vec::with_capacity(selections.len());
    let mut carets = Vec::with_capacity(selections.len());
    let (mut added, mut removed) = (0, 0);
    for selection in selections {
        edits.push(TextEdit::new(selection.span(), "<br>"));
        added += 4;
        carets.push(Selection::cursor(selection.start() + added - removed));
        removed += selection.span().len();
    }
    Some(Change::new(edits).with_selections(carets))
}

#[must_use]
pub fn step_across(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let range = primary(selections)?;
    let at = cell_at(document, range.head)?;
    let edge = match direction {
        Direction::Forward => at.cell.end,
        Direction::Backward => at.cell.start,
    };
    if !range.is_empty() || range.head != edge {
        return None;
    }
    horizontal_cell(document, selections, direction)
}

fn anchor_in(table: &TableModel, offset: usize) -> Option<(CellPlace, usize)> {
    let place = table.locate_cell(offset)?;
    let cell = table.cell(place)?.span;
    let inner = offset.clamp(cell.start, cell.end) - cell.start;
    Some((place, inner))
}

#[must_use]
pub fn realign(document: &Document, selections: &[Selection]) -> Option<Change> {
    let range = primary(selections)?;
    let table = table_at(document, range.head)?;
    let head = anchor_in(table, range.head)?;
    let anchor = anchor_in(table, range.anchor).unwrap_or(head);
    let written = render(document, table, &rows_of(table), &table.align);
    if written.edits.is_empty() {
        return None;
    }
    let mapped = |(place, inner): (CellPlace, usize)| written.cell_start(table, place) + inner;
    let selection = Selection::new(mapped(anchor), mapped(head));
    Some(Change::new(written.edits).with_selections(vec![selection]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::applied;

    #[test]
    fn filters_newlines_and_pipes() {
        assert_eq!(filter_cell_input("a\r\nb\nc"), "a b c");
        assert_eq!(filter_cell_input("a|b\\|c\\\\|d"), "a\\|b\\|c\\\\\\|d");
        assert!(matches!(filter_cell_input("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn pipe_at_cell_end_moves_to_the_next_cell() {
        let source = "| ab | c |\n| - | - |\n";
        let document = Document::parse(source);
        let moved = type_pipe(&document, &[Selection::cursor(4)]).expect("moved");
        assert_eq!(
            moved,
            PipeTyped::Moved(next_cell(&document, &[Selection::cursor(4)]).expect("next"))
        );
        assert!(type_pipe(&document, &[Selection::cursor(3)]).is_none());
        assert_eq!(
            type_pipe(&document, &[Selection::cursor(8)]),
            Some(PipeTyped::ClosedRow)
        );
        let empty = Document::parse("|  | c |\n| - | - |\n");
        assert_eq!(
            type_pipe(&empty, &[Selection::cursor(2)]),
            Some(PipeTyped::Swallowed)
        );
    }

    #[test]
    fn shift_enter_inserts_a_break() {
        let source = "| ab |\n| - |\n";
        let document = Document::parse(source);
        let change = insert_break(&document, &[Selection::cursor(3)]).expect("break");
        assert_eq!(applied(source, &change), "| a<br>b |\n| - |\n");
        assert_eq!(change.selections, Some(vec![Selection::cursor(7)]));
    }

    #[test]
    fn realigns_after_a_cell_edit_keeping_the_caret() {
        let source = "| a | b |\n| --- | --- |\n| longer | x |\n";
        let document = Document::parse(source);
        let change = realign(&document, &[Selection::cursor(30)]).expect("realigned");
        assert_eq!(
            applied(source, &change),
            "| a      | b   |\n| ------ | --- |\n| longer | x   |\n"
        );
        assert_eq!(change.selections, Some(vec![Selection::cursor(40)]));
        let tidy = Document::parse("| a   |\n| --- |\n");
        assert!(realign(&tidy, &[Selection::cursor(2)]).is_none());
    }

    #[test]
    fn arrows_cross_cells_only_at_the_edge() {
        let document = Document::parse("| ab | c |\n| - | - |\n");
        assert!(step_across(&document, &[Selection::cursor(3)], Direction::Forward).is_none());
        let change =
            step_across(&document, &[Selection::cursor(4)], Direction::Forward).expect("moved");
        assert_eq!(change.selections, Some(vec![Selection::cursor(7)]));
    }
}
