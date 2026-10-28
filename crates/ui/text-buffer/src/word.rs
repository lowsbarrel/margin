use margin_markdown::Span;
use unicode_segmentation::UnicodeSegmentation;

use crate::{grapheme, line};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Space,
    Word,
    Other,
    LineBreak,
}

fn category(cluster: &str) -> Category {
    if cluster.contains('\n') {
        Category::LineBreak
    } else if cluster.chars().all(char::is_whitespace) {
        Category::Space
    } else if cluster
        .chars()
        .any(|character| character.is_alphanumeric() || character == '_')
    {
        Category::Word
    } else {
        Category::Other
    }
}

fn group_length<'text>(clusters: impl Iterator<Item = &'text str>) -> usize {
    let mut length = 0;
    let mut group = None;
    for cluster in clusters {
        let next = category(cluster);
        match (group, next) {
            (None, Category::LineBreak) => return cluster.len(),
            (_, Category::LineBreak) => return length,
            (None | Some(Category::Space), _) => group = Some(next),
            (Some(current), _) if current != next => return length,
            _ => {}
        }
        length += cluster.len();
    }
    length
}

#[must_use]
pub fn next_word_boundary(text: &str, offset: usize) -> usize {
    let offset = line::floor_char_boundary(text, offset);
    offset + group_length(text[offset..].graphemes(true))
}

#[must_use]
pub fn prev_word_boundary(text: &str, offset: usize) -> usize {
    let offset = line::floor_char_boundary(text, offset);
    offset - group_length(text[..offset].graphemes(true).rev())
}

#[must_use]
pub fn word_span_at(text: &str, offset: usize) -> Span {
    let start = line::line_start(text, offset);
    let line = &text[start..line::line_end(text, offset)];
    let at = (line::floor_char_boundary(text, offset) - start).min(line.len());
    let (from, to) = if at == line.len() {
        (grapheme::prev_grapheme_boundary(line, at), at)
    } else {
        (at, grapheme::next_grapheme_boundary(line, at))
    };
    if from == to {
        return Span::empty(start + at);
    }
    let kind = category(&line[from..to]);
    let before: usize = line[..from]
        .graphemes(true)
        .rev()
        .take_while(|previous| category(previous) == kind)
        .map(str::len)
        .sum();
    let after: usize = line[to..]
        .graphemes(true)
        .take_while(|next| category(next) == kind)
        .map(str::len)
        .sum();
    Span::new(start + from - before, start + to + after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_skips_spaces_then_one_word() {
        let text = "hello   world";
        assert_eq!(next_word_boundary(text, 0), 5);
        assert_eq!(next_word_boundary(text, 5), 13);
        assert_eq!(next_word_boundary(text, 13), 13);
    }

    #[test]
    fn backward_skips_spaces_then_one_word() {
        let text = "hello   world";
        assert_eq!(prev_word_boundary(text, 13), 8);
        assert_eq!(prev_word_boundary(text, 8), 0);
        assert_eq!(prev_word_boundary(text, 0), 0);
    }

    #[test]
    fn punctuation_is_its_own_group() {
        let text = "foo.,bar_baz";
        assert_eq!(next_word_boundary(text, 0), 3);
        assert_eq!(next_word_boundary(text, 3), 5);
        assert_eq!(next_word_boundary(text, 5), 12);
        assert_eq!(prev_word_boundary(text, 5), 3);
    }

    #[test]
    fn line_breaks_stop_movement() {
        let text = "foo  \n  bar";
        assert_eq!(next_word_boundary(text, 3), 5);
        assert_eq!(next_word_boundary(text, 5), 6);
        assert_eq!(next_word_boundary(text, 6), 11);
        assert_eq!(prev_word_boundary(text, 8), 6);
        assert_eq!(prev_word_boundary(text, 6), 5);
        assert_eq!(next_word_boundary("a\r\nb", 1), 3);
    }

    #[test]
    fn words_include_unicode_letters_and_combining_marks() {
        let text = "cafe\u{301} ok";
        assert_eq!(next_word_boundary(text, 0), 6);
        assert_eq!(prev_word_boundary(text, 6), 0);
        assert_eq!(next_word_boundary("città!", 0), 6);
    }

    #[test]
    fn word_span_selects_the_group_under_the_offset() {
        let text = "say hello, world";
        assert_eq!(word_span_at(text, 6), Span::new(4, 9));
        assert_eq!(word_span_at(text, 4), Span::new(4, 9));
        assert_eq!(word_span_at(text, 9), Span::new(9, 10));
        assert_eq!(word_span_at(text, 3), Span::new(3, 4));
    }

    #[test]
    fn word_span_at_line_end_looks_backward() {
        let text = "one two\nnext";
        assert_eq!(word_span_at(text, 7), Span::new(4, 7));
        assert_eq!(word_span_at(text, 8), Span::new(8, 12));
        assert_eq!(word_span_at("a\n\nb", 2), Span::empty(2));
    }
}
