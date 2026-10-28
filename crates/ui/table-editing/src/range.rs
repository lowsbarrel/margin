use std::ops::RangeInclusive;

use margin_markdown::{CellPlace, Document, TableModel};
use margin_text_buffer::{Change, Selection};

use crate::locate::{extent, primary, table_at};
use crate::write::{rows_of, write_table};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRange {
    pub table: usize,
    pub anchor: CellPlace,
    pub head: CellPlace,
}

impl CellRange {
    #[must_use]
    pub fn extend(current: Option<&Self>, table: usize, from: CellPlace, head: CellPlace) -> Self {
        let anchor = current
            .filter(|range| range.table == table)
            .map_or(from, |range| range.anchor);
        Self {
            table,
            anchor,
            head,
        }
    }

    #[must_use]
    pub fn rows(&self) -> RangeInclusive<usize> {
        self.anchor.row.min(self.head.row)..=self.anchor.row.max(self.head.row)
    }

    #[must_use]
    pub fn columns(&self) -> RangeInclusive<usize> {
        self.anchor.col.min(self.head.col)..=self.anchor.col.max(self.head.col)
    }

    #[must_use]
    pub fn contains(&self, row: usize, col: usize) -> bool {
        self.rows().contains(&row) && self.columns().contains(&col)
    }

    #[must_use]
    pub fn columns_for(range: Option<&Self>, table: usize, col: usize) -> RangeInclusive<usize> {
        match range {
            Some(range) if range.table == table && range.columns().contains(&col) => {
                range.columns()
            }
            _ => col..=col,
        }
    }
}

#[must_use]
pub fn select_table(table: &TableModel) -> (Selection, CellRange) {
    let range = CellRange {
        table: table.span.start,
        anchor: CellPlace { row: 0, col: 0 },
        head: CellPlace {
            row: table.rows.len() - 1,
            col: table.cols - 1,
        },
    };
    (Selection::new(table.span.start, extent(table).end), range)
}

#[must_use]
pub fn select_row(table: &TableModel, row: usize) -> CellRange {
    CellRange {
        table: table.span.start,
        anchor: CellPlace { row, col: 0 },
        head: CellPlace {
            row,
            col: table.cols - 1,
        },
    }
}

#[must_use]
pub fn select_column(table: &TableModel, col: usize) -> CellRange {
    CellRange {
        table: table.span.start,
        anchor: CellPlace { row: 0, col },
        head: CellPlace {
            row: table.rows.len() - 1,
            col,
        },
    }
}

#[must_use]
pub fn select_table_above(
    document: &Document,
    selections: &[Selection],
) -> Option<(Change, CellRange)> {
    let range = primary(selections)?;
    if !range.is_empty() {
        return None;
    }
    let line = document.line_of(range.head);
    if line == 0 || document.line_span(line).start != range.head {
        return None;
    }
    let table = table_at(document, range.head - 1)?;
    if extent(table).end != range.head - 1 {
        return None;
    }
    let (selection, cells) = select_table(table);
    Some((
        Change::new(Vec::new()).with_selections(vec![selection]),
        cells,
    ))
}

#[must_use]
pub fn copy_cells(document: &Document, range: &CellRange) -> Option<String> {
    let table = table_at(document, range.table)?;
    let lines: Vec<String> = range
        .rows()
        .map(|row| {
            let cells: Vec<&str> = range
                .columns()
                .map(|col| {
                    table
                        .cell(CellPlace { row, col })
                        .map_or("", |cell| cell.text.as_str())
                })
                .collect();
            format!("| {} |", cells.join(" | "))
        })
        .collect();
    let rule = vec!["---"; range.columns().count()].join(" | ");
    let mut out = lines.first()?.clone();
    out.push_str("\n| ");
    out.push_str(&rule);
    out.push_str(" |");
    for line in lines.iter().skip(1) {
        out.push('\n');
        out.push_str(line);
    }
    Some(out)
}

