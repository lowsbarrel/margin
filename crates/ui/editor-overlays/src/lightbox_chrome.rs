use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    ClickEvent, Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, div, rems,
};
use margin_i18n::{t, t_with};
use margin_theme::MarginTheme;
use margin_viewers::ZoomKey;

use crate::image_lightbox::{ImageLightbox, Step};

fn control(
    id: &'static str,
    icon: IconName,
    label: SharedString,
    cx: &Context<'_, ImageLightbox>,
) -> Stateful<Div> {
    let palette = MarginTheme::palette(cx);
    let tooltip = label.clone();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size_8()
        .rounded_lg()
        .cursor_pointer()
        .text_color(palette.lightbox_control)
        .hover(|style| style.bg(palette.lightbox_control_hover))
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(Icon::new(icon))
}

pub(crate) fn lightbox_button(
    id: &'static str,
    step: Step,
    cx: &Context<'_, ImageLightbox>,
) -> impl IntoElement {
    let (icon, key) = match step {
        Step::Previous => (IconName::ChevronLeft, "viewer_previous"),
        Step::Next => (IconName::ChevronRight, "viewer_next"),
    };
    let button = control(id, icon, t(cx, key), cx)
        .size_11()
        .rounded_full()
        .on_click(cx.listener(move |lightbox, _: &ClickEvent, _, cx| lightbox.step(step, cx)));
    let placed = div().absolute().top_0().bottom_0().flex().items_center();
    match step {
        Step::Previous => placed.left_4(),
        Step::Next => placed.right_4(),
    }
    .child(button)
}

pub(crate) fn lightbox_close(cx: &Context<'_, ImageLightbox>) -> impl IntoElement {
    div().absolute().top_4().right_4().child(
        control("lightbox-close", IconName::X, t(cx, "tab_close"), cx)
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| ImageLightbox::close(cx))),
    )
}

pub(crate) fn counter(
    current: usize,
    total: usize,
    cx: &Context<'_, ImageLightbox>,
) -> impl IntoElement {
    div()
        .absolute()
        .top_4()
        .left_6()
        .text_sm()
        .text_color(MarginTheme::palette(cx).lightbox_control)
        .child(t_with(
            cx,
            "viewer_position",
            &[
                ("current", &current.to_string()),
                ("total", &total.to_string()),
            ],
        ))
}

fn zoom_control(
    id: &'static str,
    icon: IconName,
    key: &str,
    zoom: ZoomKey,
    cx: &Context<'_, ImageLightbox>,
) -> Stateful<Div> {
    control(id, icon, t(cx, key), cx)
        .on_click(cx.listener(move |lightbox, _: &ClickEvent, _, cx| lightbox.apply_zoom(zoom, cx)))
}

pub(crate) fn zoom_bar(
    lightbox: &ImageLightbox,
    cx: &Context<'_, ImageLightbox>,
) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    #[expect(clippy::cast_possible_truncation, reason = "percent display")]
    let percent = ((lightbox.current_scale() * 100.0).round() as i32).to_string();
    let fit = lightbox.is_fit();
    h_flex()
        .absolute()
        .bottom_4()
        .left_0()
        .right_0()
        .justify_center()
        .child(
            h_flex()
                .gap_1()
                .px_1()
                .rounded(rems(0.75))
                .bg(palette.lightbox_backdrop)
                .text_sm()
                .text_color(palette.lightbox_control)
                .child(zoom_control(
                    "lightbox-zoom-out",
                    IconName::ZoomOut,
                    "viewer_zoom_out",
                    ZoomKey::Out,
                    cx,
                ))
                .child(zoom_control(
                    "lightbox-zoom-in",
                    IconName::ZoomIn,
                    "viewer_zoom_in",
                    ZoomKey::In,
                    cx,
                ))
                .child(div().min_w_12().text_center().child(t_with(
                    cx,
                    "viewer_zoom_percent",
                    &[("percent", &percent)],
                )))
                .child(
                    zoom_control(
                        "lightbox-fit",
                        IconName::Expand,
                        "viewer_fit",
                        ZoomKey::Fit,
                        cx,
                    )
                    .when(fit, |control| control.bg(palette.lightbox_control_hover)),
                )
                .child(zoom_control(
                    "lightbox-actual",
                    IconName::Maximize,
                    "viewer_actual_size",
                    ZoomKey::Actual,
                    cx,
                )),
        )
}
