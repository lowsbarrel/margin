use margin_markdown::{Document, Span};
use margin_text_buffer::{
    Change, Selection, next_grapheme_boundary, next_word_boundary, prev_grapheme_boundary,
    prev_word_boundary,
};

use crate::list_tree;
use crate::mapping::{self, RangeEdit};
use crate::text::{self, Line, TAB_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Granularity {
    Grapheme,
    Word,
    LineBoundary,
}

pub(crate) fn normalize_line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[must_use]
pub fn insert_text(document: &Document, selections: &[Selection], text: &str) -> Change {
    let text = normalize_line_breaks(text);
    let length = document.text().len();
    mapping::by_range(selections, |range| {
        let span = Span::new(range.start().min(length), range.end().min(length));
        Some(RangeEdit::replace(
            span,
            text.as_str(),
            span.start + text.len(),
        ))
    })
    .unwrap_or_default()
}

fn indent_unit_width(document: &Document) -> usize {
    match list_tree::detect_indent_unit(document.text()) {
        "\t" => TAB_SIZE,
        unit => unit.len(),
    }
}

fn backward_target(document: &Document, pos: usize, granularity: Granularity) -> usize {
    let source = document.text();
    let line = Line::at(document, pos);
    match granularity {
        Granularity::Grapheme => {
            let before = text::slice(source, line.start, pos);
            let blank = before.chars().all(|c| c == ' ' || c == '\t');
            if pos > line.start && pos < line.start + 200 && blank {
                if before.ends_with('\t') {
                    return pos - 1;
                }
                let unit = indent_unit_width(document);
                let column = text::columns(before);
                let drop = match column % unit {
                    0 => unit,
                    rest => rest,
                };
                let spaces = before
                    .bytes()
                    .rev()
                    .take(drop)
                    .take_while(|byte| *byte == b' ')
                    .count();
                return pos - spaces;
            }
            prev_grapheme_boundary(source, pos)
        }
        Granularity::Word => prev_word_boundary(source, pos),
        Granularity::LineBoundary if pos > line.start => line.start,
        Granularity::LineBoundary => pos.saturating_sub(1),
    }
}

fn forward_target(document: &Document, pos: usize, granularity: Granularity) -> usize {
    let source = document.text();
    match granularity {
        Granularity::Grapheme => next_grapheme_boundary(source, pos),
        Granularity::Word => next_word_boundary(source, pos),
        Granularity::LineBoundary => {
            let end = Line::at(document, pos).end;
            if pos < end {
                end
            } else {
                (pos + 1).min(source.len())
            }
        }
    }
}

fn delete_by(selections: &[Selection], target: impl Fn(usize) -> usize) -> Change {
    mapping::by_range(selections, |range| {
        let span = if range.is_empty() {
            let to = target(range.head);
            Span::new(range.head.min(to), range.head.max(to))
        } else {
            range.span()
        };
        Some(RangeEdit::replace(span, "", span.start))
    })
    .unwrap_or_default()
}

#[must_use]
pub fn delete_backward(
    document: &Document,
    selections: &[Selection],
    granularity: Granularity,
) -> Change {
    delete_by(selections, |pos| {
        backward_target(document, pos, granularity)
    })
}

#[must_use]
pub fn delete_forward(
    document: &Document,
    selections: &[Selection],
    granularity: Granularity,
) -> Change {
    delete_by(selections, |pos| forward_target(document, pos, granularity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    fn back(source: &str, granularity: Granularity) -> Option<String> {
        run(source, |d, s| Some(delete_backward(d, s, granularity)))
    }

    fn forward(source: &str, granularity: Granularity) -> Option<String> {
        run(source, |d, s| Some(delete_forward(d, s, granularity)))
    }

    #[test]
    fn deletes_graphemes_and_selections() {
        assert_eq!(
            back("ae\u{301}|x", Granularity::Grapheme).as_deref(),
            Some("a|x")
        );
        assert_eq!(back("a\n|b", Granularity::Grapheme).as_deref(), Some("a|b"));
        assert_eq!(back("a«bc»d", Granularity::Word).as_deref(), Some("a|d"));
        assert_eq!(
            forward("a|bc", Granularity::Grapheme).as_deref(),
            Some("a|c")
        );
    }

    #[test]
    fn deletes_an_indent_unit_of_leading_space() {
        assert_eq!(
            back("- a\n  - b\n    |x", Granularity::Grapheme).as_deref(),
            Some("- a\n  - b\n  |x")
        );
        assert_eq!(back("\t|x", Granularity::Grapheme).as_deref(), Some("|x"));
    }

    #[test]
    fn deletes_words_and_line_boundaries() {
        assert_eq!(
            back("one two|", Granularity::Word).as_deref(),
            Some("one |")
        );
        assert_eq!(
            forward("|one two", Granularity::Word).as_deref(),
            Some("| two")
        );
        assert_eq!(
            back("one\ntwo th|ree", Granularity::LineBoundary).as_deref(),
            Some("one\n|ree")
        );
        assert_eq!(
            back("one\n|two", Granularity::LineBoundary).as_deref(),
            Some("one|two")
        );
        assert_eq!(
            forward("o|ne\ntwo", Granularity::LineBoundary).as_deref(),
            Some("o|\ntwo")
        );
    }

    #[test]
    fn inserts_text_over_every_selection() {
        let insert = |d: &Document, s: &[Selection]| Some(insert_text(d, s, "x\r\ny"));
        assert_eq!(run("a|b«cd»", insert).as_deref(), Some("ax\ny|bx\ny|"));
    }
}
