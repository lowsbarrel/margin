use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct FuzzyMatch {
    pub score: f32,
    pub ranges: Vec<Range<usize>>,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the length penalty only orders matches, so precision past 2^24 characters is irrelevant"
)]
fn length_penalty(characters: usize) -> f32 {
    characters as f32 * 0.01
}

fn text_char_len(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|rest| rest.chars().next())
        .map_or(0, char::len_utf8)
}

#[must_use]
pub fn fuzzy_match(needle: &str, haystack: &str) -> Option<FuzzyMatch> {
    let hay: Vec<(usize, char)> = haystack
        .char_indices()
        .map(|(at, character)| (at, character.to_lowercase().next().unwrap_or(character)))
        .collect();
    let mut ranges = Vec::new();
    let mut cursor: Option<usize> = None;
    let mut score = 0.0_f32;
    for wanted in needle.chars() {
        let from = cursor.map_or(0, |at| at + 1);
        let found = from
            + hay
                .get(from..)?
                .iter()
                .position(|(_, character)| *character == wanted)?;
        if Some(found) == cursor.map(|at| at + 1) || cursor.is_none() && found == 0 {
            score += 4.0;
        }
        let previous = found
            .checked_sub(1)
            .and_then(|at| hay.get(at))
            .map(|(_, character)| *character);
        if previous.is_none_or(|character| character == ' ' || character == '-') {
            score += 2.0;
        }
        score += 1.0;
        let (start, _) = hay.get(found).copied()?;
        let length = text_char_len(haystack, start);
        ranges.push(start..start + length);
        cursor = Some(found);
    }
    Some(FuzzyMatch {
        score: score - length_penalty(hay.len()),
        ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_consecutive_and_word_start_matches_higher() {
        let start = fuzzy_match("he", "Heading 1").expect("matches");
        let scattered = fuzzy_match("he", "the code").expect("matches");
        assert!(start.score > scattered.score);
        assert_eq!(start.ranges, vec![0..1, 1..2]);
        assert!(fuzzy_match("zz", "Heading").is_none());
    }

    #[test]
    fn reports_byte_ranges_in_the_original_text() {
        let found = fuzzy_match("é", "Élan é").expect("matches");
        assert_eq!(found.ranges, vec![0..2]);
    }
}
