use unicode_segmentation::GraphemeCursor;

use crate::line;

#[must_use]
pub fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = line::floor_char_boundary(text, offset);
    GraphemeCursor::new(offset, text.len(), true)
        .next_boundary(text, 0)
        .expect("the whole text is passed as one chunk")
        .unwrap_or(text.len())
}

#[must_use]
pub fn prev_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = line::floor_char_boundary(text, offset);
    GraphemeCursor::new(offset, text.len(), true)
        .prev_boundary(text, 0)
        .expect("the whole text is passed as one chunk")
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_moves_one_byte() {
        assert_eq!(next_grapheme_boundary("abc", 1), 2);
        assert_eq!(prev_grapheme_boundary("abc", 1), 0);
    }

    #[test]
    fn stops_at_the_text_bounds() {
        assert_eq!(next_grapheme_boundary("abc", 3), 3);
        assert_eq!(prev_grapheme_boundary("abc", 0), 0);
        assert_eq!(next_grapheme_boundary("", 0), 0);
    }

    #[test]
    fn combining_marks_stay_with_their_base() {
        let text = "e\u{301}x";
        assert_eq!(next_grapheme_boundary(text, 0), 3);
        assert_eq!(prev_grapheme_boundary(text, 3), 0);
    }

    #[test]
    fn emoji_sequences_are_one_step() {
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        let text = format!("a{family}b");
        assert_eq!(next_grapheme_boundary(&text, 1), 1 + family.len());
        assert_eq!(prev_grapheme_boundary(&text, 1 + family.len()), 1);
        let flag = "\u{1f1ee}\u{1f1f9}";
        assert_eq!(next_grapheme_boundary(flag, 0), flag.len());
    }

    #[test]
    fn crlf_is_one_step_and_mid_character_offsets_snap() {
        assert_eq!(next_grapheme_boundary("a\r\nb", 1), 3);
        assert_eq!(next_grapheme_boundary("é", 1), 2);
    }
}
