use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::v_flex;
use gpui_kit::{
    Context, CursorStyle, DispatchPhase, ElementInputHandler, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Styled, Window,
    canvas, div, list,
};
use margin_editor_actions::EDITOR_CONTEXT;
use margin_theme::MarginTheme;

use crate::note_editor::NoteEditor;
use crate::{block_drag, block_grip, drop_target, keys, pointer, rows, surfaces};

pub(crate) fn render(
    editor: &NoteEditor,
    window: &Window,
    cx: &Context<'_, NoteEditor>,
) -> impl IntoElement {
    let hits = editor.hits();
    let focus = editor.focus_handle_ref().clone();
    let entity = cx.entity();
    let list_state = editor.session().list().clone();
    editor.render_state().tables().borrow_mut().remember_scroll(
        list_state.logical_scroll_top(),
        list_state.viewport_bounds().size.height,
    );
    let rows = list(list_state.clone(), cx.processor(rows::render_row)).size_full();
    let input_focus = focus.clone();
    let grabbing = editor.block_drag().is_moving();
    let input = canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            hits.clear();
            if grabbing {
                window.set_window_cursor_style(CursorStyle::ClosedHand);
            }
            let moves = entity.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    moves.update(cx, |editor, cx| {
                        pointer::mouse_move(editor, event, window, cx);
                        block_drag::mouse_move(editor, event, window, cx);
                    });
                }
            });
            let releases = entity.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    releases.update(cx, |editor, cx| {
                        pointer::finish_drag(editor, window, cx);
                        block_drag::release(editor, window, cx);
                    });
                }
            });
            window.handle_input(&input_focus, ElementInputHandler::new(bounds, entity), cx);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let body = div()
        .id("note-body")
        .relative()
        .flex_1()
        .min_h_0()
        .track_focus(&focus)
        .key_context(EDITOR_CONTEXT)
        .cursor_text()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|editor, event, window, cx| {
                pointer::mouse_down(editor, event, window, cx);
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|editor, event, window, cx| {
                pointer::mouse_up(editor, event, window, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                pointer::context_click(editor, event, window, cx);
            }),
        );
    let body = drop_target::wire(body, cx)
        .child(input)
        .child(rows)
        .children(block_grip::overlay(editor, cx))
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .child(Scrollbar::vertical(&list_state)),
        );
    let (body, anchors) = surfaces::render_overlays(editor, body, window, cx);
    let (painted, editor_id) = (editor.hits(), cx.entity_id());
    let body = body.child(canvas(
        |_, _, _| {},
        move |_, (), _, cx| {
            let moved = anchors
                .iter()
                .any(|(offset, used)| painted.caret_bounds(*offset) != *used);
            if moved {
                cx.notify(editor_id);
            }
        },
    ));
    v_flex()
        .size_full()
        .bg(MarginTheme::palette(cx).surface(0))
        .child(editor.crumbs().clone())
        .child(keys::register(body, cx))
}
