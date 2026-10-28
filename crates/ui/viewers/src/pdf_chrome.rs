use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{Disableable, Sizable, h_flex};
use gpui_kit::{
    AnyElement, Context, CursorStyle, DispatchPhase, Entity, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ListOffset, MouseButton, MouseMoveEvent, ParentElement, ScrollEvent,
    StatefulInteractiveElement, Styled, Window, canvas, div, list, point, px,
};
use margin_i18n::{t, t_with};
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::{TabContent, file_title};

use crate::controls::{
    LINE_STEP, PAGE_FRACTION, ScrollKey, ZoomKey, icon_button, scroll_key, zoom_key,
};
use crate::external::open_external;
use crate::image_toolbar::zoom_percent;
use crate::pdf_layout::PAGE_PADDING;
use crate::pdf_pointer;
use crate::pdf_view::PdfView;
use crate::zoom::{ZOOM_STEP, wheel_factor};

pub(crate) fn toolbar(view: &PdfView, cx: &Context<'_, PdfView>) -> impl IntoElement + use<> {
    let palette = MarginTheme::palette(cx);
    let tertiary = palette.text_tertiary;
    let open_path = view.path().clone();
    h_flex()
        .gap_1()
        .p_1p5()
        .border_t_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface[1])
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px_2()
                .truncate()
                .text_ui_xs()
                .text_color(palette.text_primary)
                .child(file_title(view.path()).to_owned()),
        )
        .child(
            icon_button(
                "zoom-out",
                IconName::ZoomOut,
                t(cx, "viewer_zoom_out"),
                tertiary,
            )
            .on_click(cx.listener(|view, _, _, cx| {
                view.zoom_to(view.current_scale() / ZOOM_STEP, cx);
            })),
        )
        .child(
            div()
                .min_w_12()
                .text_center()
                .text_ui_sm()
                .text_color(palette.text_primary)
                .child(zoom_percent(view.current_scale(), cx)),
        )
        .child(
            icon_button(
                "zoom-in",
                IconName::ZoomIn,
                t(cx, "viewer_zoom_in"),
                tertiary,
            )
            .on_click(cx.listener(|view, _, _, cx| {
                view.zoom_to(view.current_scale() * ZOOM_STEP, cx);
            })),
        )
        .child(
            icon_button(
                "fit-width",
                IconName::Expand,
                t(cx, "pdf_fit_width"),
                tertiary,
            )
            .toggled(view.is_fit_width())
            .on_click(cx.listener(|view, _, _, cx| view.fit(cx))),
        )
        .child(
            icon_button(
                "open-external",
                IconName::ExternalLink,
                t(cx, "viewer_open_external"),
                tertiary,
            )
            .on_click(move |_, window, cx| open_external(&open_path, window, cx)),
        )
        .child(
            div()
                .mx_1()
                .h_4()
                .w(px(1.))
                .flex_shrink_0()
                .bg(palette.border_secondary),
        )
        .children(page_controls(view, cx))
}

fn page_controls(view: &PdfView, cx: &Context<'_, PdfView>) -> [AnyElement; 4] {
    let tertiary = MarginTheme::palette(cx).text_tertiary;
    let page = view.current_page();
    let total = view.page_count();
    [
        icon_button(
            "previous-page",
            IconName::ChevronLeft,
            t(cx, "viewer_previous"),
            tertiary,
        )
        .disabled(page == 0)
        .on_click(cx.listener(move |view, _, window, cx| {
            view.go_to_page(page.saturating_sub(1), window, cx);
        }))
        .into_any_element(),
        Input::new(view.page_input())
            .small()
            .w_11()
            .h(css_px(22.75))
            .text_ui_xs()
            .text_center()
            .aria_label(t(cx, "pdf_page_number"))
            .into_any_element(),
        div()
            .px_1()
            .text_ui_xs()
            .text_color(tertiary)
            .child(t_with(
                cx,
                "pdf_page_total",
                &[("total", &total.to_string())],
            ))
            .into_any_element(),
        icon_button(
            "next-page",
            IconName::ChevronRight,
            t(cx, "viewer_next"),
            tertiary,
        )
        .disabled(page + 1 >= total)
        .on_click(cx.listener(move |view, _, window, cx| {
            view.go_to_page(page + 1, window, cx);
        }))
        .into_any_element(),
    ]
}

