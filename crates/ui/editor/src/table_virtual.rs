use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{App, Div, FontWeight, ListOffset, ParentElement, Pixels, Styled, Window, div, px};
use margin_live_preview::{LinkKind, Reveal, render_cell};
use margin_markdown::{Document, TableModel, TableRow};
use margin_theme::Palette;

use crate::metrics;
use crate::row_context::RowContext;
use crate::table_row::{self, CELL_SIZE};
use crate::text_style::{Face, RunContext, inline_runs};

pub(crate) const VIRTUAL_TABLE_ROWS: usize = 100;
const WIDTH_SLACK: f32 = 1.04;

pub(crate) struct TableViewport {
    from: Pixels,
    to: Pixels,
    row_height: Pixels,
    widths: Rc<[Pixels]>,
}

impl TableViewport {
    pub(crate) const fn new(
        visible: Range<Pixels>,
        row_height: Pixels,
        widths: Rc<[Pixels]>,
    ) -> Self {
        Self {
            from: visible.start,
            to: visible.end,
            row_height,
            widths,
        }
    }
}

pub(crate) fn row_height(window: &Window) -> Pixels {
    let rem = window.rem_size();
    let font = metrics::em(CELL_SIZE).to_pixels(rem);
    let padding = metrics::em(CELL_SIZE * 0.3).to_pixels(rem);
    font * 1.5 + padding * 2.0 + px(1.0)
}

fn table_top(window: &Window) -> Pixels {
    metrics::em(0.5).to_pixels(window.rem_size())
}

pub(crate) fn rows(
    frame: Div,
    document: &Document,
    table: &TableModel,
    viewport: &TableViewport,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> Div {
    let count = table.rows.len();
    let height = viewport.row_height;
    let index_at = |offset: Pixels| row_index(offset / height);
    let first = index_at(viewport.from).clamp(1, count);
    let last = (index_at(viewport.to) + 1).clamp(first, count);
    let mut body = v_flex().child(row(document, table, 0, viewport, item, context, cx));
    if first > 1 {
        body = body.child(div().flex_none().h(height * row_count(first - 1)));
    }
    for index in first..last {
        body = body.child(row(document, table, index, viewport, item, context, cx));
    }
    if last < count {
        body = body.child(div().flex_none().h(height * row_count(count - last)));
    }
    frame.child(body)
}

fn row(
    document: &Document,
    table: &TableModel,
    index: usize,
    viewport: &TableViewport,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> Div {
    let mut row = h_flex().flex_none().h(viewport.row_height);
    for column in 0..table.cols {
        let width = viewport.widths.get(column).copied().unwrap_or_default();
        let cell = table_row::cell(document, table, index, column, item, context, cx)
            .flex_none()
            .flex_nowrap()
            .items_center()
            .w(width)
            .h_full()
            .overflow_hidden()
            .whitespace_nowrap();
        row = row.child(cell);
    }
    row
}

#[derive(Default)]
pub(crate) struct TableCache {
    rows: HashMap<u64, Rc<[Pixels]>>,
    table: Option<(u64, Rc<[Pixels]>)>,
    scroll: Option<(ListOffset, Pixels)>,
}

impl TableCache {
    pub(crate) fn remember_scroll(&mut self, top: ListOffset, viewport_height: Pixels) {
        self.scroll = Some((top, viewport_height));
    }

    pub(crate) fn visible(&self, item: usize, window: &Window) -> Range<Pixels> {
        let (offset, height) = match self.scroll {
            Some((top, height)) if top.item_ix == item => (top.offset_in_item, height),
            Some((_, height)) => (px(0.0), height),
            None => (px(0.0), px(0.0)),
        };
        let start = offset - table_top(window);
        start - height * 0.5..start + height * 1.5
    }

    pub(crate) fn columns(
        &mut self,
        document: &Document,
        table: &TableModel,
        palette: &Palette,
        window: &Window,
    ) -> Rc<[Pixels]> {
        let text = document.text();
        let key = hash_of(&table.span.slice(text));
        if let Some((cached, widths)) = &self.table
            && *cached == key
        {
            return Rc::clone(widths);
        }
        let rem = window.rem_size();
        let font_size = metrics::em(CELL_SIZE).to_pixels(rem);
        let padding = metrics::em(CELL_SIZE * 1.2).to_pixels(rem) + px(1.0);
        let minimum = metrics::em(CELL_SIZE * 3.0).to_pixels(rem);
        let mut columns = vec![minimum; table.cols];
        let mut next = HashMap::with_capacity(table.rows.len());
        for (index, table_row) in table.rows.iter().enumerate() {
            let row_key = hash_of(&(index == 0, table_row.span.slice(text)));
            let widths = self.rows.remove(&row_key).unwrap_or_else(|| {
                measure(document, table_row, index == 0, font_size, palette, window)
            });
            for (column, width) in columns.iter_mut().zip(widths.iter()) {
                *column = (*column).max(*width * WIDTH_SLACK + padding);
            }
            next.insert(row_key, widths);
        }
        self.rows = next;
        let widths: Rc<[Pixels]> = columns.into();
        self.table = Some((key, Rc::clone(&widths)));
        widths
    }
}

fn measure(
    document: &Document,
    table_row: &TableRow,
    header: bool,
    font_size: Pixels,
    palette: &Palette,
    window: &Window,
) -> Rc<[Pixels]> {
    let missing = |_: &LinkKind| false;
    let context = RunContext {
        palette,
        face: Face::Body,
        color: palette.text_primary,
        missing: &missing,
        live: true,
    };
    table_row
        .cells
        .iter()
        .map(|cell| {
            let view = render_cell(document, cell, &Reveal::default());
            if view.text.is_empty() {
                return Pixels::ZERO;
            }
            let mut runs = inline_runs(&view, 0..view.text.len(), &context);
            if header {
                for run in &mut runs {
                    run.font.weight = FontWeight(run.font.weight.0.max(FontWeight::SEMIBOLD.0));
                }
            }
            window
                .text_system()
                .layout_line(&view.text, font_size, &runs, None)
                .width
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a non-negative row index clamped to the u32 range"
)]
fn row_index(ratio: f32) -> usize {
    usize::try_from(ratio.floor().clamp(0.0, 4_000_000_000.0) as u32).unwrap_or(usize::MAX)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "table row counts stay far below the f32 mantissa"
)]
fn row_count(rows: usize) -> f32 {
    rows as f32
}

fn hash_of(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
