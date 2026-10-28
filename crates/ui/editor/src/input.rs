use std::ops::Range;

use gpui_kit::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use margin_markdown::{Span, TextEdit};
use margin_text_buffer::{Change, EditOrigin, Selection, utf8_to_utf16, utf16_to_utf8};

use crate::editing;
use crate::note_editor::NoteEditor;

fn to_utf8(text: &str, range: &Range<usize>) -> Range<usize> {
    utf16_to_utf8(text, range.start)..utf16_to_utf8(text, range.end)
}

fn to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    utf8_to_utf16(text, range.start)..utf8_to_utf16(text, range.end)
}

impl EntityInputHandler for NoteEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<String> {
        let text = self.session().text();
        let bytes = to_utf8(text, &range);
        *adjusted_range = Some(to_utf16(text, &bytes));
        text.get(bytes).map(str::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.session().primary();
        let text = self.session().text();
        Some(UTF16Selection {
            range: to_utf16(text, &(selection.start()..selection.end())),
            reversed: selection.head < selection.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Range<usize>> {
        let marked = self.marked()?;
        Some(to_utf16(self.session().text(), &marked))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) {
        self.set_marked(None);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let target = self
            .marked()
            .or_else(|| range.map(|range| to_utf8(self.session().text(), &range)));
        self.set_marked(None);
        match target {
            Some(target) => {
                let end = target.start + text.len();
                let change = Change::new(vec![TextEdit::new(
                    Span::new(target.start, target.end),
                    text,
                )])
                .with_selections(vec![Selection::cursor(end)]);
                self.apply(change, EditOrigin::Typing, window, cx);
            }
            None => editing::type_text(self, text, window, cx),
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let session_text = self.session().text();
        let target = self
            .marked()
            .or_else(|| range.map(|range| to_utf8(session_text, &range)))
            .unwrap_or_else(|| {
                let primary = self.session().primary();
                primary.start()..primary.end()
            });
        let selected = new_selected_range.map_or(new_text.len()..new_text.len(), |range| {
            to_utf8(new_text, &range)
        });
        let start = target.start;
        let change = Change::new(vec![TextEdit::new(
            Span::new(target.start, target.end),
            new_text,
        )])
        .with_selections(vec![Selection::new(
            start + selected.start,
            start + selected.end,
        )]);
        self.apply(change, EditOrigin::Typing, window, cx);
        self.set_marked((!new_text.is_empty()).then(|| start..start + new_text.len()));
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        let start = utf16_to_utf8(self.session().text(), range_utf16.start);
        self.hits().caret_bounds(start)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let offset = self.hits().offset_at(point)?;
        Some(utf8_to_utf16(self.session().text(), offset))
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let text = self.session().text();
        Some(utf8_to_utf16(text, text.len()))
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> bool {
        self.is_loaded()
    }
}
