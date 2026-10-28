use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu};
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Rems, StatefulInteractiveElement, Styled, Window, div, relative,
    rems,
};
use margin_i18n::t;
use margin_markdown::PropertyKind;
use margin_theme::Palette;

use crate::effect::{CaretAt, Effect, FocusAfter};
use crate::panel_actions::{
    ADD_CONTEXT, Activate, ActivateAtEnd, ActivateAtStart, AddProperty, Cancel, FocusNext,
    FocusPrevious, HEADING_CONTEXT, KEY_CONTEXT, LeaveToBody, MoveDown, MoveLeft, MoveRight,
    MoveUp, ROW_CONTEXT, Remove,
};
use crate::properties_event::PropertiesEvent;
use crate::properties_panel::PropertiesPanel;
use crate::property_menus::{heading_menu, type_label, type_menu};
use crate::property_row::{Row, kind_icon, row_listener};

const PANEL_REM: f32 = 13.125 / 14.0;

pub(crate) fn em(factor: f32) -> Rems {
    rems(PANEL_REM * factor)
}

fn focus_ring(element: Div, focused: bool, palette: &Palette) -> Div {
    element.border_2().border_color(if focused {
        palette.bg_brand
    } else {
        gpui_kit::transparent_black()
    })
}

fn error_line(
    message: Option<&'static str>,
    palette: &Palette,
    cx: &Context<'_, PropertiesPanel>,
) -> Option<AnyElement> {
    let message = message?;
    Some(
        div()
            .px_2()
            .pb_1()
            .text_xs()
            .text_color(palette.text_negative)
            .child(t(cx, message))
            .into_any_element(),
    )
}

pub(crate) fn heading(
    focus: &FocusHandle,
    collapsed: bool,
    palette: &Palette,
    window: &Window,
    cx: &Context<'_, PropertiesPanel>,
) -> AnyElement {
    let panel = cx.entity();
    let fold = Icon::new(if collapsed {
        IconName::ChevronRight
    } else {
        IconName::ChevronDown
    })
    .size(em(1.143))
    .text_color(palette.text_tertiary);
    focus_ring(div(), focus.is_focused(window), palette)
        .id("properties-heading")
        .group("properties-heading")
        .track_focus(focus)
        .key_context(HEADING_CONTEXT)
        .relative()
        .flex()
        .flex_none()
        .self_start()
        .items_center()
        .mb(em(0.57))
        .ml(em(-0.29))
        .p(em(0.29))
        .rounded_lg()
        .text_size(em(1.143))
        .line_height(relative(1.2))
        .text_color(palette.text_primary)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .on_click(cx.listener(move |panel, _, window, cx| panel.fold(!collapsed, window, cx)))
        .on_action(cx.listener(|panel, _: &MoveLeft, window, cx| panel.fold(true, window, cx)))
        .on_action(cx.listener(|panel, _: &MoveRight, window, cx| panel.fold(false, window, cx)))
        .on_action(
            cx.listener(move |panel, _: &Activate, window, cx| panel.fold(!collapsed, window, cx)),
        )
        .on_action(cx.listener(|_, _: &MoveUp, _, cx| cx.emit(PropertiesEvent::FocusTitle)))
        .on_action(cx.listener(|panel, _: &MoveDown, window, cx| panel.heading_down(window, cx)))
        .child(
            div()
                .absolute()
                .left(em(-1.25 * 1.143))
                .flex()
                .items_center()
                .when(!collapsed, |icon| {
                    icon.invisible()
                        .group_hover("properties-heading", Styled::visible)
                })
                .child(fold),
        )
        .child(t(cx, "editor_properties"))
        .context_menu(move |menu, _, cx| heading_menu(menu, &panel, cx))
        .into_any_element()
}

fn focus_value_at<E: 'static>(
    key: &str,
    at: CaretAt,
    cx: &Context<'_, PropertiesPanel>,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    row_listener(key, cx, move |row, _: &E, window, cx| {
        row.focus_value(at, window, cx);
        Effect::Nothing
    })
}

fn step<E: 'static>(
    index: usize,
    offset: isize,
    cx: &Context<'_, PropertiesPanel>,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    cx.listener(move |panel: &mut PropertiesPanel, _: &E, window, cx| {
        panel.row_at(index, offset, window, cx);
    })
}

fn key_cell(
    row: &Row,
    index: usize,
    palette: &Palette,
    cx: &Context<'_, PropertiesPanel>,
) -> AnyElement {
    let key = row.key().to_owned();
    let panel = cx.entity();
    let kind = row.kind();
    let value = row.property().value.clone();
    let menu_key = key.clone();
    div()
        .flex()
        .flex_none()
        .items_center()
        .w(em(10.3))
        .h(em(2.07))
        .child(
            Button::new(("kind", index))
                .ghost()
                .xsmall()
                .icon(Icon::new(kind_icon(kind)))
                .tooltip(t(cx, type_label(kind)))
                .dropdown_menu(move |menu, _, cx| {
                    type_menu(menu, &panel, &menu_key, kind, &value, cx)
                }),
        )
        .child(
            div()
                .key_context(KEY_CONTEXT)
                .flex_1()
                .min_w_0()
                .text_color(palette.text_secondary)
                .on_action(row_listener(&key, cx, |row, _: &Cancel, window, cx| {
                    row.cancel_key(window, cx)
                }))
                .on_action(row_listener(&key, cx, |row, _: &FocusNext, _, cx| {
                    row.submit_key(cx)
                }))
                .child(row.render_key()),
        )
        .into_any_element()
}

