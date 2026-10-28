use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::scroll::ScrollbarHandle;
use gpui_kit::{Bounds, Pixels, Point, Size, point, px, size};

use crate::grid_layout::GridLayout;
use crate::session::Screen;

#[derive(Clone, Debug)]
pub(crate) struct ScrollHandle {
    screen: Arc<Screen>,
    layout: Rc<Cell<GridLayout>>,
}

impl ScrollHandle {
    pub(crate) const fn new(screen: Arc<Screen>, layout: Rc<Cell<GridLayout>>) -> Self {
        Self { screen, layout }
    }

    fn lines_above_viewport(&self) -> usize {
        let emulator = self.screen.lock();
        emulator.history_size() - emulator.display_offset()
    }
}

impl ScrollbarHandle for ScrollHandle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.layout.get().bounds()
    }

    fn offset(&self) -> Point<Pixels> {
        let line_height = self.layout.get().line_height();
        point(px(0.), -line_height * lines(self.lines_above_viewport()))
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let line_height = self.layout.get().line_height();
        let mut emulator = self.screen.lock();
        let history = emulator.history_size();
        let above = whole_lines(-offset.y / line_height).min(history);
        let target = history - above;
        let current = emulator.display_offset();
        let delta =
            i32::try_from(target).unwrap_or(i32::MAX) - i32::try_from(current).unwrap_or(i32::MAX);
        emulator.scroll(delta);
    }

    fn content_size(&self) -> Size<Pixels> {
        let layout = self.layout.get();
        let emulator = self.screen.lock();
        let rows = emulator.history_size() + usize::from(emulator.size().rows());
        size(
            layout.bounds().size.width,
            layout.line_height() * lines(rows),
        )
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "scrollback is capped far below the f32 mantissa"
)]
fn lines(count: usize) -> f32 {
    count as f32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the line count is rounded and clamped to the u16 range"
)]
fn whole_lines(ratio: f32) -> usize {
    usize::from(ratio.round().clamp(0.0, f32::from(u16::MAX)) as u16)
}
