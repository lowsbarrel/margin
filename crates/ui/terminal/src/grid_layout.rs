use alacritty_terminal::event::WindowSize;
use alacritty_terminal::index::{Column, Point, Side};
use gpui_kit::{Bounds, Pixels, Size, point, px, size};
use margin_terminals::TerminalSize;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct GridLayout {
    bounds: Bounds<Pixels>,
    cell: Size<Pixels>,
}

impl GridLayout {
    pub(crate) fn new(bounds: Bounds<Pixels>, cell_width: Pixels, line_height: Pixels) -> Self {
        Self {
            bounds,
            cell: size(cell_width.max(px(1.)), line_height.max(px(1.))),
        }
    }

    pub(crate) fn terminal_size(&self) -> TerminalSize {
        TerminalSize::new(
            cells(self.bounds.size.width / self.cell.width),
            cells(self.bounds.size.height / self.cell.height),
        )
    }

    pub(crate) const fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    pub(crate) const fn line_height(&self) -> Pixels {
        self.cell.height
    }

    pub(crate) const fn cell_width(&self) -> Pixels {
        self.cell.width
    }

    pub(crate) fn window_size(&self) -> WindowSize {
        let size = self.terminal_size();
        WindowSize {
            num_lines: size.rows(),
            num_cols: size.cols(),
            cell_width: cells(f32::from(self.cell.width)),
            cell_height: cells(f32::from(self.cell.height)),
        }
    }

    pub(crate) fn wheel_lines(&self, scrolled: &mut Pixels, delta: Pixels) -> i32 {
        *scrolled += delta;
        let lines = (*scrolled / self.cell.height).trunc();
        *scrolled -= self.cell.height * lines;
        signed_lines(lines)
    }

    pub(crate) fn cell_bounds(&self, column: usize, row: usize, width: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(
                self.bounds.origin.x + self.cell.width * count(column),
                self.bounds.origin.y + self.cell.height * count(row),
            ),
            size(self.cell.width * count(width), self.cell.height),
        )
    }

    pub(crate) fn cell_at(&self, position: gpui_kit::Point<Pixels>) -> (Point<usize>, Side) {
        let grid = self.terminal_size();
        let x = (position.x - self.bounds.origin.x).max(px(0.));
        let y = (position.y - self.bounds.origin.y).max(px(0.));
        let horizontal = x / self.cell.width;
        let column = index(horizontal).min(usize::from(grid.cols()) - 1);
        let row = index(y / self.cell.height).min(usize::from(grid.rows()) - 1);
        let side = if horizontal.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        (Point::new(row, Column(column)), side)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a non-negative cell count clamped to the u16 range"
)]
fn cells(ratio: f32) -> u16 {
    ratio.floor().clamp(0.0, f32::from(u16::MAX)) as u16
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is a non-negative cell index clamped to the u16 range"
)]
fn index(ratio: f32) -> usize {
    usize::from(ratio.floor().clamp(0.0, f32::from(u16::MAX)) as u16)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "grid coordinates stay far below the f32 mantissa"
)]
fn count(cells: usize) -> f32 {
    cells as f32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the line count is truncated and clamped to the i16 range"
)]
fn signed_lines(lines: f32) -> i32 {
    i32::from(lines.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> GridLayout {
        GridLayout::new(
            Bounds::new(point(px(10.), px(20.)), size(px(85.), px(50.))),
            px(8.),
            px(16.),
        )
    }

    #[test]
    fn whole_cells_fit_the_bounds() {
        assert_eq!(layout().terminal_size(), TerminalSize::new(10, 3));
    }

    #[test]
    fn pointer_positions_map_to_cells_and_sides() {
        let (cell, side) = layout().cell_at(point(px(31.), px(37.)));
        assert_eq!(cell, Point::new(1, Column(2)));
        assert_eq!(side, Side::Right);
        let (clamped, _) = layout().cell_at(point(px(500.), px(500.)));
        assert_eq!(clamped, Point::new(2, Column(9)));
    }
}
