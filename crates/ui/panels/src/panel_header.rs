use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, Div, ElementId, IntoElement, ParentElement, SharedString, Styled, div, rems,
};
use margin_theme::{MarginTheme, TextScale};

const PLACEHOLDER_HEIGHT_REMS: f32 = 6.25;

pub(crate) fn panel_header(
    icon: IconName,
    title: &SharedString,
    count: usize,
    actions: impl IntoIterator<Item = Button>,
    cx: &App,
) -> Div {
    let palette = MarginTheme::palette(cx);
    let heading = h_flex()
        .gap_2()
        .min_w_0()
        .text_color(palette.text_tertiary)
        .child(Icon::new(icon).small())
        .child(
            div()
                .text_ui_xs()
                .font_semibold()
                .truncate()
                .child(title.to_uppercase()),
        )
        .when(count > 0, |heading| {
            heading.child(
                div()
                    .flex_shrink_0()
                    .px_1p5()
                    .py_px()
                    .rounded_full()
                    .bg(palette.surface(2))
                    .text_ui_xs()
                    .font_medium()
                    .child(count.to_string()),
            )
        });
    h_flex()
        .h_11()
        .flex_shrink_0()
        .px_3()
        .gap_2()
        .justify_between()
        .border_b_1()
        .border_color(palette.border_divider)
        .child(heading)
        .child(h_flex().gap_0p5().children(actions))
}

pub(crate) fn icon_button(id: impl Into<ElementId>, icon: IconName, label: SharedString) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .icon(icon)
        .accessibility_label(label.clone())
        .tooltip(label)
}

pub(crate) fn panel_placeholder(text: SharedString, cx: &App) -> Div {
    h_flex()
        .h(rems(PLACEHOLDER_HEIGHT_REMS))
        .px_3()
        .items_center()
        .justify_center()
        .text_center()
        .text_ui_sm()
        .text_color(MarginTheme::palette(cx).text_tertiary)
        .child(text)
}

pub(crate) fn centered_message(
    icon: IconName,
    title: SharedString,
    hint: SharedString,
    cx: &App,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .h_40()
        .items_center()
        .justify_center()
        .gap_2()
        .text_color(palette.text_tertiary)
        .child(Icon::new(icon).size_5())
        .child(
            div()
                .text_ui_sm()
                .text_color(palette.text_primary)
                .child(title),
        )
        .child(div().text_ui_xs().max_w_80().truncate().child(hint))
        .into_any_element()
}