pub(crate) fn row_view(
    row: &Row,
    index: usize,
    error: Option<&'static str>,
    palette: &Palette,
    window: &Window,
    cx: &Context<'_, PropertiesPanel>,
) -> AnyElement {
    let key = row.key();
    let focused = row.focus_handle().is_focused(window);
    let within = focused || row.focus_handle().contains_focused(window, cx);
    let line = div()
        .id(("property", index))
        .track_focus(row.focus_handle())
        .when(focused, |line| line.key_context(ROW_CONTEXT))
        .flex()
        .items_start()
        .min_h(em(2.07))
        .rounded_lg()
        .border_1()
        .border_color(if within {
            palette.border_strong
        } else {
            gpui_kit::transparent_black()
        })
        .hover(|line| line.bg(palette.surface(3)))
        .on_action(step::<MoveUp>(index, -1, cx))
        .on_action(step::<FocusPrevious>(index, -1, cx))
        .on_action(step::<MoveDown>(index, 1, cx))
        .on_action(step::<FocusNext>(index, 1, cx))
        .on_action(row_listener(key, cx, |row, _: &MoveLeft, window, cx| {
            row.focus_key(window, cx);
            Effect::Nothing
        }))
        .on_action(focus_value_at::<MoveRight>(key, CaretAt::All, cx))
        .on_action(focus_value_at::<Activate>(key, CaretAt::All, cx))
        .on_action(focus_value_at::<ActivateAtEnd>(key, CaretAt::End, cx))
        .on_action(focus_value_at::<ActivateAtStart>(key, CaretAt::Start, cx))
        .on_action(cx.listener(|panel, _: &AddProperty, window, cx| panel.start_adding(window, cx)))
        .on_action(row_listener(key, cx, |_, _: &Remove, _, _| Effect::Remove))
        .on_action(cx.listener(|_, _: &LeaveToBody, _, cx| cx.emit(PropertiesEvent::FocusBody)))
        .child(key_cell(row, index, palette, cx))
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .min_w_0()
                .min_h(em(2.07))
                .text_color(palette.text_primary)
                .child(row.render_value(window, cx)),
        );
    div()
        .flex()
        .flex_col()
        .child(line)
        .children(error_line(error, palette, cx))
        .into_any_element()
}

pub(crate) fn pending_view(
    input: &Entity<InputState>,
    error: Option<&'static str>,
    palette: &Palette,
    cx: &Context<'_, PropertiesPanel>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .min_h(em(2.07))
                .w(em(10.3))
                .gap_1()
                .px_1()
                .key_context(KEY_CONTEXT)
                .text_color(palette.text_secondary)
                .on_action(cx.listener(|panel, _: &Cancel, window, cx| {
                    panel.dismiss_new(FocusAfter::Row, window, cx);
                }))
                .on_action(cx.listener(|panel, _: &FocusNext, window, cx| {
                    panel.commit_new(FocusAfter::Value, window, cx);
                }))
                .child(
                    Icon::new(kind_icon(PropertyKind::Text))
                        .size_4()
                        .text_color(palette.text_tertiary),
                )
                .child(Input::new(input).appearance(false).small()),
        )
        .children(error_line(error, palette, cx))
        .into_any_element()
}

pub(crate) fn add_button(
    focus: &FocusHandle,
    palette: &Palette,
    window: &Window,
    cx: &Context<'_, PropertiesPanel>,
) -> AnyElement {
    let focused = focus.is_focused(window);
    focus_ring(div(), focused, palette)
        .id("properties-add")
        .track_focus(focus)
        .key_context(ADD_CONTEXT)
        .flex()
        .items_center()
        .gap(em(0.29))
        .mt(em(0.5))
        .pl(em(0.43))
        .pr(em(0.35))
        .py(em(0.29))
        .rounded_lg()
        .cursor_pointer()
        .text_color(if focused {
            palette.text_primary
        } else {
            palette.text_secondary
        })
        .when(focused, |add| add.bg(palette.surface(3)))
        .hover(|add| add.bg(palette.surface(3)).text_color(palette.text_primary))
        .on_click(cx.listener(|panel, _, window, cx| panel.start_adding(window, cx)))
        .on_action(cx.listener(|panel, _: &Activate, window, cx| panel.start_adding(window, cx)))
        .on_action(cx.listener(|panel, _: &MoveUp, window, cx| panel.add_up(window, cx)))
        .on_action(cx.listener(|_, _: &MoveDown, _, cx| cx.emit(PropertiesEvent::FocusBody)))
        .child(Icon::new(IconName::Plus).size(em(1.29)))
        .child(t(cx, "editor_property_add"))
        .into_any_element()
}
