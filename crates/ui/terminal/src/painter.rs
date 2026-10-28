use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::RenderableCursor;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use gpui_kit::{
    App, BorderStyle, Bounds, Font, FontStyle, FontWeight, Hsla, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, StrikethroughStyle, TextAlign, TextRun, UnderlineStyle, Window, fill,
    outline, point, px, size,
};

use crate::colors::{Focus, TerminalColors};
use crate::emulator::Requests;
use crate::grid_layout::GridLayout;
use crate::session::Screen;

const BAR_WIDTH: Pixels = px(2.);
const HAIRLINE: Pixels = px(1.);

pub(crate) struct Frame {
    line_height: Pixels,
    background: Hsla,
    quads: Vec<PaintQuad>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Vec<PaintQuad>,
    cursor_text: Option<(Point<Pixels>, ShapedLine)>,
}

#[derive(Clone, PartialEq)]
struct CellStyle {
    color: Hsla,
    flags: Flags,
    underline: Hsla,
}

struct Batch {
    column: usize,
    row: usize,
    width: usize,
    monospace: bool,
    text: String,
    style: CellStyle,
}

pub(crate) struct Painter<'frame> {
    layout: GridLayout,
    colors: &'frame TerminalColors,
    base: Font,
    font_size: Pixels,
    focus: Focus,
    cursor_visible: bool,
}

impl Frame {
    pub(crate) fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let line_height = self.line_height;
        window.paint_quad(fill(bounds, self.background));
        for quad in self.quads.drain(..) {
            window.paint_quad(quad);
        }
        for (origin, line) in &self.lines {
            drop(line.paint(*origin, line_height, TextAlign::Left, None, window, cx));
        }
        for quad in self.cursor.drain(..) {
            window.paint_quad(quad);
        }
        if let Some((origin, line)) = &self.cursor_text {
            drop(line.paint(*origin, line_height, TextAlign::Left, None, window, cx));
        }
    }
}

impl<'frame> Painter<'frame> {
    pub(crate) const fn new(
        layout: GridLayout,
        colors: &'frame TerminalColors,
        base: Font,
        font_size: Pixels,
        focus: Focus,
        cursor_visible: bool,
    ) -> Self {
        Self {
            layout,
            colors,
            base,
            font_size,
            focus,
            cursor_visible,
        }
    }

