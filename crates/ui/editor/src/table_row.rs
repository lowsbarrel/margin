use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Styled, div};
use margin_live_preview::{Row, render_cell};
use margin_markdown::{Align, Document, TableCell, TableModel};
use margin_table_editing::{OnTableOp, TableControl, table_control};

use crate::atoms::render_atoms;
use crate::line_row::{Style, inline_pieces};
use crate::metrics;
use crate::row_context::{RowAction, RowContext};
use crate::table_virtual::{self, TableViewport};

pub(crate) const CELL_SIZE: f32 = 0.95;

pub(crate) fn render(
    document: &Document,
    row: &Row,
    item: usize,
    context: &RowContext,
    viewport: Option<&TableViewport>,
    cx: &mut App,
) -> AnyElement {
    let Some(table) = document
        .block_at(row.span.start)
        .and_then(|block| block.table())
    else {
        return div().into_any_element();
    };
    let palette = context.palette();
    let frame = div()
        .max_w_full()
        .overflow_hidden()
        .border_1()
        .border_color(palette.border_divider)
        .text_size(metrics::em(CELL_SIZE))
        .line_height(gpui_kit::relative(1.5));
    let body = if let Some(viewport) = viewport {
        table_virtual::rows(frame, document, table, viewport, item, context, cx)
    } else {
        let mut grid = frame
            .grid()
            .grid_cols_max_content(u16::try_from(table.cols).unwrap_or(u16::MAX));
        for index in 0..table.rows.len() {
            for column in 0..table.cols {
                grid = grid.child(cell(document, table, index, column, item, context, cx));
            }
        }
        grid
    };
    let anchor = table
        .rows
        .first()
        .and_then(|first| first.cells.first())
        .map_or(table.span.start, |cell| cell.slot.start);
    let handler = context.handler();
    let on_op: OnTableOp = Rc::new(move |op, window, cx| {
        handler(RowAction::Table { at: anchor, op }, window, cx);
    });
    let framed = v_flex()
        .w_full()
        .py(metrics::em(0.5))
        .gap_1()
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_1()
                .child(body)
                .children(
                    (!context.is_embedded())
                        .then(|| table_control(TableControl::AddColumn, Rc::clone(&on_op), cx)),
                ),
        )
        .children((!context.is_embedded()).then(|| table_control(TableControl::AddRow, on_op, cx)));
    if context.is_embedded() {
        framed.into_any_element()
    } else {
        metrics::column(framed).into_any_element()
    }
}

pub(crate) fn cell(
    document: &Document,
    table: &TableModel,
    index: usize,
    column: usize,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> Div {
    let palette = context.palette();
    let align = table.align.get(column).copied().unwrap_or(Align::None);
    let body = table
        .rows
        .get(index)
        .and_then(|row| row.cells.get(column))
        .map(|cell| cell_content(document, cell, item, context, cx));
    let mut cell = div()
        .min_w(metrics::em(CELL_SIZE * 3.0))
        .px(metrics::em(CELL_SIZE * 0.6))
        .py(metrics::em(CELL_SIZE * 0.3))
        .flex()
        .flex_wrap()
        .border_color(palette.border_divider);
    cell = match align {
        Align::Center => cell.justify_center(),
        Align::Right => cell.justify_end(),
        Align::Left | Align::None => cell,
    };
    if index == 0 {
        cell = cell
            .bg(palette.surface(2))
            .font_weight(FontWeight::SEMIBOLD);
    } else if index.is_multiple_of(2) {
        cell = cell.bg(palette.surface(1));
    }
    if index + 1 < table.rows.len() {
        cell = cell.border_b_1();
    }
    if column + 1 < table.cols {
        cell = cell.border_r_1();
    }
    cell.children(body.into_iter().flatten())
}

fn cell_content(
    document: &Document,
    cell: &TableCell,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> Vec<AnyElement> {
    let view = render_cell(document, cell, context.reveal());
    let atoms = render_atoms(&view, document.text(), item, context, cx);
    inline_pieces(
        &view,
        &Style::prose(context.palette().text_primary),
        atoms,
        context,
        cx,
    )
}