pub(crate) fn pages(view: &PdfView, cx: &Context<'_, PdfView>) -> impl IntoElement + use<> {
    let entity = cx.entity();
    let dragger = view.selection.is_dragging().then(|| cx.entity());
    let list_state = view.list_state();
    let h_scroll = view.h_scroll();
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("pdf-pages")
                .size_full()
                .overflow_x_scroll()
                .track_scroll(&h_scroll)
                .cursor(
                    if view.selection.is_dragging() || view.selection.is_hovering_text() {
                        CursorStyle::IBeam
                    } else {
                        CursorStyle::Arrow
                    },
                )
                .on_scroll(cx.listener(on_wheel))
                .on_mouse_down(MouseButton::Left, cx.listener(pdf_pointer::on_mouse_down))
                .on_mouse_move(cx.listener(pdf_pointer::on_mouse_move))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| pdf_pointer::on_mouse_up(view, cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| pdf_pointer::on_mouse_up(view, cx)),
                )
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            let width = f32::from(bounds.size.width);
                            entity.update(cx, |view, cx| view.set_container_width(width, cx));
                        },
                        move |_, (), window, _| {
                            if let Some(dragger) = dragger {
                                window.on_mouse_event(
                                    move |event: &MouseMoveEvent, phase, _, cx| {
                                        if phase == DispatchPhase::Bubble {
                                            dragger.update(cx, |view, cx| {
                                                pdf_pointer::drag_to(view, event, cx);
                                            });
                                        }
                                    },
                                );
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .child(
                    list(
                        list_state.clone(),
                        cx.processor(|view, index, window, cx| view.render_page(index, window, cx)),
                    )
                    .w(px(view.content_width()))
                    .h_full()
                    .pb(px(PAGE_PADDING)),
                ),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .child(Scrollbar::vertical(&list_state)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .child(Scrollbar::horizontal(&h_scroll)),
        )
        .children(view.find().is_open().then(|| view.find().render(cx)))
}

pub(crate) fn on_page_input(
    view: &mut PdfView,
    input: &Entity<InputState>,
    event: &InputEvent,
    window: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    match event {
        InputEvent::Focus => view.set_page_editing(true),
        InputEvent::PressEnter { .. } | InputEvent::Blur => {
            view.set_page_editing(false);
            match input.read(cx).value().trim().parse::<usize>() {
                Ok(page) => view.go_to_page(page.saturating_sub(1), window, cx),
                Err(_) => view.sync_page_input(window, cx),
            }
        }
        InputEvent::Change => {}
    }
}

fn scroll_pages(view: &PdfView, key: ScrollKey) {
    let list_state = view.list_state();
    let height = f32::from(list_state.viewport_bounds().size.height);
    match key {
        ScrollKey::Line(x, y) => {
            list_state.scroll_by(px(y * LINE_STEP));
            let h_scroll = view.h_scroll();
            let offset = h_scroll.offset();
            h_scroll.set_offset(point(offset.x - px(x * LINE_STEP), offset.y));
        }
        ScrollKey::Page(direction) => list_state.scroll_by(px(direction * height * PAGE_FRACTION)),
        ScrollKey::Start => list_state.scroll_to(ListOffset::default()),
        ScrollKey::End => list_state.scroll_to_end(),
    }
}

pub(crate) fn on_key(
    view: &mut PdfView,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    let key = event.keystroke.key.as_str();
    if key == "escape" && view.find().is_open() {
        view.close_find(window, cx);
        cx.stop_propagation();
        return;
    }
    if view.find().input().focus_handle(cx).is_focused(window)
        || view.page_input().focus_handle(cx).is_focused(window)
    {
        return;
    }
    if key == "c" && event.keystroke.modifiers.secondary(cx) {
        view.copy_selection(cx);
        cx.stop_propagation();
        return;
    }
    if key == "a" && event.keystroke.modifiers.secondary(cx) {
        let pages = view.page_count();
        view.selection.select_all(pages);
        cx.notify();
        cx.stop_propagation();
        return;
    }
    if key == "enter" && view.find().is_open() {
        let step = if event.keystroke.modifiers.shift {
            -1
        } else {
            1
        };
        view.step_match(step, window, cx);
        cx.stop_propagation();
        return;
    }
    let scale = view.current_scale();
    match zoom_key(event, cx) {
        Some(ZoomKey::In) => view.zoom_to(scale * ZOOM_STEP, cx),
        Some(ZoomKey::Out) => view.zoom_to(scale / ZOOM_STEP, cx),
        Some(ZoomKey::Actual) => view.zoom_to(1.0, cx),
        Some(ZoomKey::Fit) => view.fit(cx),
        None => match scroll_key(event) {
            Some(key) => {
                scroll_pages(view, key);
                cx.notify();
            }
            None => return,
        },
    }
    cx.stop_propagation();
}

fn on_wheel(
    view: &mut PdfView,
    event: &ScrollEvent,
    _: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    if event.modifiers.control || event.modifiers.platform {
        let delta = event.delta.pixel_delta(px(16.));
        view.zoom_to(view.current_scale() * wheel_factor(-f32::from(delta.y)), cx);
        cx.stop_propagation();
    }
}

pub(crate) fn on_find_input(
    view: &mut PdfView,
    input: &Entity<InputState>,
    event: &InputEvent,
    window: &mut Window,
    cx: &mut Context<'_, PdfView>,
) {
    match event {
        InputEvent::Change => {
            let query = input.read(cx).value().to_string();
            view.search(query, window, cx);
        }
        InputEvent::PressEnter { shift, .. } => {
            view.step_match(if *shift { -1 } else { 1 }, window, cx);
        }
        InputEvent::Focus | InputEvent::Blur => {}
    }
}