    pub(crate) fn frame(&self, screen: &Screen, window: &Window) -> Frame {
        let emulator = screen.lock();
        let term = emulator.term();
        let content = term.renderable_content();
        let offset = content.display_offset;
        let overrides = content.colors;
        let selection = content.selection;
        let background = self
            .colors
            .resolve(Color::Named(NamedColor::Background), overrides);
        let mut quads = Vec::new();
        let mut lines = Vec::new();
        let mut batch: Option<Batch> = None;
        for indexed in content.display_iter {
            let Some(row) = viewport_row(indexed.point.line.0, offset) else {
                continue;
            };
            let column = indexed.point.column.0;
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                2
            } else {
                1
            };
            let bounds = self.layout.cell_bounds(column, row, width);
            let (foreground, cell_background) = self.cell_colors(cell, overrides);
            if cell_background != background {
                quads.push(fill(bounds, cell_background));
            }
            if selection.is_some_and(|range| range.contains(indexed.point)) {
                quads.push(fill(bounds, self.colors.selection(self.focus)));
            }
            let style = CellStyle {
                color: foreground,
                flags: cell.flags,
                underline: cell
                    .underline_color()
                    .map_or(foreground, |color| self.colors.resolve(color, overrides)),
            };
            let decorated = cell
                .flags
                .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT);
            let blank = (cell.c == ' ' || cell.c == '\t') && !decorated;
            let monospace = width == 1 && cell.c.is_ascii() && cell.zerowidth().is_none();
            let joins = batch.as_ref().is_some_and(|open| {
                open.monospace
                    && monospace
                    && open.row == row
                    && open.column + open.width == column
                    && open.style == style
            });
            if (!joins || blank)
                && let Some(done) = batch.take()
            {
                lines.push(self.shape(done, window));
            }
            if blank {
                continue;
            }
            let open = batch.get_or_insert_with(|| Batch {
                column,
                row,
                width: 0,
                monospace,
                text: String::new(),
                style,
            });
            open.width += width;
            open.text.push(cell.c);
            open.text.extend(cell.zerowidth().into_iter().flatten());
        }
        if let Some(done) = batch.take() {
            lines.push(self.shape(done, window));
        }
        let (cursor, cursor_text) = self.cursor(term, content.cursor, offset, window);
        Frame {
            line_height: self.layout.line_height(),
            background,
            quads,
            lines,
            cursor,
            cursor_text,
        }
    }

    fn cell_colors(&self, cell: &Cell, overrides: &Colors) -> (Hsla, Hsla) {
        let mut foreground = self.colors.resolve(cell.fg, overrides);
        let mut background = self.colors.resolve(cell.bg, overrides);
        if cell.flags.contains(Flags::DIM) {
            foreground = TerminalColors::dim(foreground);
        }
        if cell.flags.contains(Flags::INVERSE) {
            (foreground, background) = (background, foreground);
        }
        if cell.flags.contains(Flags::HIDDEN) {
            foreground = background;
        }
        (foreground, background)
    }

    fn shape(&self, batch: Batch, window: &Window) -> (Point<Pixels>, ShapedLine) {
        let flags = batch.style.flags;
        let font = Font {
            weight: if flags.contains(Flags::BOLD) {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            style: if flags.contains(Flags::ITALIC) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..self.base.clone()
        };
        let run = TextRun {
            len: batch.text.len(),
            font,
            color: batch.style.color,
            background_color: None,
            underline: flags
                .intersects(Flags::ALL_UNDERLINES)
                .then_some(UnderlineStyle {
                    thickness: HAIRLINE,
                    color: Some(batch.style.underline),
                    wavy: flags.contains(Flags::UNDERCURL),
                }),
            strikethrough: flags
                .contains(Flags::STRIKEOUT)
                .then_some(StrikethroughStyle {
                    thickness: HAIRLINE,
                    color: Some(batch.style.color),
                }),
        };
        let force_width = batch.monospace.then(|| self.layout.cell_width());
        let line = window.text_system().shape_line(
            SharedString::from(batch.text),
            self.font_size,
            &[run],
            force_width,
        );
        let origin = self.layout.cell_bounds(batch.column, batch.row, 1).origin;
        (origin, line)
    }

    fn cursor(
        &self,
        term: &Term<Requests>,
        cursor: RenderableCursor,
        offset: usize,
        window: &Window,
    ) -> (Vec<PaintQuad>, Option<(Point<Pixels>, ShapedLine)>) {
        let hidden_by_blink =
            term.cursor_style().blinking && !self.cursor_visible && self.focus == Focus::Focused;
        let Some(row) = viewport_row(cursor.point.line.0, offset)
            .filter(|row| *row < term.screen_lines())
            .filter(|_| cursor.shape != CursorShape::Hidden && !hidden_by_blink)
        else {
            return (Vec::new(), None);
        };
        let cell = &term.grid()[cursor.point];
        let width = if cell.flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };
        let bounds = self.layout.cell_bounds(cursor.point.column.0, row, width);
        let color = self
            .colors
            .resolve(Color::Named(NamedColor::Cursor), term.colors());
        let shape = match self.focus {
            Focus::Focused => cursor.shape,
            Focus::Unfocused => CursorShape::HollowBlock,
        };
        match shape {
            CursorShape::Block => {
                let accent = self
                    .colors
                    .resolve(Color::Named(NamedColor::Background), term.colors());
                let text = (cell.c != ' ').then(|| {
                    self.shape(
                        Batch {
                            column: cursor.point.column.0,
                            row,
                            width,
                            monospace: width == 1 && cell.c.is_ascii(),
                            text: cell.c.to_string(),
                            style: CellStyle {
                                color: accent,
                                flags: cell.flags & (Flags::BOLD | Flags::ITALIC),
                                underline: accent,
                            },
                        },
                        window,
                    )
                });
                (vec![fill(bounds, color)], text)
            }
            CursorShape::Beam => (
                vec![fill(
                    Bounds::new(bounds.origin, size(BAR_WIDTH, bounds.size.height)),
                    color,
                )],
                None,
            ),
            CursorShape::Underline => (
                vec![fill(
                    Bounds::new(
                        point(bounds.origin.x, bounds.bottom() - BAR_WIDTH),
                        size(bounds.size.width, BAR_WIDTH),
                    ),
                    color,
                )],
                None,
            ),
            CursorShape::HollowBlock | CursorShape::Hidden => {
                (vec![outline(bounds, color, BorderStyle::Solid)], None)
            }
        }
    }
}

fn viewport_row(line: i32, display_offset: usize) -> Option<usize> {
    let offset = i32::try_from(display_offset).ok()?;
    usize::try_from(line.checked_add(offset)?).ok()
}
