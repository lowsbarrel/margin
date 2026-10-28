use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Anchor, AnyElement, App, ClickEvent, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rems,
};
use margin_i18n::t;
use margin_theme::MarginTheme;

use crate::canvas_view::{CanvasView, ToolbarPopover};
use crate::color;
use crate::controls::{
    icon_button, popover_panel, shape_icon, shape_label, swatch, tool_button, tool_foreground,
};
use crate::model::{INK_COLOR, ShapeKind};
use crate::tool::{COLOR_PRESETS, StrokeWidth, Tool};

const SWATCH: f32 = 1.5;
const POPOVER_GAP: f32 = 8.0;

fn pick_tool(
    view: &Entity<CanvasView>,
    tool: Tool,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let view = view.downgrade();
    move |_, window, cx| {
        drop(view.update(cx, |view, cx| view.choose_tool(tool, window, cx)));
    }
}

fn popover_toggle(
    view: &Entity<CanvasView>,
    which: ToolbarPopover,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let view = view.downgrade();
    move |open, _, cx| {
        drop(view.update(cx, |view, cx| {
            view.open_popover = open.then_some(which);
            cx.notify();
        }));
    }
}

fn shape_picker(view: &Entity<CanvasView>, current: ShapeKind, cx: &App) -> AnyElement {
    let mut row = h_flex().gap_0p5().p_0p5();
    for kind in ShapeKind::ALL {
        row = row.child(
            tool_button(
                SharedString::from(format!("canvas-shape-{}", kind.as_str())),
                shape_icon(kind),
                shape_label(kind),
                kind == current,
                cx,
            )
            .on_click(pick_tool(view, Tool::Shape(kind))),
        );
    }
    popover_panel(cx).child(row).into_any_element()
}

fn color_swatch(view: &Entity<CanvasView>, preset: &'static str, cx: &App) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let selected = view.read(cx).board.color == preset;
    let tooltip = if preset == INK_COLOR {
        t(cx, "canvas_color_ink")
    } else {
        SharedString::from(preset)
    };
    let handle = view.downgrade();
    let group = SharedString::from(format!("canvas-color-{preset}"));
    let scale = if selected { 1.15 } else { 1.0 };
    div()
        .size(rems(SWATCH))
        .m(px(2.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id(group.clone())
                .group(group.clone())
                .flex_none()
                .rounded_full()
                .border_2()
                .border_color(if selected {
                    palette.text_primary
                } else {
                    gpui_kit::transparent_black()
                })
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .child(
                    div()
                        .size(rems(SWATCH * scale))
                        .when(!selected, |this| {
                            this.group_hover(group, |style| style.size(rems(SWATCH * 1.2)))
                        })
                        .child(swatch(color::resolve(preset, palette), cx)),
                )
                .on_click(move |_, _, cx| {
                    drop(handle.update(cx, |view, cx| {
                        preset.clone_into(&mut view.board.color);
                        cx.notify();
                    }));
                }),
        )
        .into_any_element()
}

fn style_picker(view: &Entity<CanvasView>, cx: &App) -> AnyElement {
    let board = &view.read(cx).board;
    let palette = MarginTheme::palette(cx);
    let tool = board.tool;
    let active = StrokeWidth::nearest(tool, board.sizes.current(tool));
    let mut swatches = h_flex().gap_1().px_1().py_0p5();
    for preset in COLOR_PRESETS {
        swatches = swatches.child(color_swatch(view, preset, cx));
    }
    let mut widths = h_flex().gap_0p5().p_0p5();
    for width in StrokeWidth::ALL {
        let handle = view.downgrade();
        let selected = width == active;
        widths = widths.child(
            icon_button(
                SharedString::from(format!("canvas-width-{}", width as u8)),
                selected,
                cx,
            )
            .tooltip(t(cx, width.message_key()))
            .child(
                div()
                    .w(rems(1.125))
                    .h(px(width.bar()))
                    .rounded_full()
                    .bg(tool_foreground(selected, cx)),
            )
            .on_click(move |_, _, cx| {
                drop(handle.update(cx, |view, cx| {
                    let tool = view.board.tool;
                    view.board.sizes.set(tool, width.value(tool));
                    cx.notify();
                }));
            }),
        );
    }
    popover_panel(cx)
        .child(swatches)
        .child(div().my_1().h(px(1.0)).bg(palette.border_secondary))
        .child(widths)
        .into_any_element()
}

pub(crate) fn toolbar(view: &CanvasView, cx: &Context<'_, CanvasView>) -> impl IntoElement {
    let entity = cx.entity();
    let palette = MarginTheme::palette(cx).clone();
    let tool = view.board.tool;
    let shape = match tool {
        Tool::Shape(kind) => kind,
        _ => view.board.last_shape,
    };
    let picker_view = entity.clone();
    let style_view = entity.clone();
    let style_open = view.open_popover == Some(ToolbarPopover::Style);
    let shape_popover = Popover::new("canvas-shape-popover")
        .anchor(Anchor::BottomCenter)
        .offset(px(POPOVER_GAP))
        .appearance(false)
        .open(view.open_popover == Some(ToolbarPopover::Shape))
        .on_open_change(popover_toggle(&entity, ToolbarPopover::Shape))
        .trigger(tool_button(
            "canvas-shape",
            shape_icon(shape),
            shape_label(shape),
            matches!(tool, Tool::Shape(_)),
            cx,
        ))
        .content(move |_, _, cx| shape_picker(&picker_view, shape, cx));
    let style_popover = Popover::new("canvas-style-popover")
        .anchor(Anchor::BottomCenter)
        .offset(px(POPOVER_GAP))
        .appearance(false)
        .open(style_open)
        .on_open_change(popover_toggle(&entity, ToolbarPopover::Style))
        .trigger(
            icon_button("canvas-style", style_open, cx)
                .tooltip(t(cx, "canvas_color"))
                .child(
                    div()
                        .size(px(18.0))
                        .child(swatch(color::resolve(&view.board.color, &palette), cx)),
                ),
        )
        .content(move |_, _, cx| style_picker(&style_view, cx));
    let simple = |id: &'static str, icon: IconName, label: &'static str, which: Tool| {
        tool_button(id, icon, label, tool == which, cx).on_click(pick_tool(&entity, which))
    };
    h_flex()
        .id("canvas-toolbar")
        .gap_0p5()
        .px_1p5()
        .py_1()
        .rounded_full()
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .shadow_lg()
        .occlude()
        .child(simple(
            "canvas-hand",
            IconName::Hand,
            "canvas_hand",
            Tool::Hand,
        ))
        .child(simple(
            "canvas-pen",
            IconName::Pencil,
            "canvas_pen",
            Tool::Pen,
        ))
        .child(simple(
            "canvas-eraser",
            IconName::Eraser,
            "canvas_eraser",
            Tool::Eraser,
        ))
        .child(shape_popover)
        .child(simple(
            "canvas-text",
            IconName::Type,
            "canvas_text",
            Tool::Text,
        ))
        .child(
            div()
                .mx_1()
                .h_5()
                .w(px(1.0))
                .flex_shrink_0()
                .bg(palette.border_secondary),
        )
        .child(style_popover)
}
