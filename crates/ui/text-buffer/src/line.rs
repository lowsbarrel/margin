use margin_markdown::Span;

#[must_use]
pub fn floor_char_boundary(text: &str, offset: usize) -> usize {
    text.floor_char_boundary(offset)
}

#[must_use]
pub fn line_start(text: &str, offset: usize) -> usize {
    let offset = floor_char_boundary(text, offset);
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

#[must_use]
pub fn line_end(text: &str, offset: usize) -> usize {
    let offset = floor_char_boundary(text, offset);
    let end = text[offset..]
        .find('\n')
        .map_or(text.len(), |at| offset + at);
    if end > offset && text[..end].ends_with('\r') {
        end - 1
    } else {
        end
    }
}

#[must_use]
pub fn line_span(text: &str, offset: usize) -> Span {
    Span::new(line_start(text, offset), line_end(text, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_bounds_exclude_the_line_break() {
        let text = "one\ntwo\r\nthree";
        assert_eq!(line_start(text, 0), 0);
        assert_eq!(line_end(text, 1), 3);
        assert_eq!(line_start(text, 3), 0);
        assert_eq!(line_start(text, 4), 4);
        assert_eq!(line_end(text, 4), 7);
        assert_eq!(line_span(text, 10), Span::new(9, 14));
        assert_eq!(line_span(text, 99), Span::new(9, 14));
    }

    #[test]
    fn empty_lines_have_empty_spans() {
        let text = "a\n\nb";
        assert_eq!(line_span(text, 2), Span::empty(2));
        assert_eq!(line_span("", 0), Span::empty(0));
    }

    #[test]
    fn floor_snaps_inside_a_character_and_clamps() {
        let text = "aé";
        assert_eq!(floor_char_boundary(text, 2), 1);
        assert_eq!(floor_char_boundary(text, 3), 3);
        assert_eq!(floor_char_boundary(text, 10), 3);
    }
}
