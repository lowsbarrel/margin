use margin_markdown::{Document, Span, TableModel, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::locate::{Direction, cell_at, extent, on_delimiter_row, primary, table_at};
use crate::ops::append_row;
use crate::write::{render, replace, rows_of};

fn caret(edits: Vec<TextEdit>, at: usize) -> Change {
    Change::new(edits).with_selections(vec![Selection::cursor(at)])
}

fn ensure_cell(
    document: &Document,
    table: &TableModel,
    row: usize,
    col: usize,
) -> Option<(Vec<TextEdit>, Span)> {
    let line = table.rows.get(row)?;
    let real = line
        .cells
        .iter()
        .position(|cell| cell.slot == Span::empty(line.span.end))
        .unwrap_or(line.cells.len());
    if col < real {
        return Some((Vec::new(), line.cells.get(col)?.span));
    }
    let text = line.span.slice(document.text());
    let trimmed = text.trim_end();
    let mut insert = String::from(if trimmed.ends_with('|') { "" } else { " |" });
    for _ in real..=col {
        insert.push_str("  |");
    }
    let end = line.span.start + trimmed.len();
    let at = end + insert.len() - 2;
    let edit = TextEdit::new(Span::new(end, line.span.end), insert);
    Some((vec![edit], Span::empty(at)))
}

fn enter_cell_end(
    document: &Document,
    table: &TableModel,
    row: usize,
    col: usize,
) -> Option<Change> {
    let (edits, cell) = ensure_cell(document, table, row, col)?;
    Some(caret(edits, cell.end))
}

#[must_use]
pub fn next_cell(document: &Document, selections: &[Selection]) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    let table = table_at(document, at.table.start)?;
    if at.column + 1 < table.cols {
        return enter_cell_end(document, table, at.row, at.column + 1);
    }
    if at.row + 1 < table.rows.len() {
        return enter_cell_end(document, table, at.row + 1, 0);
    }
    Some(append_row(document, table, 0))
}

#[must_use]
pub fn previous_cell(document: &Document, selections: &[Selection]) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    let table = table_at(document, at.table.start)?;
    if at.column > 0 {
        return enter_cell_end(document, table, at.row, at.column - 1);
    }
    if at.row > 0 {
        return enter_cell_end(document, table, at.row - 1, table.cols - 1);
    }
    Some(Change::new(Vec::new()).with_selections(selections.to_vec()))
}

#[must_use]
pub fn enter_cell(
    document: &Document,
    selections: &[Selection],
    column: Option<usize>,
) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    let table = table_at(document, at.table.start)?;
    let last = at.row + 1 == table.rows.len();
    let blank = table
        .rows
        .get(at.row)
        .is_some_and(|row| row.cells.iter().all(|cell| cell.text.is_empty()));
    if last && at.row > 0 && blank {
        return Some(drop_last_row(document, table));
    }
    let target = column.unwrap_or(at.column);
    if last {
        return Some(append_row(document, table, target));
    }
    enter_cell_end(document, table, at.row + 1, target)
}

fn drop_last_row(document: &Document, table: &TableModel) -> Change {
    let mut rows = rows_of(table);
    rows.pop();
    let written = render(document, table, &rows, &table.align);
    let end = written.end(table);
    if extent(table).end < document.text().len() {
        return caret(written.edits, end + 1);
    }
    let old = extent(table).slice(document.text());
    let mut text = written.text;
    text.push('\n');
    caret(replace(table.span.start, old, &text), end + 1)
}

#[must_use]
pub fn leave_delimiter(document: &Document, selections: &[Selection]) -> Option<Change> {
    let head = primary(selections)?.head;
    let table = table_at(document, head)?;
    if !on_delimiter_row(table, head) {
        return None;
    }
    if table.rows.len() > 1 {
        return enter_cell_end(document, table, 1, 0);
    }
    Some(append_row(document, table, 0))
}

#[must_use]
pub fn leave_table(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    Some(leave_span(document, at.table, direction))
}

fn leave_span(document: &Document, table: Span, direction: Direction) -> Change {
    match direction {
        Direction::Backward => {
            let first = document.line_of(table.start);
            if first == 0 {
                caret(vec![TextEdit::new(Span::empty(0), "\n")], 0)
            } else {
                caret(Vec::new(), document.line_span(first - 1).end)
            }
        }
        Direction::Forward => {
            let last = document.line_of(table.end);
            if last + 1 >= document.line_count() {
                let end = document.line_span(last).end;
                caret(vec![TextEdit::new(Span::empty(end), "\n")], end + 1)
            } else {
                caret(Vec::new(), document.line_span(last + 1).start)
            }
        }
    }
}

#[must_use]
pub fn vertical_cell(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    let table = table_at(document, at.table.start)?;
    let row = match direction {
        Direction::Backward => at.row.checked_sub(1),
        Direction::Forward => Some(at.row + 1).filter(|row| *row < table.rows.len()),
    };
    let Some(row) = row else {
        return Some(leave_span(document, at.table, direction));
    };
    let (edits, cell) = ensure_cell(document, table, row, at.column)?;
    Some(caret(edits, cell.start))
}

