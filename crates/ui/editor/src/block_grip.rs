use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, StatefulInteractiveElement, Styled, div, px,
};
use margin_i18n::t;
use margin_theme::MarginTheme;

use crate::block_drag::{self, GRIP_HEIGHT, GRIP_WIDTH};
use crate::note_editor::NoteEditor;

const DOT_DIAMETER: f32 = 2.6;
const DOT_CENTERS: [(f32, f32); 6] = [
    (2.5, 2.5),
    (2.5, 7.0),
    (2.5, 11.5),
    (7.5, 2.5),
    (7.5, 7.0),
    (7.5, 11.5),
];
const GRIP_GROUP: &str = "block-grip";
const REST_OPACITY: f32 = 0.55;

pub(crate) fn overlay(editor: &NoteEditor, cx: &Context<'_, NoteEditor>) -> Vec<AnyElement> {
    let state = editor.block_drag();
    let origin = editor.session().list().viewport_bounds().origin;
    let mut out = Vec::new();
    if let Some(grip) = state.grip_origin() {
        out.push(grip_element(grip - origin, state.is_moving(), cx));
    }
    if let Some(at) = state.drop_point() {
        let (_, right) = block_drag::text_edges(editor);
        out.push(
            div()
                .absolute()
                .left(at.x - origin.x)
                .top(at.y - origin.y - px(1.0))
                .h(px(2.0))
                .w((right - at.x).max(GRIP_HEIGHT))
                .rounded(px(1.0))
                .bg(MarginTheme::palette(cx).bg_brand)
                .into_any_element(),
        );
    }
    out
}

fn grip_element(at: Point<Pixels>, active: bool, cx: &Context<'_, NoteEditor>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let (rest, lit, fill) = (
        palette.text_tertiary,
        palette.text_secondary,
        palette.surface(3),
    );
    let label = t(cx, "editor_block_handle");
    let dots = DOT_CENTERS.iter().map(|&(x, y)| {
        div()
            .absolute()
            .left(px(x - DOT_DIAMETER / 2.0))
            .top(px(y - DOT_DIAMETER / 2.0))
            .size(px(DOT_DIAMETER))
            .rounded_full()
            .bg(if active { lit } else { rest })
            .group_hover(GRIP_GROUP, |style| style.bg(lit))
    });
    let grip = div()
        .id(GRIP_GROUP)
        .group(GRIP_GROUP)
        .absolute()
        .left(at.x)
        .top(at.y)
        .w(GRIP_WIDTH)
        .h(GRIP_HEIGHT)
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor(CursorStyle::OpenHand)
        .on_mouse_down(MouseButton::Left, cx.listener(block_drag::press))
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
        .child(div().relative().w(px(10.0)).h(px(14.0)).children(dots));
    if active {
        grip.bg(fill).into_any_element()
    } else {
        grip.opacity(REST_OPACITY)
            .hover(move |style| style.opacity(1.0).bg(fill))
            .into_any_element()
    }
}
