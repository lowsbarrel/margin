use std::ops::Range;

pub(crate) fn word_chunks(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut chunks = Vec::new();
    let mut start = range.start;
    let mut after_space = false;
    for (offset, ch) in text.get(range.clone()).unwrap_or_default().char_indices() {
        let at = range.start + offset;
        if ch.is_whitespace() {
            after_space = true;
        } else if after_space {
            chunks.push(start..at);
            start = at;
            after_space = false;
        }
    }
    if start < range.end || chunks.is_empty() {
        chunks.push(start..range.end);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str, range: Range<usize>) -> Vec<&str> {
        word_chunks(text, range)
            .into_iter()
            .map(|chunk| text.get(chunk).expect("chunks stay on char boundaries"))
            .collect()
    }

    #[test]
    fn punctuation_after_an_atom_stays_with_it_and_spaces_trail_each_word() {
        assert_eq!(
            words("x. Hover è a", 1..13),
            vec![". ", "Hover ", "è ", "a"]
        );
    }

    #[test]
    fn an_empty_range_still_yields_one_chunk_for_the_caret() {
        assert_eq!(word_chunks("abc", 3..3), vec![3..3]);
    }
}
