use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{App, Div, ParentElement, Styled, div};
use margin_i18n::t;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};

pub(crate) fn menu_header(title: &str, filter: &str, cx: &App) -> Div {
    let palette = MarginTheme::palette(cx);
    h_flex()
        .flex_none()
        .min_h_6()
        .gap_2()
        .px_2p5()
        .pb_1p5()
        .border_b_1()
        .border_color(palette.border_secondary)
        .text_ui_xs()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(palette.text_tertiary)
                .child(title.to_owned()),
        )
        .when(!filter.is_empty(), |header| {
            header.child(
                div()
                    .flex_none()
                    .font_family(MONO_FONT_FAMILY)
                    .text_color(palette.text_secondary)
                    .child(filter.to_owned()),
            )
        })
}

pub(crate) fn menu_empty(filtering: bool, cx: &App) -> Div {
    let key = if filtering {
        "breadcrumb_no_matches"
    } else {
        "breadcrumb_empty"
    };
    div()
        .px_2p5()
        .py_2()
        .text_ui_sm()
        .text_color(MarginTheme::palette(cx).text_tertiary)
        .child(t(cx, key))
}

pub(crate) fn menu_footer(cx: &App) -> Div {
    let palette = MarginTheme::palette(cx);
    div()
        .flex_none()
        .px_2p5()
        .pt_1p5()
        .border_t_1()
        .border_color(palette.border_secondary)
        .text_ui_xs()
        .text_color(palette.text_tertiary)
        .child(t(cx, "breadcrumb_filter_hint"))
}
