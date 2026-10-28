use std::ops::RangeInclusive;

use margin_markdown::{Align, CellPlace, Document, TableModel, empty_row};
use margin_text_buffer::Change;

use crate::locate::{CellLocation, table_at};
use crate::sort::compare_cells;
use crate::write::{rows_of, write_table};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableOp {
    InsertRowAbove,
    InsertRowBelow,
    InsertColumnLeft,
    InsertColumnRight,
    AppendRow,
    AppendColumn,
    MoveRowUp,
    MoveRowDown,
    MoveColumnLeft,
    MoveColumnRight,
    MoveRowTo(usize),
    MoveColumnTo(usize),
    SortAscending,
    SortDescending,
    AlignLeft,
    AlignCenter,
    AlignRight,
    AlignNone,
    DeleteRow,
    DeleteColumn,
}

#[must_use]
pub fn apply_table_op(document: &Document, at: &CellLocation, op: TableOp) -> Option<Change> {
    let table = table_at(document, at.table.start)?;
    let (row, col) = (at.row, at.column);
    match op {
        TableOp::InsertRowAbove => Some(insert_row(document, table, row)),
        TableOp::InsertRowBelow => Some(insert_row(document, table, row + 1)),
        TableOp::InsertColumnLeft => Some(insert_column(document, table, col)),
        TableOp::InsertColumnRight => Some(insert_column(document, table, col + 1)),
        TableOp::AppendRow => Some(insert_row(document, table, table.rows.len())),
        TableOp::AppendColumn => Some(insert_column(document, table, table.cols)),
        TableOp::MoveRowUp => move_row(document, table, row, row.checked_sub(1)?),
        TableOp::MoveRowDown => move_row(document, table, row, row + 1),
        TableOp::MoveColumnLeft => move_column(document, table, col, col.checked_sub(1)?),
        TableOp::MoveColumnRight => move_column(document, table, col, col + 1),
        TableOp::MoveRowTo(to) => move_row(document, table, row, to),
        TableOp::MoveColumnTo(to) => move_column(document, table, col, to),
        TableOp::SortAscending => sort_rows(document, table, col, Order::Ascending),
        TableOp::SortDescending => sort_rows(document, table, col, Order::Descending),
        TableOp::AlignLeft => align_columns(document, at, col..=col, Align::Left),
        TableOp::AlignCenter => align_columns(document, at, col..=col, Align::Center),
        TableOp::AlignRight => align_columns(document, at, col..=col, Align::Right),
        TableOp::AlignNone => align_columns(document, at, col..=col, Align::None),
        TableOp::DeleteRow => delete_row(document, table, row),
        TableOp::DeleteColumn => delete_column(document, table, col),
    }
}

#[must_use]
pub fn align_columns(
    document: &Document,
    at: &CellLocation,
    columns: RangeInclusive<usize>,
    align: Align,
) -> Option<Change> {
    let table = table_at(document, at.table.start)?;
    let mut next = table.align.clone();
    for slot in next.get_mut(columns)? {
        *slot = align;
    }
    Some(write_table(document, table, &rows_of(table), &next, None))
}

#[derive(Clone, Copy)]
enum Order {
    Ascending,
    Descending,
}

fn insert_row(document: &Document, table: &TableModel, index: usize) -> Change {
    let mut rows = rows_of(table);
    rows.insert(index.min(rows.len()), empty_row(table.cols));
    write_table(document, table, &rows, &table.align, None)
}

pub(crate) fn append_row(document: &Document, table: &TableModel, select: usize) -> Change {
    let mut rows = rows_of(table);
    rows.push(empty_row(table.cols));
    let place = CellPlace {
        row: rows.len() - 1,
        col: select,
    };
    write_table(document, table, &rows, &table.align, Some(place))
}

pub(crate) fn delete_row(document: &Document, table: &TableModel, index: usize) -> Option<Change> {
    if table.rows.len() <= 1 || index >= table.rows.len() {
        return None;
    }
    let mut rows = rows_of(table);
    rows.remove(index);
    Some(write_table(document, table, &rows, &table.align, None))
}

fn move_row(document: &Document, table: &TableModel, from: usize, to: usize) -> Option<Change> {
    let count = table.rows.len();
    if from == to || from >= count || to >= count {
        return None;
    }
    let mut rows = rows_of(table);
    let moved = rows.remove(from);
    rows.insert(to, moved);
    Some(write_table(document, table, &rows, &table.align, None))
}

fn insert_column(document: &Document, table: &TableModel, index: usize) -> Change {
    let at = index.min(table.cols);
    let mut rows = rows_of(table);
    for cells in &mut rows {
        cells.insert(at.min(cells.len()), String::new());
    }
    let mut align = table.align.clone();
    align.insert(at, Align::None);
    write_table(document, table, &rows, &align, None)
}

