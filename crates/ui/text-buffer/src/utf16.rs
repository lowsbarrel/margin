use crate::line;

#[must_use]
pub fn utf16_to_utf8(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (at, character) in text.char_indices() {
        units += character.len_utf16();
        if units > utf16 {
            return at;
        }
    }
    text.len()
}

#[must_use]
pub fn utf8_to_utf16(text: &str, utf8: usize) -> usize {
    text[..line::floor_char_boundary(text, utf8)]
        .chars()
        .map(char::len_utf16)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_offsets_are_identical() {
        assert_eq!(utf16_to_utf8("hello", 3), 3);
        assert_eq!(utf8_to_utf16("hello", 3), 3);
    }

    #[test]
    fn multibyte_and_astral_characters_convert() {
        let text = "aé😀b";
        assert_eq!(utf8_to_utf16(text, 1), 1);
        assert_eq!(utf8_to_utf16(text, 3), 2);
        assert_eq!(utf8_to_utf16(text, 7), 4);
        assert_eq!(utf8_to_utf16(text, 8), 5);
        assert_eq!(utf16_to_utf8(text, 2), 3);
        assert_eq!(utf16_to_utf8(text, 4), 7);
        assert_eq!(utf16_to_utf8(text, 5), 8);
    }

    #[test]
    fn offsets_inside_a_character_round_down_and_clamp() {
        let text = "😀x";
        assert_eq!(utf16_to_utf8(text, 1), 0);
        assert_eq!(utf8_to_utf16(text, 2), 0);
        assert_eq!(utf16_to_utf8(text, 99), 5);
        assert_eq!(utf8_to_utf16(text, 99), 3);
    }
}