#[must_use]
pub fn horizontal_cell(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let at = cell_at(document, primary(selections)?.head)?;
    let table = table_at(document, at.table.start)?;
    let target = match direction {
        Direction::Forward if at.column + 1 < table.cols => Some((at.row, at.column + 1)),
        Direction::Forward => Some((at.row + 1, 0)).filter(|(row, _)| *row < table.rows.len()),
        Direction::Backward if at.column > 0 => Some((at.row, at.column - 1)),
        Direction::Backward => at.row.checked_sub(1).map(|row| (row, table.cols - 1)),
    };
    let Some((row, col)) = target else {
        return Some(leave_span(document, at.table, direction));
    };
    let (edits, cell) = ensure_cell(document, table, row, col)?;
    let anchor = match direction {
        Direction::Forward => cell.start,
        Direction::Backward => cell.end,
    };
    Some(caret(edits, anchor))
}

#[must_use]
pub fn enter_table(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let range = primary(selections)?;
    if !range.is_empty() || cell_at(document, range.head).is_some() {
        return None;
    }
    let line = document.line_of(range.head);
    let next = match direction {
        Direction::Backward => line.checked_sub(1)?,
        Direction::Forward => Some(line + 1).filter(|next| *next < document.line_count())?,
    };
    let at = cell_at(document, document.line_span(next).start)?;
    let table = table_at(document, at.table.start)?;
    let row = match direction {
        Direction::Forward => 0,
        Direction::Backward => table.rows.len() - 1,
    };
    let (edits, cell) = ensure_cell(document, table, row, 0)?;
    Some(caret(edits, cell.start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::applied;

    fn run(
        source: &str,
        at: usize,
        command: impl Fn(&Document, &[Selection]) -> Option<Change>,
    ) -> (String, usize) {
        let document = Document::parse(source);
        let change = command(&document, &[Selection::cursor(at)]).expect("applies");
        let caret = change
            .selections
            .as_ref()
            .and_then(|list| list.first())
            .expect("caret")
            .head;
        (applied(source, &change), caret)
    }

    const TABLE: &str = "| a | b |\n| - | - |\n| c | d |\n";

    #[test]
    fn tab_moves_to_the_next_cell_end() {
        assert_eq!(run(TABLE, 2, next_cell), (TABLE.to_owned(), 7));
        assert_eq!(run(TABLE, 7, next_cell), (TABLE.to_owned(), 23));
    }

    #[test]
    fn tab_at_the_last_cell_appends_a_row() {
        let (text, caret) = run(TABLE, 27, next_cell);
        assert_eq!(
            text,
            "| a   | b   |\n| --- | --- |\n| c   | d   |\n|     |     |\n"
        );
        assert_eq!(caret, 44);
    }

    #[test]
    fn shift_tab_moves_back() {
        assert_eq!(run(TABLE, 22, previous_cell).1, 7);
        assert_eq!(run(TABLE, 2, previous_cell).1, 2);
    }

    #[test]
    fn enter_goes_down_the_same_column() {
        assert_eq!(run(TABLE, 6, |d, s| enter_cell(d, s, None)).1, 27);
    }

    #[test]
    fn enter_on_an_empty_last_row_leaves_the_table() {
        let source = "| a | b |\n| - | - |\n|  |  |\nnext";
        let (text, caret) = run(source, 22, |d, s| enter_cell(d, s, None));
        assert_eq!(text, "| a   | b   |\n| --- | --- |\nnext");
        assert_eq!(caret, 28);
        let (text, caret) = run("| a |\n| - |\n|  |", 14, |d, s| enter_cell(d, s, None));
        assert_eq!(text, "| a   |\n| --- |\n");
        assert_eq!(caret, 16);
    }

    #[test]
    fn missing_cells_are_written_out() {
        let source = "| a | b |\n| - | - |\n| c\n";
        let (text, caret) = run(source, 22, next_cell);
        assert_eq!(text, "| a | b |\n| - | - |\n| c |  |\n");
        assert_eq!(caret, 26);
    }

    #[test]
    fn enter_on_the_delimiter_row_goes_to_the_body() {
        assert_eq!(run(TABLE, 12, leave_delimiter).1, 23);
    }

    #[test]
    fn arrows_leave_and_enter_the_table() {
        let source = "above\n\n| a |\n| - |\n| b |\n\nbelow";
        assert_eq!(
            run(source, 21, |d, s| vertical_cell(d, s, Direction::Forward)).1,
            25
        );
        assert_eq!(
            run(source, 9, |d, s| vertical_cell(d, s, Direction::Backward)).1,
            6
        );
        assert_eq!(
            run(source, 6, |d, s| enter_table(d, s, Direction::Forward)).1,
            9
        );
        assert_eq!(
            run(source, 25, |d, s| enter_table(d, s, Direction::Backward)).1,
            21
        );
        let (text, caret) = run("| a |\n| - |", 2, |d, s| {
            leave_table(d, s, Direction::Backward)
        });
        assert_eq!((text.as_str(), caret), ("\n| a |\n| - |", 0));
    }
}
