use std::rc::Rc;

use gpui_kit::{App, Hsla, SharedString, TextRun, Window, font, px};
use margin_theme::{MarginTheme, UI_FONT_FAMILY};

use crate::board::Board;
use crate::canvas_view::CanvasView;
use crate::color;
use crate::gesture::Gesture;
use crate::paint::{CursorRing, Scene};
use crate::tool::Tool;

pub(crate) fn scene(view: &CanvasView, cx: &App) -> Scene {
    let board = &view.board;
    let palette = MarginTheme::palette(cx).clone();
    let (live_stroke, live_shape, snaps) = match &board.gesture {
        Gesture::Draw(stroke) => (Some(stroke.clone()), None, Vec::new()),
        Gesture::Shape {
            shape,
            snap_start,
            snap_end,
        } => (
            None,
            Some(shape.clone()),
            snap_start.iter().chain(snap_end).copied().collect(),
        ),
        _ => (None, None, Vec::new()),
    };
    let tool = board.tool;
    let cursor = view
        .cursor
        .filter(|_| tool != Tool::Text && !board.is_panning() && view.menu.is_none())
        .map(|local| CursorRing {
            local,
            diameter: board.sizes.current(tool) * board.camera.zoom,
            color: if tool == Tool::Eraser {
                palette.text_tertiary
            } else {
                color::resolve(&board.color, &palette)
            },
        });
    Scene {
        drawing: Rc::clone(&board.drawing),
        live_stroke,
        live_shape,
        snaps,
        camera: board.camera,
        cursor,
        palette,
    }
}

pub(crate) fn measure_labels(board: &mut Board, window: &Window) {
    let text_system = window.text_system();
    board.label_widths = board
        .drawing
        .labels
        .iter()
        .map(|label| {
            let text = SharedString::from(label.text.clone());
            #[expect(clippy::cast_possible_truncation, reason = "font sizes fit in f32")]
            let size = px(label.font_size as f32);
            let run = TextRun {
                len: text.len(),
                font: font(UI_FONT_FAMILY),
                color: Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f64::from(
                text_system
                    .shape_line(text, size, &[run], None)
                    .width
                    .as_f32(),
            )
        })
        .collect();
}