fn rewrite_cells(
    document: &Document,
    range: &CellRange,
    edit: impl Fn(&str, &[String]) -> String,
) -> Option<Change> {
    let table = table_at(document, range.table)?;
    let mut rows = rows_of(table);
    let places: Vec<CellPlace> = range
        .rows()
        .flat_map(|row| range.columns().map(move |col| CellPlace { row, col }))
        .filter(|place| table.cell(*place).is_some())
        .collect();
    let texts: Vec<String> = places
        .iter()
        .filter_map(|place| table.cell(*place).map(|cell| cell.text.clone()))
        .collect();
    for place in &places {
        if let Some(cell) = rows
            .get_mut(place.row)
            .and_then(|row| row.get_mut(place.col))
        {
            *cell = edit(cell, &texts);
        }
    }
    Some(write_table(
        document,
        table,
        &rows,
        &table.align,
        Some(range.anchor),
    ))
}

#[must_use]
pub fn clear_cells(document: &Document, range: &CellRange) -> Option<Change> {
    rewrite_cells(document, range, |_, _| String::new())
}

fn wrapped(text: &str, marker: &str) -> bool {
    if text.len() <= marker.len() * 2 || !text.starts_with(marker) || !text.ends_with(marker) {
        return false;
    }
    marker != "*" || (!text.starts_with("**") && !text.ends_with("**"))
}

#[must_use]
pub fn format_cells(document: &Document, range: &CellRange, marker: &str) -> Option<Change> {
    rewrite_cells(document, range, |text, all| {
        if text.is_empty() {
            return String::new();
        }
        let unwrap = all
            .iter()
            .filter(|cell| !cell.is_empty())
            .all(|cell| wrapped(cell, marker));
        match text
            .strip_prefix(marker)
            .and_then(|rest| rest.strip_suffix(marker))
        {
            Some(inner) if unwrap => inner.to_owned(),
            _ => format!("{marker}{text}{marker}"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::applied;

    const TABLE: &str = "| a | b |\n| - | - |\n| c | d |\n\nnext";

    fn whole() -> CellRange {
        CellRange {
            table: 0,
            anchor: CellPlace { row: 0, col: 0 },
            head: CellPlace { row: 1, col: 1 },
        }
    }

    #[test]
    fn copies_a_rectangle_as_a_table() {
        let document = Document::parse(TABLE);
        let range = CellRange {
            table: 0,
            anchor: CellPlace { row: 1, col: 1 },
            head: CellPlace { row: 0, col: 1 },
        };
        assert_eq!(
            copy_cells(&document, &range).as_deref(),
            Some("| b |\n| --- |\n| d |")
        );
        assert!(range.contains(0, 1) && !range.contains(0, 0));
    }

    #[test]
    fn clears_cells_and_keeps_the_caret_at_the_anchor() {
        let document = Document::parse(TABLE);
        let change = clear_cells(&document, &whole()).expect("cleared");
        assert!(applied(TABLE, &change).starts_with("|     |     |\n| --- | --- |\n|     |     |"));
        assert_eq!(change.selections, Some(vec![Selection::cursor(2)]));
    }

    #[test]
    fn formats_and_unformats_cells() {
        let document = Document::parse(TABLE);
        let bold = applied(
            TABLE,
            &format_cells(&document, &whole(), "**").expect("bold"),
        );
        assert!(bold.starts_with("| **a** | **b** |"));
        let again = Document::parse(&bold);
        let plain = applied(&bold, &format_cells(&again, &whole(), "**").expect("plain"));
        assert!(plain.starts_with("| a   | b   |"));
    }

    #[test]
    fn backspace_below_a_table_selects_it() {
        let source = "| a |\n| - |\nnext";
        let document = Document::parse(source);
        let (change, range) =
            select_table_above(&document, &[Selection::cursor(12)]).expect("selected");
        assert_eq!(change.selections, Some(vec![Selection::new(0, 11)]));
        assert_eq!(range.head, CellPlace { row: 0, col: 0 });
        assert!(select_table_above(&document, &[Selection::cursor(13)]).is_none());
    }

    #[test]
    fn menu_alignment_targets_the_ranged_columns() {
        assert_eq!(CellRange::columns_for(Some(&whole()), 0, 1), 0..=1);
        assert_eq!(CellRange::columns_for(Some(&whole()), 9, 1), 1..=1);
    }
}
