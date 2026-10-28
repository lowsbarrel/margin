use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Styled, div, rems};
use margin_i18n::t;
use margin_theme::{MarginTheme, TextScale};

use crate::logo_mark::{animated_logo_mark, logo_mark};

const BRAND_LOGO_REMS: f32 = 5.7143;
const INTRO_LOGO_REMS: f32 = 3.8571;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Brand {
    WithTagline,
    Plain,
}

pub(crate) fn brand(kind: Brand, cx: &App) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .mb_8()
        .items_center()
        .child(
            div()
                .mb_3()
                .child(logo_mark(rems(BRAND_LOGO_REMS), palette.logo_mark)),
        )
        .child(
            div()
                .text_ui_2xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .child(t(cx, "app_name")),
        )
        .when(kind == Brand::WithTagline, |this| {
            this.child(
                div()
                    .mt_1p5()
                    .text_ui_sm()
                    .italic()
                    .text_color(palette.text_tertiary)
                    .child(t(cx, "app_tagline")),
            )
        })
}

pub(crate) fn intro_brand(cx: &App) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .items_center()
        .gap_1()
        .child(animated_logo_mark(rems(INTRO_LOGO_REMS), palette.logo_mark))
        .child(
            div()
                .text_ui_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .child(t(cx, "app_name")),
        )
}

pub(crate) fn auto_opening(cx: &App) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .items_center()
        .gap_6()
        .child(brand(Brand::Plain, cx))
        .child(
            div()
                .text_ui_sm()
                .italic()
                .text_color(palette.text_tertiary)
                .child(t(cx, "login_auto_opening")),
        )
        .into_any_element()
}

pub(crate) fn card(cx: &App) -> Div {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .w_full()
        .p_6()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .shadow_lg()
}
