use std::f32::consts::FRAC_PI_2;

use gpui_kit::assets::IconName;
use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, ClickEvent, CursorStyle, Div, FontWeight, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, radians,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, TextScale};

use crate::settings_section::SettingsSection;

pub(crate) fn card(
    section: SettingsSection,
    open: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    body: impl FnOnce(Div) -> Div,
    cx: &App,
) -> Div {
    let palette = MarginTheme::palette(cx);
    let chevron = Icon::new(IconName::ChevronRight)
        .small()
        .text_color(palette.text_tertiary)
        .when(open, |icon| icon.rotate(radians(FRAC_PI_2)));
    let header = h_flex()
        .id(section.title_key())
        .justify_between()
        .px_4()
        .py_3()
        .bg(palette.surface(1))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(palette.surface(2)))
        .when(open, |header| {
            header.border_b_1().border_color(palette.border_secondary)
        })
        .on_click(on_toggle)
        .child(
            h_flex()
                .gap_2()
                .text_ui_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .child(
                    Icon::new(section.icon())
                        .small()
                        .text_color(palette.text_tertiary),
                )
                .child(t(cx, section.title_key())),
        )
        .child(chevron)
        .test_support();
    v_flex()
        .flex_none()
        .overflow_hidden()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .child(header)
        .when(open, |card| card.child(body(v_flex().gap_3().p_4())))
}
