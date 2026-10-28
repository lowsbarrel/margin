use gpui_kit::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, div, rems,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, TextScale};

use crate::canvas_view::{CanvasMenu, CanvasView};

fn item(
    id: &'static str,
    label: SharedString,
    color: Hsla,
    (hover_fill, hover_text): (Hsla, Hsla),
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .w_full()
        .px_2p5()
        .py_1p5()
        .rounded_sm()
        .text_ui_sm()
        .text_color(color)
        .hover(move |style| style.bg(hover_fill).text_color(hover_text))
        .child(label)
}

pub(crate) fn panel(menu: CanvasMenu, cx: &Context<'_, CanvasView>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let negative = palette.bg_negative;
    let body = if menu.confirming {
        div()
            .child(
                div()
                    .px_2p5()
                    .py_1p5()
                    .text_ui_sm()
                    .text_color(palette.text_secondary)
                    .child(t(cx, "canvas_clear_confirm")),
            )
            .child(
                div()
                    .flex()
                    .gap_1p5()
                    .px_0p5()
                    .pt_0p5()
                    .child(
                        item(
                            "canvas-clear-no",
                            t(cx, "canvas_clear_no"),
                            palette.text_secondary,
                            (palette.surface(3), palette.text_primary),
                        )
                        .text_center()
                        .border_1()
                        .border_color(palette.border_secondary)
                        .on_click(cx.listener(|view, _, _, cx| view.close_menu(cx))),
                    )
                    .child(
                        item(
                            "canvas-clear-yes",
                            t(cx, "canvas_clear_yes"),
                            negative,
                            (negative.opacity(0.1), negative),
                        )
                        .text_center()
                        .border_1()
                        .border_color(negative.opacity(0.4))
                        .on_click(cx.listener(|view, _, _, cx| view.clear_all(cx))),
                    ),
            )
    } else {
        div().child(
            item(
                "canvas-clear-all",
                t(cx, "canvas_clear_all"),
                negative,
                (negative.opacity(0.1), negative),
            )
            .on_click(cx.listener(|view, _, _, cx| view.confirm_clear(cx))),
        )
    };
    div()
        .id("canvas-menu")
        .absolute()
        .left(menu.position.x)
        .top(menu.position.y)
        .w(rems(13.0))
        .p_1p5()
        .rounded_lg()
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .shadow_lg()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .child(body)
}
