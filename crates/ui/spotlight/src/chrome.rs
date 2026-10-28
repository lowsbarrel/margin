use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex};
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, IntoElement, ParentElement, Rems, Styled, div,
    rems,
};
use margin_i18n::{t, t_count, t_with};
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};

use crate::spotlight::Spotlight;

const ROOT_PX: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LeadingIcon {
    Busy,
    Ask,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReplaceToggle {
    Hidden,
    Off,
    On,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReplaceAll {
    Ready,
    Unavailable,
    Running,
}

pub(crate) fn scaled(size_px: f32) -> Rems {
    rems(size_px / ROOT_PX)
}

pub(crate) fn icon(name: IconName, size_px: f32) -> Icon {
    Icon::new(name).size(scaled(size_px))
}

pub(crate) fn quiet_button(active: bool, cx: &App) -> ButtonCustomVariant {
    let palette = MarginTheme::palette(cx);
    let variant = ButtonCustomVariant::new(cx)
        .foreground(palette.text_tertiary)
        .hover(palette.surface(3))
        .active(palette.surface(3));
    if active {
        variant
            .color(palette.brand(12.0))
            .foreground(palette.text_brand)
    } else {
        variant
    }
}

pub(crate) fn search_bar(
    input: &Entity<InputState>,
    leading: LeadingIcon,
    toggle: ReplaceToggle,
    cx: &Context<'_, Spotlight>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let leading = match leading {
        LeadingIcon::Busy => Spinner::new()
            .color(palette.text_tertiary)
            .into_any_element(),
        LeadingIcon::Ask => icon(IconName::Sparkles, 16.0)
            .text_color(palette.text_tertiary)
            .into_any_element(),
        LeadingIcon::Search => icon(IconName::Search, 16.0)
            .text_color(palette.text_tertiary)
            .into_any_element(),
    };
    let mut bar = h_flex()
        .gap_2p5()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(palette.border_secondary)
        .child(leading)
        .child(
            Input::new(input)
                .id("spotlight-query")
                .appearance(false)
                .aria_label(t(cx, "spotlight_title"))
                .text_ui_base()
                .flex_1(),
        );
    if toggle != ReplaceToggle::Hidden {
        let label = t(cx, "spotlight_toggle_replace");
        bar = bar.child(
            Button::new("spotlight-toggle-replace")
                .custom(quiet_button(toggle == ReplaceToggle::On, cx))
                .xsmall()
                .icon(icon(IconName::Replace, 14.0))
                .toggled(toggle == ReplaceToggle::On)
                .accessibility_label(label.clone())
                .tooltip(label)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_replace(cx))),
        );
    }
    bar.child(
        div()
            .flex_shrink_0()
            .px_1p5()
            .py_0p5()
            .rounded(scaled(4.0))
            .border_1()
            .border_color(palette.border_secondary)
            .font_family(MONO_FONT_FAMILY)
            .text_ui_xs()
            .text_color(palette.text_tertiary)
            .child(t(cx, "spotlight_escape_key")),
    )
    .into_any_element()
}

pub(crate) fn replace_bar(
    input: &Entity<InputState>,
    count: usize,
    state: ReplaceAll,
    cx: &Context<'_, Spotlight>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let count = count.to_string();
    h_flex()
        .gap_2p5()
        .px_4()
        .py_2p5()
        .border_b_1()
        .border_color(palette.border_secondary)
        .child(icon(IconName::Replace, 14.0).text_color(palette.text_tertiary))
        .child(
            Input::new(input)
                .id("spotlight-replacement")
                .appearance(false)
                .aria_label(t(cx, "spotlight_replace_placeholder"))
                .text_ui_sm()
                .flex_1(),
        )
        .child(
            Button::new("spotlight-replace-all")
                .outline()
                .xsmall()
                .icon(icon(IconName::ReplaceAll, 13.0))
                .label(t_with(
                    cx,
                    "spotlight_replace_all_short",
                    &[("count", &count)],
                ))
                .tooltip(t(cx, "spotlight_replace_all_files"))
                .disabled(state != ReplaceAll::Ready)
                .loading(state == ReplaceAll::Running)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let paths = this
                        .results()
                        .hits()
                        .iter()
                        .map(|hit| hit.path.clone())
                        .collect();
                    this.replace(paths, window, cx);
                })),
        )
        .into_any_element()
}

pub(crate) fn tag_crumb(tag: &str, count: usize, cx: &Context<'_, Spotlight>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    h_flex()
        .justify_between()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(palette.border_secondary)
        .child(
            Button::new("spotlight-tag-back")
                .text()
                .xsmall()
                .h(scaled(32.0))
                .px_4()
                .icon(icon(IconName::ChevronLeft, 13.0).text_color(palette.text_secondary))
                .child(
                    div()
                        .font_family(MONO_FONT_FAMILY)
                        .text_ui_xs()
                        .text_color(palette.text_brand)
                        .child(format!("#{tag}")),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.back_to_tags(window, cx);
                })),
        )
        .child(
            div()
                .text_ui_xs()
                .text_color(palette.text_tertiary)
                .child(t_count(
                    cx,
                    "spotlight_tag_files",
                    u64::try_from(count).unwrap_or(u64::MAX),
                    &[],
                )),
        )
        .into_any_element()
}

pub(crate) fn empty_state(message_key: &str, cx: &Context<'_, Spotlight>) -> AnyElement {
    div()
        .px_4()
        .py_8()
        .text_center()
        .text_ui_sm()
        .text_color(MarginTheme::palette(cx).text_tertiary)
        .child(t(cx, message_key))
        .into_any_element()
}
