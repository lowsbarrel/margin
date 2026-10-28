use std::ops::RangeInclusive;

use margin_markdown::{Document, Span};
use margin_text_buffer::Selection;

pub(crate) const TAB_SIZE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line<'text> {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub text: &'text str,
}

impl<'text> Line<'text> {
    pub(crate) fn at(document: &'text Document, offset: usize) -> Self {
        Self::numbered(document, document.line_of(offset))
    }

    pub(crate) fn numbered(document: &'text Document, index: usize) -> Self {
        let span = document.line_span(index);
        Self {
            index,
            start: span.start,
            end: span.end,
            text: document.line_text(index),
        }
    }

    pub(crate) fn span(&self) -> Span {
        Span::new(self.start, self.end)
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

pub(crate) fn selected_lines(document: &Document, selection: Selection) -> RangeInclusive<usize> {
    document.line_of(selection.start())..=document.line_of(selection.end())
}

pub(crate) fn leading_space(text: &str) -> &str {
    let rest = text.trim_start_matches([' ', '\t']);
    text.get(..text.len() - rest.len()).unwrap_or_default()
}

pub(crate) fn count_column(text: &str, to: usize) -> usize {
    let mut column = 0;
    for character in text.get(..to.min(text.len())).unwrap_or(text).chars() {
        column = if character == '\t' {
            column + TAB_SIZE - column % TAB_SIZE
        } else {
            column + 1
        };
    }
    column
}

pub(crate) fn columns(indent: &str) -> usize {
    count_column(indent, indent.len())
}

pub(crate) fn slice(text: &str, from: usize, to: usize) -> &str {
    let to = to.min(text.len());
    text.get(from.min(to)..to).unwrap_or_default()
}

pub(crate) fn char_before(text: &str, at: usize) -> Option<char> {
    text.get(..at).and_then(|head| head.chars().next_back())
}

pub(crate) fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..).and_then(|rest| rest.chars().next())
}

pub(crate) fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

pub(crate) fn word_at(document: &Document, at: usize) -> Option<Span> {
    let line = Line::at(document, at);
    let text = document.text();
    let mut from = at;
    while from > line.start {
        match char_before(text, from) {
            Some(character) if is_word_char(character) => from -= character.len_utf8(),
            _ => break,
        }
    }
    let mut to = at;
    while to < line.end {
        match char_at(text, to) {
            Some(character) if is_word_char(character) => to += character.len_utf8(),
            _ => break,
        }
    }
    (from < to).then(|| Span::new(from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_tab_stops() {
        assert_eq!(count_column("\t a", 3), 6);
        assert_eq!(count_column("  \tx", 3), 4);
    }

    #[test]
    fn finds_the_word_around_a_position() {
        let document = Document::parse("say héllo_x now");
        assert_eq!(word_at(&document, 7), Some(Span::new(4, 12)));
        assert_eq!(word_at(&document, 3), Some(Span::new(0, 3)));
        assert_eq!(Line::at(&document, 3).span(), Span::new(0, 16));
    }
}
