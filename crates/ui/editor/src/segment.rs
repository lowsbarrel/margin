use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{
    AnyElement, Bounds, Hsla, IntoElement, ParentElement, Pixels, SharedString, Styled, StyledText,
    TextLayout, TextRun, Window, canvas, div, fill, point, px, size,
};
use margin_live_preview::{LinkHit, OffsetMap};
use margin_markdown::Span;
use margin_text_buffer::Selection;

use crate::hit_map::{HitMap, Segment};

pub(crate) struct Overlay {
    selections: Vec<Selection>,
    primary: usize,
    focused: bool,
    matches: Vec<Span>,
    current_match: Option<Span>,
    colors: OverlayColors,
}

#[derive(Clone, Copy)]
pub(crate) struct OverlayColors {
    caret: Hsla,
    selection: Hsla,
    found: Hsla,
    current: Hsla,
}

impl OverlayColors {
    pub(crate) const fn new(caret: Hsla, selection: Hsla, found: Hsla, current: Hsla) -> Self {
        Self {
            caret,
            selection,
            found,
            current,
        }
    }
}

impl Overlay {
    pub(crate) const fn new(
        selections: Vec<Selection>,
        primary: usize,
        focused: bool,
        matches: Vec<Span>,
        current_match: Option<Span>,
        colors: OverlayColors,
    ) -> Self {
        Self {
            selections,
            primary,
            focused,
            matches,
            current_match,
            colors,
        }
    }
}

pub(crate) fn text_segment(
    text: SharedString,
    runs: Vec<TextRun>,
    map: Rc<OffsetMap>,
    display: Range<usize>,
    links: Rc<[LinkHit]>,
    overlay: Rc<Overlay>,
    hits: HitMap,
) -> AnyElement {
    let styled = StyledText::new(text).with_runs(runs);
    let layout = styled.layout().clone();
    div()
        .relative()
        .min_w_0()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    hits.push(Segment::new(
                        layout.clone(),
                        Rc::clone(&map),
                        display.clone(),
                        bounds,
                        Rc::clone(&links),
                    ));
                    paint_overlay(&layout, &map, &display, bounds, &overlay, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(styled)
        .into_any_element()
}

fn paint_overlay(
    layout: &TextLayout,
    map: &OffsetMap,
    display: &Range<usize>,
    bounds: Bounds<Pixels>,
    overlay: &Overlay,
    window: &mut Window,
) {
    for found in &overlay.matches {
        let color = if overlay.current_match == Some(*found) {
            overlay.colors.current
        } else {
            overlay.colors.found
        };
        paint_range(layout, map, display, bounds, *found, color, window);
    }
    for (index, selection) in overlay.selections.iter().enumerate() {
        if selection.is_empty() {
            if overlay.focused {
                let color = if index == overlay.primary {
                    overlay.colors.caret
                } else {
                    overlay.colors.caret.opacity(0.6)
                };
                paint_caret(layout, map, display, selection.head, color, window);
            }
        } else {
            paint_range(
                layout,
                map,
                display,
                bounds,
                selection.span(),
                overlay.colors.selection,
                window,
            );
        }
    }
}

fn local_index(map: &OffsetMap, display: &Range<usize>, offset: usize) -> Option<usize> {
    if offset < map.source_start() || offset > map.source_end() {
        return None;
    }
    let at = map.to_display(offset);
    (display.start..=display.end)
        .contains(&at)
        .then(|| at - display.start)
}

fn paint_caret(
    layout: &TextLayout,
    map: &OffsetMap,
    display: &Range<usize>,
    offset: usize,
    color: Hsla,
    window: &mut Window,
) {
    let Some(index) = local_index(map, display, offset) else {
        return;
    };
    let Some(origin) = layout.position_for_index(index) else {
        return;
    };
    window.paint_quad(fill(
        Bounds::new(origin, size(px(2.0), layout.line_height())),
        color,
    ));
}

fn paint_range(
    layout: &TextLayout,
    map: &OffsetMap,
    display: &Range<usize>,
    bounds: Bounds<Pixels>,
    span: Span,
    color: Hsla,
    window: &mut Window,
) {
    if span.end < map.source_start() || span.start > map.source_end() {
        return;
    }
    let start = span.start.max(map.source_start());
    let end = span.end.min(map.source_end());
    let from = map.to_display(start).clamp(display.start, display.end) - display.start;
    let to = map.to_display(end).clamp(display.start, display.end) - display.start;
    let runs_past_end = span.end > map.source_end();
    if from == to && !runs_past_end {
        return;
    }
    let line_height = layout.line_height();
    let (Some(first), Some(last)) = (
        layout.position_for_index(from),
        layout.position_for_index(to),
    ) else {
        return;
    };
    let right = |x: Pixels| if runs_past_end { bounds.right() } else { x };
    if (first.y - last.y).abs() < px(1.0) {
        let end_x = right(last.x).max(first.x + px(4.0));
        window.paint_quad(fill(
            Bounds::from_corners(first, point(end_x, first.y + line_height)),
            color,
        ));
        return;
    }
    window.paint_quad(fill(
        Bounds::from_corners(first, point(bounds.right(), first.y + line_height)),
        color,
    ));
    if last.y > first.y + line_height {
        window.paint_quad(fill(
            Bounds::from_corners(
                point(bounds.left(), first.y + line_height),
                point(bounds.right(), last.y),
            ),
            color,
        ));
    }
    window.paint_quad(fill(
        Bounds::from_corners(
            point(bounds.left(), last.y),
            point(right(last.x), last.y + line_height),
        ),
        color,
    ));
}
