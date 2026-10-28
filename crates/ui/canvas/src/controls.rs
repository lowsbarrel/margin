use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants};
use gpui_kit::{App, ElementId, Hsla, InteractiveElement, Styled, div};
use margin_i18n::t;
use margin_theme::MarginTheme;

use crate::model::ShapeKind;

pub(crate) const fn shape_icon(kind: ShapeKind) -> IconName {
    match kind {
        ShapeKind::Rect => IconName::Square,
        ShapeKind::Ellipse => IconName::Circle,
        ShapeKind::Line => IconName::Minus,
        ShapeKind::Arrow => IconName::ArrowUpRight,
    }
}

pub(crate) const fn shape_label(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Rect => "canvas_rect",
        ShapeKind::Ellipse => "canvas_ellipse",
        ShapeKind::Line => "canvas_line",
        ShapeKind::Arrow => "canvas_arrow",
    }
}

pub(crate) fn tool_foreground(active: bool, cx: &App) -> Hsla {
    let palette = MarginTheme::palette(cx);
    if active {
        palette.text_primary
    } else {
        palette.text_tertiary
    }
}

pub(crate) fn icon_button(id: impl Into<ElementId>, active: bool, cx: &App) -> Button {
    let palette = MarginTheme::palette(cx);
    let variant = ButtonCustomVariant::new(cx)
        .foreground(tool_foreground(active, cx))
        .hover(palette.surface(3))
        .active(palette.surface(3));
    let variant = if active {
        variant.color(palette.surface(2)).hover(palette.surface(2))
    } else {
        variant
    };
    Button::new(id)
        .custom(variant)
        .rounded(ButtonRounded::Small)
        .size_8()
        .p_0()
}

pub(crate) fn tool_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &str,
    active: bool,
    cx: &App,
) -> Button {
    icon_button(id, active, cx)
        .icon(Icon::new(icon).size_4())
        .tooltip(t(cx, label))
}

pub(crate) fn swatch(color: Hsla, cx: &App) -> gpui_kit::Div {
    div()
        .size_full()
        .rounded_full()
        .border_1()
        .border_color(MarginTheme::palette(cx).border_strong)
        .bg(color)
}

pub(crate) fn popover_panel(cx: &App) -> gpui_kit::Div {
    let palette = MarginTheme::palette(cx);
    div()
        .flex()
        .flex_col()
        .p_1p5()
        .rounded_lg()
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .shadow_lg()
        .occlude()
}
