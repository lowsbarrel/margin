use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Sizable, Size, v_flex};
use gpui_kit::{
    App, Context, CursorStyle, Div, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, Stateful,
    StatefulInteractiveElement, Styled, Window, anchored, canvas, deferred, div, point, px,
};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_workbench::{TabContent, WorkbenchEvent, notify_error};

use crate::board::Board;
use crate::canvas_view::{CanvasMenu, CanvasView};
use crate::gesture::PressButton;
use crate::tool::Tool;
use crate::{canvas_scene, color, menu, paint, toolbar};

const INPUT_WIDTH_EM: f32 = 9.2;
const INPUT_LINE_HEIGHT: f32 = 1.2;
const INPUT_TEXT_SCALE: f32 = 0.875;

fn surface(view: &CanvasView, cx: &Context<'_, CanvasView>) -> impl IntoElement {
    let scene = canvas_scene::scene(view, cx);
    let bounds = Rc::clone(&view.bounds);
    let cursor = match view.board.tool {
        _ if view.board.is_panning() => CursorStyle::ClosedHand,
        _ if view.board.space_held => CursorStyle::OpenHand,
        Tool::Hand => CursorStyle::OpenHand,
        Tool::Text => CursorStyle::IBeam,
        _ => CursorStyle::Crosshair,
    };
    div()
        .id("canvas-surface")
        .size_full()
        .cursor(cursor)
        .child(
            canvas(
                move |area, _, _| bounds.set(area),
                move |area, (), window, cx| paint::paint(&scene, area, window, cx),
            )
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, event, window, cx| {
                view.mouse_down(event, PressButton::Primary, window, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(|view, event, window, cx| {
                view.mouse_down(event, PressButton::Middle, window, cx);
            }),
        )
        .on_mouse_move(cx.listener(|view, event, _, cx| view.mouse_move(event, cx)))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|view, _, _, cx| view.mouse_up(cx)),
        )
        .on_mouse_up(
            MouseButton::Middle,
            cx.listener(|view, _, _, cx| view.mouse_up(cx)),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|view, _, _, cx| view.mouse_up(cx)),
        )
        .on_mouse_up_out(
            MouseButton::Middle,
            cx.listener(|view, _, _, cx| view.mouse_up(cx)),
        )
        .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
            if !hovered {
                view.cursor = None;
                cx.notify();
            }
        }))
        .on_scroll(cx.listener(|view, event, window, cx| view.scroll(event, window, cx)))
        .on_pinch(cx.listener(|view, event, _, cx| view.pinch(event, cx)))
}

fn text_overlay(view: &CanvasView, palette: &Palette, cx: &Context<'_, CanvasView>) -> Option<Div> {
    let edit = view.text_edit.as_ref()?;
    let (left, top) = view.board.camera.to_local(edit.at);
    #[expect(clippy::cast_possible_truncation, reason = "screen sizes fit in f32")]
    let (left, top, font_size) = (
        left as f32,
        top as f32,
        (view.board.sizes.current(Tool::Text) * view.board.camera.zoom) as f32,
    );
    Some(
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(font_size * INPUT_WIDTH_EM))
            .min_w(px(100.0))
            .px_1()
            .py_0p5()
            .border_1()
            .border_dashed()
            .border_color(palette.text_primary)
            .rounded_sm()
            .text_size(px(font_size))
            .text_color(color::resolve(&view.board.color, palette))
            .capture_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    view.cancel_text(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                Input::new(&edit.input)
                    .appearance(false)
                    .with_size(Size::Size(px(font_size / INPUT_TEXT_SCALE)))
                    .p_0()
                    .h(px(font_size * INPUT_LINE_HEIGHT)),
            ),
    )
}

fn zoom_button(view: &CanvasView, cx: &Context<'_, CanvasView>) -> Button {
    let palette = MarginTheme::palette(cx);
    let percent = format!("{:.0}", view.board.camera.zoom * 100.0);
    Button::new("canvas-zoom")
        .custom(
            ButtonCustomVariant::new(cx)
                .foreground(palette.text_tertiary)
                .hover(palette.surface(3))
                .active(palette.surface(3)),
        )
        .rounded(ButtonRounded::Size(px(9999.0)))
        .h_6()
        .px_2p5()
        .text_ui_xs()
        .label(t_with(
            cx,
            "canvas_zoom_percent",
            &[("percent", percent.as_str())],
        ))
        .tooltip(t(cx, "canvas_reset_view"))
        .on_click(cx.listener(|view, _, _, cx| view.command(Board::reset_view, cx)))
}

fn context_menu(
    menu: CanvasMenu,
    window: &Window,
    cx: &Context<'_, CanvasView>,
) -> impl IntoElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.0), px(0.0))).child(
            div()
                .id("canvas-menu-backdrop")
                .w(viewport.width)
                .h(viewport.height)
                .occlude()
                .on_any_mouse_down(cx.listener(|view, _, _, cx| view.close_menu(cx)))
                .child(menu::panel(menu, cx)),
        ),
    )
    .with_priority(1)
}

fn root(view: &CanvasView, palette: &Palette, cx: &Context<'_, CanvasView>) -> Stateful<Div> {
    div()
        .id("canvas")
        .key_context("Canvas")
        .track_focus(&view.focus_handle)
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(palette.surface(0))
        .on_key_down(cx.listener(CanvasView::key_down))
        .on_key_up(cx.listener(|view, event, _, cx| view.key_up(event, cx)))
        .on_mouse_down(MouseButton::Right, cx.listener(CanvasView::open_menu))
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if let Some(error) = self.save_error.take() {
            window.defer(cx, move |window, cx| {
                notify_error(window, cx, "toast_canvas_save_failed", &error);
            });
        }
        canvas_scene::measure_labels(&mut self.board, window);
        let palette = MarginTheme::palette(cx).clone();
        let board = root(self, &palette, cx)
            .flex_1()
            .child(surface(self, cx))
            .children(text_overlay(self, &palette, cx))
            .child(
                div()
                    .absolute()
                    .bottom_4()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(toolbar::toolbar(self, cx)),
            )
            .child(
                div()
                    .absolute()
                    .right_4()
                    .bottom_4()
                    .child(zoom_button(self, cx)),
            )
            .children(self.menu.map(|menu| context_menu(menu, window, cx)));
        v_flex().size_full().child(self.crumbs.clone()).child(board)
    }
}

impl Focusable for CanvasView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<WorkbenchEvent> for CanvasView {}

impl TabContent for CanvasView {
    fn path(&self) -> &RelativePath {
        &self.path
    }

    fn rename(&mut self, to: RelativePath, cx: &mut Context<'_, Self>) {
        self.crumbs
            .update(cx, |crumbs, cx| crumbs.set_path(to.clone(), cx));
        self.path = to;
        cx.notify();
    }

    fn flush(&mut self, cx: &mut Context<'_, Self>) {
        self.save(cx);
    }

    fn reload(&mut self, cx: &mut Context<'_, Self>) {
        if !self.is_dirty() {
            self.load(cx);
        }
    }
}
