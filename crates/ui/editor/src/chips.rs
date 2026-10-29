use std::ops::Range;

use gpui_kit::{BorderStyle, Bounds, Edges, Hsla, Pixels, TextLayout, Window, point, px, quad};
use margin_live_preview::{InlineStyle, LineView};
use margin_theme::Palette;

use crate::text_style::highlight_color;

const CODE_RADIUS: Pixels = px(4.0);
const HIGHLIGHT_RADIUS: Pixels = px(2.0);
const CODE_OUTSET: Pixels = px(2.0);
const HIGHLIGHT_OUTSET: Pixels = px(1.0);
const VERTICAL_INSET: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Code,
    Highlight,
    Pill,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chip {
    range: Range<usize>,
    color: Hsla,
    shape: Shape,
}

pub(crate) fn chips_in(
    view: &LineView,
    range: Range<usize>,
    palette: &Palette,
    live: bool,
) -> Vec<Chip> {
    let mut chips: Vec<Chip> = Vec::new();
    for styled in &view.runs {
        let start = styled.range.start.max(range.start);
        let end = styled.range.end.min(range.end);
        let Some((color, shape)) = chip_of(&styled.style, palette, live) else {
            continue;
        };
        if start >= end {
            continue;
        }
        let local = start - range.start..end - range.start;
        match chips.last_mut() {
            Some(last) if last.range.end == local.start && last.color == color => {
                last.range.end = local.end;
            }
            _ => chips.push(Chip {
                range: local,
                color,
                shape,
            }),
        }
    }
    chips
}

fn chip_of(style: &InlineStyle, palette: &Palette, live: bool) -> Option<(Hsla, Shape)> {
    if let Some(tone) = style.highlight {
        return Some((highlight_color(palette, tone), Shape::Highlight));
    }
    if style.code || style.keyboard {
        return Some((palette.surface(2), Shape::Code));
    }
    (style.tag && live).then(|| (palette.brand(12.0), Shape::Pill))
}

pub(crate) fn paint_chips(layout: &TextLayout, chips: &[Chip], window: &mut Window) {
    for chip in chips {
        for row in row_bounds(layout, chip.range.clone()) {
            let inset = row.size.height * VERTICAL_INSET;
            let height = row.size.height - inset * 2.0;
            let (outset, radius) = match chip.shape {
                Shape::Code => (CODE_OUTSET, CODE_RADIUS),
                Shape::Highlight => (HIGHLIGHT_OUTSET, HIGHLIGHT_RADIUS),
                Shape::Pill => (CODE_OUTSET * 2.0, height / 2.0),
            };
            let bounds = Bounds::from_corners(
                point(row.left() - outset, row.top() + inset),
                point(row.right() + outset, row.bottom() - inset),
            );
            window.paint_quad(quad(
                bounds,
                radius,
                chip.color,
                Edges::default(),
                Hsla::transparent_black(),
                BorderStyle::default(),
            ));
        }
    }
}

fn row_bounds(layout: &TextLayout, range: Range<usize>) -> Vec<Bounds<Pixels>> {
    let bounds = layout.bounds();
    let line_height = layout.line_height();
    let mut rows = Vec::new();
    let mut top = bounds.top();
    let mut line_start = 0;
    for wrapped in layout.line_layouts() {
        let line_end = line_start + wrapped.len();
        let unwrapped = &wrapped.unwrapped_layout;
        let boundaries = wrapped.wrap_boundaries().iter().filter_map(|boundary| {
            let glyph = unwrapped
                .runs
                .get(boundary.run_ix)?
                .glyphs
                .get(boundary.glyph_ix)?;
            Some((line_start + glyph.index, glyph.position.x))
        });
        let mut row_start = line_start;
        let mut row_start_x = Pixels::ZERO;
        for (row_end, row_end_x) in boundaries.chain([(line_end, unwrapped.width)]) {
            let from = range.start.max(row_start);
            let to = range.end.min(row_end);
            if from < to {
                let x = |index: usize| {
                    bounds.left() + unwrapped.x_for_index(index - line_start) - row_start_x
                };
                rows.push(Bounds::from_corners(
                    point(x(from), top),
                    point(x(to), top + line_height),
                ));
            }
            row_start = row_end;
            row_start_x = row_end_x;
            top += line_height;
        }
        line_start = line_end + 1;
    }
    rows
}
