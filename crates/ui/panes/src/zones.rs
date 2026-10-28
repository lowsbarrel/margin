use gpui_kit::component::h_flex;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Stateful, Styled, div, px, relative,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_workbench::DraggedFiles;

use crate::drag::{DraggedTab, DropZone, ZoneMode, zone_mode};
use crate::panes::{PaneItem, Panes};
use crate::strip::STRIP_HEIGHT;

const FILE_ZONE_WIDTH: f32 = 56.0;
const CENTER_GROW: f32 = 40.0;
const SIDE_GROW: f32 = 30.0;

pub(crate) fn render_zones<T: PaneItem>(
    this: &Panes<T>,
    pane: usize,
    cx: &Context<'_, Panes<T>>,
) -> Option<AnyElement> {
    let dragged = this.dragging?;
    let mode = zone_mode(dragged, pane, this.layout.panes()[pane].tabs.len());
    if mode == ZoneMode::None {
        return None;
    }
    let palette = MarginTheme::palette(cx).clone();
    let file_drag = dragged.is_files();
    let mut overlay = h_flex()
        .absolute()
        .top(STRIP_HEIGHT)
        .left_0()
        .right_0()
        .bottom_0();
    overlay = if file_drag {
        overlay.justify_between()
    } else {
        overlay.bg(palette.brand(6.0))
    };
    if this.strip_gap(pane).is_none() {
        overlay = overlay.child(zone(pane, DropZone::Left, file_drag, &palette, cx));
        if mode == ZoneMode::All {
            overlay = overlay.child(zone(pane, DropZone::Center, file_drag, &palette, cx));
        }
        overlay = overlay.child(zone(pane, DropZone::Right, file_drag, &palette, cx));
    }
    Some(overlay.into_any_element())
}

fn zone<T: PaneItem>(
    pane: usize,
    zone: DropZone,
    file_drag: bool,
    palette: &Palette,
    cx: &Context<'_, Panes<T>>,
) -> Stateful<Div> {
    let group = SharedString::from(format!("pane-zone-{pane}-{zone:?}"));
    let (label, fill) = match zone {
        DropZone::Left => ("pane_split_left", palette.brand(22.0)),
        DropZone::Center => ("pane_move_here", palette.brand(22.0)),
        DropZone::Right => ("pane_split_right", palette.brand(22.0)),
    };
    let mut element = div()
        .id(group.clone())
        .group(group.clone())
        .flex()
        .h_full()
        .items_center()
        .justify_center();
    element = match (zone, file_drag) {
        (DropZone::Center, _) => {
            grow(element, CENTER_GROW).drag_over::<DraggedTab>(move |style, _, _, _| style.bg(fill))
        }
        (side, true) => {
            let (rest, active) = (palette.brand(30.0), palette.bg_brand);
            let file_fill = palette.brand(18.0);
            let element = element
                .w(px(FILE_ZONE_WIDTH))
                .flex_none()
                .border_color(rest)
                .drag_over::<DraggedFiles>(move |style, _, _, _| {
                    style.bg(file_fill).border_color(active)
                });
            if side == DropZone::Left {
                element.border_r_2()
            } else {
                element.border_l_2()
            }
        }
        (side, false) => {
            let element = grow(element, SIDE_GROW)
                .border_dashed()
                .border_color(palette.brand(40.0))
                .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(fill));
            if side == DropZone::Left {
                element.border_r_1()
            } else {
                element.border_l_1()
            }
        }
    };
    element
        .on_drop(cx.listener(move |this, payload: &DraggedTab, window, cx| {
            cx.stop_propagation();
            this.drop_tab_on_zone(payload, pane, zone, window, cx);
        }))
        .on_drop(
            cx.listener(move |this, payload: &DraggedFiles, window, cx| {
                cx.stop_propagation();
                this.drop_files_on_zone(payload, pane, zone, window, cx);
            }),
        )
        .child(zone_label(t(cx, label), &group, palette))
}

fn grow(mut element: Stateful<Div>, factor: f32) -> Stateful<Div> {
    element.style().flex_grow = Some(factor);
    element.style().flex_shrink = Some(1.0);
    element.flex_basis(relative(0.0))
}

fn zone_label(label: SharedString, group: &SharedString, palette: &Palette) -> Div {
    div()
        .rounded(px(4.0))
        .border_1()
        .border_color(palette.brand(50.0))
        .bg(palette.surface(1))
        .px_2()
        .py(px(3.0))
        .text_ui_xs()
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .text_color(palette.text_brand)
        .opacity(0.0)
        .cursor_default()
        .group_drag_over::<DraggedTab>(group.clone(), |style| style.opacity(1.0))
        .group_drag_over::<DraggedFiles>(group.clone(), |style| style.opacity(1.0))
        .child(label)
}
