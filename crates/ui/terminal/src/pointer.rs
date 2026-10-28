use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::TermMode;
use gpui_kit::{
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollEvent, px,
};

use crate::grid_layout::GridLayout;
use crate::keys;
use crate::mouse::{self, MouseAction};
use crate::session::Screen;

#[derive(Debug)]
pub(crate) enum PointerOutcome {
    Ignored,
    Redraw,
    Send(Vec<u8>),
}

#[derive(Debug)]
pub(crate) struct Pointer {
    selecting: bool,
    scrolled: Pixels,
}

impl Pointer {
    pub(crate) const fn new() -> Self {
        Self {
            selecting: false,
            scrolled: px(0.),
        }
    }

    pub(crate) fn down(
        &mut self,
        screen: &Screen,
        layout: GridLayout,
        event: &MouseDownEvent,
        right_selects_word: bool,
    ) -> PointerOutcome {
        let (cell, side) = layout.cell_at(event.position);
        let mut emulator = screen.lock();
        let mode = emulator.mode();
        if mouse::reports_mouse(mode, event.modifiers) {
            return report(mouse::mouse_report(
                MouseAction::Press(event.button),
                cell,
                event.modifiers,
                mode,
            ));
        }
        let kind = match (event.button, event.click_count) {
            (MouseButton::Right, _) if right_selects_word => SelectionType::Semantic,
            (MouseButton::Left, 2) => SelectionType::Semantic,
            (MouseButton::Left, 3..) => SelectionType::Lines,
            (MouseButton::Left, _) => SelectionType::Simple,
            _ => return PointerOutcome::Ignored,
        };
        let point = emulator.point(cell.column.0, cell.line);
        emulator.start_selection(kind, point, side);
        self.selecting = true;
        PointerOutcome::Redraw
    }

    pub(crate) fn moved(
        &mut self,
        screen: &Screen,
        layout: GridLayout,
        event: &MouseMoveEvent,
    ) -> PointerOutcome {
        let (cell, side) = layout.cell_at(event.position);
        let mut emulator = screen.lock();
        if self.selecting && event.pressed_button == Some(MouseButton::Left) {
            let point = emulator.point(cell.column.0, cell.line);
            emulator.update_selection(point, side);
            return PointerOutcome::Redraw;
        }
        self.selecting = false;
        let mode = emulator.mode();
        if !mouse::reports_mouse(mode, event.modifiers) {
            return PointerOutcome::Ignored;
        }
        let action = event
            .pressed_button
            .map_or(MouseAction::Move, MouseAction::Drag);
        report(mouse::mouse_report(action, cell, event.modifiers, mode))
    }

    pub(crate) fn up(
        &mut self,
        screen: &Screen,
        layout: GridLayout,
        event: &MouseUpEvent,
    ) -> PointerOutcome {
        self.selecting = false;
        let (cell, _) = layout.cell_at(event.position);
        let mode = screen.lock().mode();
        if !mouse::reports_mouse(mode, event.modifiers) {
            return PointerOutcome::Ignored;
        }
        let action = MouseAction::Release(event.button);
        report(mouse::mouse_report(action, cell, event.modifiers, mode))
    }

    pub(crate) fn wheel(
        &mut self,
        screen: &Screen,
        layout: GridLayout,
        event: &ScrollEvent,
    ) -> PointerOutcome {
        let delta = event.delta.pixel_delta(layout.line_height()).y;
        let lines = layout.wheel_lines(&mut self.scrolled, delta);
        if lines == 0 {
            return PointerOutcome::Ignored;
        }
        let (cell, _) = layout.cell_at(event.position);
        let mut emulator = screen.lock();
        let mode = emulator.mode();
        if mouse::reports_mouse(mode, event.modifiers) {
            let action = if lines > 0 {
                MouseAction::WheelUp
            } else {
                MouseAction::WheelDown
            };
            let mut bytes = Vec::new();
            for _ in 0..lines.unsigned_abs() {
                bytes.extend(
                    mouse::mouse_report(action, cell, event.modifiers, mode).unwrap_or_default(),
                );
            }
            return report(Some(bytes).filter(|bytes| !bytes.is_empty()));
        }
        if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            return PointerOutcome::Send(keys::alternate_scroll(lines, mode));
        }
        emulator.scroll(lines);
        PointerOutcome::Redraw
    }
}

fn report(bytes: Option<Vec<u8>>) -> PointerOutcome {
    bytes.map_or(PointerOutcome::Ignored, PointerOutcome::Send)
}