fn delete_column(document: &Document, table: &TableModel, index: usize) -> Option<Change> {
    if table.cols <= 1 || index >= table.cols {
        return None;
    }
    let mut rows = rows_of(table);
    for cells in &mut rows {
        if index < cells.len() {
            cells.remove(index);
        }
    }
    let mut align = table.align.clone();
    align.remove(index);
    Some(write_table(document, table, &rows, &align, None))
}

fn move_column(document: &Document, table: &TableModel, from: usize, to: usize) -> Option<Change> {
    if from == to || from >= table.cols || to >= table.cols {
        return None;
    }
    let mut rows = rows_of(table);
    for cells in &mut rows {
        if from < cells.len() && to < cells.len() {
            let moved = cells.remove(from);
            cells.insert(to, moved);
        }
    }
    let mut align = table.align.clone();
    let moved = align.remove(from);
    align.insert(to, moved);
    Some(write_table(document, table, &rows, &align, None))
}

fn sort_rows(document: &Document, table: &TableModel, col: usize, order: Order) -> Option<Change> {
    let mut rows = rows_of(table);
    let body = rows.get_mut(1..)?;
    body.sort_by(|a, b| {
        let a = a.get(col).map_or("", String::as_str);
        let b = b.get(col).map_or("", String::as_str);
        match order {
            Order::Ascending => compare_cells(a, b),
            Order::Descending => compare_cells(b, a),
        }
    });
    Some(write_table(document, table, &rows, &table.align, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locate::cell_at;
    use crate::write::applied;

    const TABLE: &str = "| a | b |\n| --- | --- |\n| 2 | x |\n| 10 | y |\n";

    fn run(source: &str, offset: usize, op: TableOp) -> String {
        let document = Document::parse(source);
        let at = cell_at(&document, offset).expect("cell");
        applied(
            source,
            &apply_table_op(&document, &at, op).expect("applies"),
        )
    }

    #[test]
    fn inserts_rows() {
        let above = run(TABLE, 26, TableOp::InsertRowAbove);
        assert_eq!(
            above,
            "| a   | b   |\n| --- | --- |\n|     |     |\n| 2   | x   |\n| 10  | y   |\n"
        );
        let below = run(TABLE, 26, TableOp::InsertRowBelow);
        assert_eq!(
            below,
            "| a   | b   |\n| --- | --- |\n| 2   | x   |\n|     |     |\n| 10  | y   |\n"
        );
    }

    #[test]
    fn inserts_and_deletes_columns() {
        let left = run(TABLE, 2, TableOp::InsertColumnLeft);
        assert!(left.starts_with("|     | a   | b   |\n| --- | --- | --- |\n|     | 2   |"));
        let gone = run(TABLE, 2, TableOp::DeleteColumn);
        assert_eq!(gone, "| b   |\n| --- |\n| x   |\n| y   |\n");
    }

    #[test]
    fn deletes_rows_but_keeps_the_last_one() {
        assert_eq!(
            run(TABLE, 26, TableOp::DeleteRow),
            "| a   | b   |\n| --- | --- |\n| 10  | y   |\n"
        );
        let single = Document::parse("| a |\n| - |\n");
        let at = cell_at(&single, 2).expect("cell");
        assert!(apply_table_op(&single, &at, TableOp::DeleteRow).is_none());
    }

    #[test]
    fn sorts_body_rows_by_column() {
        let asc = run(TABLE, 2, TableOp::SortAscending);
        assert!(asc.ends_with("| 2   | x   |\n| 10  | y   |\n"));
        let desc = run(TABLE, 2, TableOp::SortDescending);
        assert!(desc.ends_with("| 10  | y   |\n| 2   | x   |\n"));
    }

    #[test]
    fn rewrites_the_alignment_row() {
        let left = run(TABLE, 2, TableOp::AlignLeft);
        assert!(left.contains("\n| :-- | --- |\n"));
        let center = run(TABLE, 6, TableOp::AlignCenter);
        assert!(center.contains("\n| --- | :-: |\n"));
        let right = run(TABLE, 2, TableOp::AlignRight);
        assert!(right.contains("\n| --: | --- |\n|   2 | x   |"));
        let none = run(&right, 2, TableOp::AlignNone);
        assert!(none.contains("\n| --- | --- |\n"));
    }

    #[test]
    fn moves_rows_and_columns() {
        let down = run(TABLE, 26, TableOp::MoveRowDown);
        assert!(down.ends_with("| 10  | y   |\n| 2   | x   |\n"));
        let right = run(TABLE, 2, TableOp::MoveColumnRight);
        assert!(right.starts_with("| b   | a   |"));
        let document = Document::parse(TABLE);
        let at = cell_at(&document, 2).expect("cell");
        assert!(apply_table_op(&document, &at, TableOp::MoveRowUp).is_none());
    }
}
