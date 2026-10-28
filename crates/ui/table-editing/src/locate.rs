use margin_markdown::{Block, CellPlace, Document, Span, TableModel};
use margin_text_buffer::Selection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellLocation {
    pub table: Span,
    pub row: usize,
    pub column: usize,
    pub cell: Span,
}

impl CellLocation {
    #[must_use]
    pub const fn place(&self) -> CellPlace {
        CellPlace {
            row: self.row,
            col: self.column,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
}

#[must_use]
pub fn table_at(document: &Document, offset: usize) -> Option<&TableModel> {
    document
        .blocks()
        .iter()
        .filter_map(Block::table)
        .find(|table| {
            let span = extent(table);
            table.anchored && span.start <= offset && offset <= span.end
        })
}

#[must_use]
pub fn cell_at(document: &Document, offset: usize) -> Option<CellLocation> {
    let table = table_at(document, offset)?;
    let place = table.locate_cell(offset)?;
    let cell = table.cell(place)?;
    Some(CellLocation {
        table: extent(table),
        row: place.row,
        column: place.col,
        cell: cell.span,
    })
}

#[must_use]
pub fn on_delimiter_row(table: &TableModel, offset: usize) -> bool {
    table.delimiter.start <= offset && offset <= table.delimiter.end
}

pub(crate) fn primary(selections: &[Selection]) -> Option<Selection> {
    selections.last().copied()
}

pub(crate) fn extent(table: &TableModel) -> Span {
    Span::new(table.span.start, table.span.end.max(table.delimiter.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_cells_by_offset() {
        let document = Document::parse("| a | b |\n| - | - |\n| c | d |\n");
        let found = cell_at(&document, 25).expect("cell d");
        assert_eq!((found.row, found.column), (1, 1));
        assert_eq!(found.cell.slice(document.text()), "d");
        assert!(cell_at(&document, 12).is_none());
    }

    #[test]
    fn ignores_tables_glued_to_a_paragraph() {
        let document = Document::parse("text\n| a | b |\n| - | - |\n");
        assert!(table_at(&document, 7).is_none());
    }
}
