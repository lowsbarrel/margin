use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{
    ClipboardItem, Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Window,
    point, px,
};
use margin_workbench::run_blocking;

use crate::pdf_document::PdfDocument;
use crate::pdf_layout::page_top;
use crate::pdf_select::{Caret, edge_scroll};
use crate::pdf_text::PageText;
use crate::pdf_view::PdfView;

const AUTOSCROLL_TICK: Duration = Duration::from_millis(16);

struct Hit {
    caret: Caret,
    over_text: bool,
    text: Arc<PageText>,
}

fn hit(view: &PdfView, position: Point<Pixels>, nearest: bool) -> Option<Hit> {
    let scale = view.current_scale();
    let mut best: Option<(f32, Hit)> = None;
    let first = view.list_state.logical_scroll_top().item_ix;
    let bottom = view.list_state.viewport_bounds().bottom();
    for index in first..view.sizes.len() {
        let Some(bounds) = view.list_state.bounds_for_item(index) else {
            break;
        };
        if bounds.top() > bottom {
            break;
        }
        let (Some(text), Some(size)) = (view.cache.text(index), view.sizes.get(index)) else {
            continue;
        };
        let width = f32::from(bounds.size.width);
        let left = f32::from(bounds.left()) + size.0.mul_add(-scale, width) / 2.0;
        let top = f32::from(bounds.top()) + page_top(index);
        let x = (f32::from(position.x) - left) / scale;
        let y = (f32::from(position.y) - top) / scale;
        let distance = if y < 0.0 { -y } else { (y - size.1).max(0.0) };
        if !nearest && (distance > 0.0 || x < 0.0 || x > size.0) {
            continue;
        }
        if best
            .as_ref()
            .is_some_and(|(closest, _)| *closest <= distance)
        {
            continue;
        }
        let y = y.clamp(0.0, size.1);
        let found = Hit {
            caret: Caret {
                page: index,
                at: text.caret_at(x, y),
            },
            over_text: text.is_over_text(x, y),
            text: Arc::clone(text),
        };
        best = Some((distance, found));
    }
    best.map(|(_, found)| found)
}

pub(crate) fn on_mouse_down(
    view: &mut PdfView,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    view.focus_handle.focus(window, cx);
    match hit(view, event.position, false) {
        Some(found) if event.click_count >= 3 => {
            let line = found.text.line_at(found.caret.at);
            view.selection.select(found.caret.page, line);
        }
        Some(found) if event.click_count == 2 => {
            let word = found.text.word_at(found.caret.at);
            view.selection.select(found.caret.page, word);
        }
        Some(found) => view.selection.begin(found.caret),
        None => view.selection.clear(),
    }
    cx.notify();
}

pub(crate) fn on_mouse_move(
    view: &mut PdfView,
    event: &MouseMoveEvent,
    _: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    if view.selection.is_dragging() {
        return;
    }
    let over_text = hit(view, event.position, false).is_some_and(|found| found.over_text);
    if view.selection.set_hovering_text(over_text) {
        cx.notify();
    }
}

pub(crate) fn drag_to(view: &mut PdfView, event: &MouseMoveEvent, cx: &mut Context<'_, PdfView>) {
    if !view.selection.is_dragging() {
        return;
    }
    if event.pressed_button != Some(MouseButton::Left) {
        view.selection.end();
        cx.notify();
        return;
    }
    view.selection
        .set_pointer((f32::from(event.position.x), f32::from(event.position.y)));
    if let Some(found) = hit(view, event.position, true)
        && view.selection.extend(found.caret)
    {
        cx.notify();
    }
    if edge_speed(view).abs() > f32::EPSILON && view.selection.start_autoscroll() {
        autoscroll(cx);
    }
}

pub(crate) fn on_mouse_up(view: &mut PdfView, cx: &mut Context<'_, PdfView>) {
    if view.selection.end() {
        cx.notify();
    }
}

fn edge_speed(view: &PdfView) -> f32 {
    let Some((_, y)) = view.selection.pointer() else {
        return 0.0;
    };
    let bounds = view.list_state.viewport_bounds();
    edge_scroll(y, f32::from(bounds.top()), f32::from(bounds.bottom()))
}

fn autoscroll(cx: &Context<'_, PdfView>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(AUTOSCROLL_TICK).await;
            let scrolling = this
                .update(cx, |view, cx| {
                    let speed = edge_speed(view);
                    let pointer = view.selection.pointer();
                    if speed.abs() < f32::EPSILON || !view.selection.is_dragging() {
                        view.selection.stop_autoscroll();
                        return false;
                    }
                    view.list_state.scroll_by(px(speed));
                    if let Some((x, y)) = pointer
                        && let Some(found) = hit(view, point(px(x), px(y)), true)
                    {
                        view.selection.extend(found.caret);
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !scrolling {
                break;
            }
        }
    })
    .detach();
}

pub(crate) fn copy_text(
    document: Arc<PdfDocument>,
    (start, end): (Caret, Caret),
    cx: &Context<'_, PdfView>,
) {
    let task = run_blocking(cx, move || {
        (start.page..=end.page)
            .map(|page| {
                let text = document.page_text(page);
                let to = if page == end.page {
                    end.at.min(text.len())
                } else {
                    text.len()
                };
                let from = if page == start.page {
                    start.at.min(to)
                } else {
                    0
                };
                text.slice(from..to)
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    cx.spawn(async move |_, cx| {
        let text = task.await;
        cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(text)));
    })
    .detach();
}
