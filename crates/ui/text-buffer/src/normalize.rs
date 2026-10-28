use std::cmp::Ordering;

use crate::line;
use crate::selection::Selection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Normalized {
    pub selections: Vec<Selection>,
    pub primary: usize,
}

fn snapped(text: &str, selection: Selection) -> Selection {
    Selection::new(
        line::floor_char_boundary(text, selection.anchor),
        line::floor_char_boundary(text, selection.head),
    )
}

fn joins(previous: Selection, next: Selection) -> bool {
    match next.start().cmp(&previous.end()) {
        Ordering::Less => true,
        Ordering::Equal => next.is_empty() || previous.is_empty(),
        Ordering::Greater => false,
    }
}

fn merged(previous: Selection, next: Selection) -> Selection {
    let from = previous.start();
    let to = previous.end().max(next.end());
    let oriented = if next.is_empty() { previous } else { next };
    if oriented.anchor > oriented.head {
        Selection::new(to, from)
    } else {
        Selection::new(from, to)
    }
}

#[must_use]
pub(crate) fn normalize(text: &str, selections: Vec<Selection>, primary: usize) -> Normalized {
    let primary = primary.min(selections.len().saturating_sub(1));
    let mut tagged: Vec<(Selection, bool)> = selections
        .into_iter()
        .enumerate()
        .map(|(index, selection)| (snapped(text, selection), index == primary))
        .collect();
    tagged.sort_by_key(|(selection, _)| (selection.start(), selection.end()));
    let mut kept: Vec<(Selection, bool)> = Vec::with_capacity(tagged.len());
    for (selection, is_primary) in tagged {
        match kept.last_mut() {
            Some((previous, previous_primary)) if joins(*previous, selection) => {
                *previous = merged(*previous, selection);
                *previous_primary |= is_primary;
            }
            _ => kept.push((selection, is_primary)),
        }
    }
    if kept.is_empty() {
        kept.push((Selection::cursor(0), true));
    }
    Normalized {
        primary: kept
            .iter()
            .position(|(_, is_primary)| *is_primary)
            .unwrap_or(0),
        selections: kept.into_iter().map(|(selection, _)| selection).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_becomes_a_cursor_at_the_start() {
        let normalized = normalize("abc", Vec::new(), 3);
        assert_eq!(normalized.selections, [Selection::cursor(0)]);
        assert_eq!(normalized.primary, 0);
    }

    #[test]
    fn selections_are_clamped_snapped_and_sorted() {
        let text = "aé bc";
        let normalized = normalize(text, vec![Selection::cursor(99), Selection::new(2, 0)], 0);
        assert_eq!(
            normalized.selections,
            [Selection::new(1, 0), Selection::cursor(6)]
        );
        assert_eq!(normalized.primary, 1);
    }

    #[test]
    fn overlapping_ranges_and_duplicate_cursors_merge() {
        let normalized = normalize(
            "abcdefghij",
            vec![
                Selection::cursor(1),
                Selection::new(2, 5),
                Selection::new(7, 3),
                Selection::cursor(1),
                Selection::cursor(9),
            ],
            3,
        );
        assert_eq!(
            normalized.selections,
            [
                Selection::cursor(1),
                Selection::new(7, 2),
                Selection::cursor(9)
            ]
        );
        assert_eq!(normalized.primary, 0);
    }

    #[test]
    fn touching_ranges_stay_apart_but_touching_cursors_merge() {
        let normalized = normalize(
            "abcdefghij",
            vec![
                Selection::new(0, 2),
                Selection::new(2, 4),
                Selection::cursor(4),
            ],
            2,
        );
        assert_eq!(
            normalized.selections,
            [Selection::new(0, 2), Selection::new(2, 4)]
        );
        assert_eq!(normalized.primary, 1);
    }
}
