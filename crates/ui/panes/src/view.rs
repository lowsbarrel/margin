use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, deferred, div, px, relative,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_workbench::DraggedFiles;

use crate::drag::{DividerDrag, Dragged, DraggedTab};
use crate::panes::{PaneItem, Panes};
use crate::strip::render_strip;
use crate::zones::render_zones;

const DIVIDER_REACH: f32 = 2.5;

impl<T: PaneItem> Render for Panes<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.dragging = None;
            self.strip_target = None;
            self.resizing = None;
        }
        let palette = MarginTheme::palette(cx).clone();
        let geometry = Rc::clone(&self.geometry);
        let mut children = Vec::new();
        for index in 0..self.layout.panes().len() {
            if index > 0 {
                children.push(divider(index - 1, self.resizing, &palette));
            }
            children.push(render_pane(self, index, &palette, cx));
        }
        let row = h_flex()
            .on_children_prepainted(move |bounds, _, _| geometry.borrow_mut().row = bounds)
            .id("panes")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .track_focus(&self.focus)
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedTab>, _, cx| {
                    let dragged = event
                        .dragged_item()
                        .downcast_ref::<DraggedTab>()
                        .and_then(|payload| this.dragged_tab(payload));
                    this.drag_moved(dragged, event.event.position, cx);
                }),
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedFiles>, _, cx| {
                    let dragged = event
                        .dragged_item()
                        .downcast_ref::<DraggedFiles>()
                        .map(Dragged::files);
                    this.drag_moved(dragged, event.event.position, cx);
                }),
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DividerDrag>, _, cx| {
                    if let Some(DividerDrag(divider)) = event.dragged_item().downcast_ref() {
                        this.resize_divider(*divider, event.event.position, cx);
                    }
                }),
            )
            .children(children);
        if self.resizing.is_some() {
            row.cursor_col_resize()
        } else {
            row
        }
    }
}

fn render_pane<T: PaneItem>(
    this: &Panes<T>,
    index: usize,
    palette: &Palette,
    cx: &Context<'_, Panes<T>>,
) -> AnyElement {
    let entry = &this.layout.panes()[index];
    let id = entry.id;
    let geometry = Rc::clone(&this.geometry);
    let content = entry.active_tab().map_or_else(
        || {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_ui_sm()
                .text_color(palette.text_tertiary)
                .child(t(cx, "editor_empty_state"))
                .into_any_element()
        },
        |tab| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(tab.item.handle().view())
                .into_any_element()
        },
    );
    let mut pane = v_flex()
        .relative()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .bg(palette.surface(0))
        .flex_basis(relative(0.0));
    pane.style().flex_grow = Some(this.layout.flexes()[index]);
    pane.style().flex_shrink = Some(1.0);
    pane.on_children_prepainted(move |bounds, _, _| {
        if let Some(strip) = bounds.first() {
            geometry.borrow_mut().strips.insert(id, *strip);
        }
    })
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, _, window, cx| this.focus_pane(index, window, cx)),
    )
    .child(render_strip(this, index, cx))
    .child(content)
    .children(render_zones(this, index, cx))
    .into_any_element()
}

fn divider(index: usize, resizing: Option<usize>, palette: &Palette) -> AnyElement {
    let hover = palette.brand(25.0);
    let handle = div()
        .id(("pane-divider", index))
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(-DIVIDER_REACH))
        .right(px(-DIVIDER_REACH))
        .cursor_col_resize()
        .on_drag(DividerDrag(index), |drag, _, _, cx| cx.new(|_| *drag));
    let handle = if resizing == Some(index) {
        handle.bg(palette.brand(50.0))
    } else {
        handle.hover(move |style| style.bg(hover))
    };
    div()
        .relative()
        .flex_none()
        .w(px(1.0))
        .h_full()
        .bg(palette.border_secondary)
        .child(deferred(handle).with_priority(1))
        .into_any_element()
}
