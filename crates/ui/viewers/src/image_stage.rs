use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::scroll::ScrollbarHandle;
use gpui_kit::{
    Bounds, Hsla, IntoElement, Pixels, Point, Size, Styled, canvas, fill, point, px, size,
};

use crate::zoom::{offset_from_scroll, scroll_from_offset};

const CHECKER: Pixels = px(8.);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Stage {
    pub viewport: Bounds<Pixels>,
    pub content: (f32, f32),
    pub offset: (f32, f32),
}

impl Stage {
    pub(crate) fn viewport_size(&self) -> (f32, f32) {
        (
            f32::from(self.viewport.size.width),
            f32::from(self.viewport.size.height),
        )
    }

    pub(crate) fn scroll(&self) -> (f32, f32) {
        let (width, height) = self.viewport_size();
        (
            scroll_from_offset(self.offset.0, self.content.0, width),
            scroll_from_offset(self.offset.1, self.content.1, height),
        )
    }

    pub(crate) fn set_scroll(&mut self, scroll: (f32, f32)) {
        let (width, height) = self.viewport_size();
        self.offset = (
            offset_from_scroll(scroll.0, self.content.0, width),
            offset_from_scroll(scroll.1, self.content.1, height),
        );
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StageHandle(Rc<Cell<Stage>>);

impl StageHandle {
    pub(crate) fn get(&self) -> Stage {
        self.0.get()
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut Stage)) {
        let mut stage = self.0.get();
        change(&mut stage);
        self.0.set(stage);
    }
}

impl ScrollbarHandle for StageHandle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.get().viewport
    }

    fn offset(&self) -> Point<Pixels> {
        let (x, y) = self.get().scroll();
        point(px(-x), px(-y))
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.update(|stage| stage.set_scroll((-f32::from(offset.x), -f32::from(offset.y))));
    }

    fn content_size(&self) -> Size<Pixels> {
        let stage = self.get();
        let (width, height) = stage.viewport_size();
        size(
            px(stage.content.0.max(width)),
            px(stage.content.1.max(height)),
        )
    }
}

pub(crate) fn checkerboard(base: Hsla, square: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            window.paint_quad(fill(bounds, base));
            let visible = bounds.intersect(&window.content_mask().bounds);
            if visible.size.width <= Pixels::ZERO || visible.size.height <= Pixels::ZERO {
                return;
            }
            let first_row = ((visible.top() - bounds.top()) / CHECKER).floor();
            let first_column = ((visible.left() - bounds.left()) / CHECKER).floor();
            let mut row = first_row;
            while bounds.top() + CHECKER * row < visible.bottom() {
                let mut column = first_column;
                while bounds.left() + CHECKER * column < visible.right() {
                    if ((row + column) % 2.0).abs() < 0.5 {
                        let origin = point(
                            bounds.left() + CHECKER * column,
                            bounds.top() + CHECKER * row,
                        );
                        let cell = Bounds::new(origin, size(CHECKER, CHECKER));
                        window.paint_quad(fill(cell.intersect(&bounds), square));
                    }
                    column += 1.0;
                }
                row += 1.0;
            }
        },
    )
    .absolute()
    .size_full()
}
