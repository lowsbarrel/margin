use std::sync::Arc;
use std::{fmt, mem};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle, Processor, Rgb};
use margin_terminals::TerminalSize;
use parking_lot::Mutex;

const SCROLLBACK_LINES: usize = 5000;

pub(crate) type ColorFormatter = Arc<dyn Fn(Rgb) -> String + Send + Sync>;
pub(crate) type SizeFormatter = Arc<dyn Fn(WindowSize) -> String + Send + Sync>;

pub(crate) enum TermRequest {
    Reply(String),
    Copy(String),
    Color {
        index: usize,
        format: ColorFormatter,
    },
    TextAreaSize(SizeFormatter),
}

#[derive(Clone, Default)]
pub(crate) struct Requests(Arc<Mutex<Vec<TermRequest>>>);

impl EventListener for Requests {
    fn send_event(&self, event: Event) {
        let request = match event {
            Event::PtyWrite(text) => TermRequest::Reply(text),
            Event::ClipboardStore(_, text) => TermRequest::Copy(text),
            Event::ColorRequest(index, format) => TermRequest::Color { index, format },
            Event::TextAreaSizeRequest(format) => TermRequest::TextAreaSize(format),
            _ => return,
        };
        self.0.lock().push(request);
    }
}

pub(crate) struct Emulator {
    term: Term<Requests>,
    parser: Processor,
    requests: Requests,
}

impl fmt::Debug for Emulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emulator")
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl Emulator {
    pub(crate) fn new(size: TerminalSize) -> Self {
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            default_cursor_style: CursorStyle {
                shape: CursorShape::Block,
                blinking: true,
            },
            ..Config::default()
        };
        let requests = Requests::default();
        Self {
            term: Term::new(config, &term_size(size), requests.clone()),
            parser: Processor::new(),
            requests,
        }
    }

    pub(crate) fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub(crate) fn take_requests(&self) -> Vec<TermRequest> {
        mem::take(&mut *self.requests.0.lock())
    }

    pub(crate) const fn term(&self) -> &Term<Requests> {
        &self.term
    }

    pub(crate) fn size(&self) -> TerminalSize {
        TerminalSize::new(
            u16::try_from(self.term.columns()).unwrap_or(u16::MAX),
            u16::try_from(self.term.screen_lines()).unwrap_or(u16::MAX),
        )
    }

    pub(crate) fn resize(&mut self, size: TerminalSize) {
        self.term.resize(term_size(size));
    }

    pub(crate) fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    pub(crate) fn set_focused(&mut self, focused: bool) {
        self.term.is_focused = focused;
    }

    pub(crate) fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    pub(crate) fn scroll_page(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    pub(crate) fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub(crate) fn history_size(&self) -> usize {
        self.term.history_size()
    }

    pub(crate) fn point(&self, column: usize, row: usize) -> Point {
        let line = Line(i32::try_from(row).unwrap_or(i32::MAX))
            - i32::try_from(self.display_offset()).unwrap_or(i32::MAX);
        let column = column.min(self.term.last_column().0);
        Point::new(line.min(self.term.bottommost_line()), Column(column))
    }

    pub(crate) fn start_selection(&mut self, kind: SelectionType, point: Point, side: Side) {
        self.term.selection = Some(Selection::new(kind, point, side));
    }

    pub(crate) fn update_selection(&mut self, point: Point, side: Side) {
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub(crate) fn selection_text(&self) -> Option<String> {
        let selection = self.term.selection.as_ref()?;
        if selection.is_empty() {
            return None;
        }
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }
}

fn term_size(size: TerminalSize) -> TermSize {
    TermSize::new(usize::from(size.cols()), usize::from(size.rows()))
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::term::cell::{Cell, Flags};
    use alacritty_terminal::vte::ansi::{Color, NamedColor};

    use super::*;

    fn emulator(bytes: &[u8]) -> Emulator {
        let mut emulator = Emulator::new(TerminalSize::new(10, 3));
        emulator.advance(bytes);
        emulator
    }

    fn cell_at(emulator: &Emulator, line: i32, column: usize) -> &Cell {
        &emulator.term().grid()[Point::new(Line(line), Column(column))]
    }

    #[test]
    fn colours_attributes_and_cursor_moves_land_in_the_grid() {
        let emulator = emulator(b"\x1b[1;31mA\x1b[0m\x1b[38;5;196mB\x1b[38;2;1;2;3mC\x1b[3;5HZ");
        let bold_red = cell_at(&emulator, 0, 0);
        assert_eq!(bold_red.c, 'A');
        assert_eq!(bold_red.fg, Color::Named(NamedColor::Red));
        assert!(bold_red.flags.contains(Flags::BOLD));
        assert_eq!(cell_at(&emulator, 0, 1).fg, Color::Indexed(196));
        assert_eq!(
            cell_at(&emulator, 0, 2).fg,
            Color::Spec(Rgb { r: 1, g: 2, b: 3 })
        );
        assert_eq!(cell_at(&emulator, 2, 4).c, 'Z');
        assert_eq!(
            emulator.term().grid().cursor.point,
            Point::new(Line(2), Column(5))
        );
    }

    #[test]
    fn wide_characters_take_two_cells() {
        let emulator = emulator("a界b".as_bytes());
        assert_eq!(cell_at(&emulator, 0, 1).c, '界');
        assert!(cell_at(&emulator, 0, 1).flags.contains(Flags::WIDE_CHAR));
        assert!(
            cell_at(&emulator, 0, 2)
                .flags
                .contains(Flags::WIDE_CHAR_SPACER)
        );
        assert_eq!(cell_at(&emulator, 0, 3).c, 'b');
    }

    #[test]
    fn replies_to_device_status_queries() {
        let emulator = emulator(b"\x1b[2;3H\x1b[6n");
        let replies: Vec<String> = emulator
            .take_requests()
            .into_iter()
            .filter_map(|request| match request {
                TermRequest::Reply(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(replies, vec!["\x1b[2;3R".to_owned()]);
    }

    #[test]
    fn selection_copies_the_selected_text_across_lines() {
        let mut emulator = emulator(b"hello\r\nworld");
        assert_eq!(emulator.selection_text(), None);
        emulator.start_selection(SelectionType::Simple, emulator.point(1, 0), Side::Left);
        emulator.update_selection(emulator.point(2, 1), Side::Right);
        assert_eq!(emulator.selection_text().as_deref(), Some("ello\nwor"));
        emulator.start_selection(SelectionType::Semantic, emulator.point(2, 1), Side::Left);
        assert_eq!(emulator.selection_text().as_deref(), Some("world"));
        emulator.clear_selection();
        assert_eq!(emulator.selection_text(), None);
    }

    #[test]
    fn scrollback_keeps_lines_that_left_the_screen() {
        let mut emulator = emulator(b"1\r\n2\r\n3\r\n4\r\n5");
        assert_eq!(emulator.history_size(), 2);
        emulator.scroll(1);
        assert_eq!(emulator.display_offset(), 1);
        assert_eq!(emulator.term().grid()[emulator.point(0, 0)].c, '2');
        emulator.scroll_page(Scroll::Bottom);
        assert_eq!(emulator.display_offset(), 0);
    }
}
