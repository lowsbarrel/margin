use std::collections::BTreeMap;
use std::mem;

use margin_markdown::{Fold, TextEdit};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CalloutFolds {
    collapsed: BTreeMap<usize, bool>,
}

impl CalloutFolds {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_collapsed(&self, block_start: usize, fold: Fold) -> bool {
        self.collapsed
            .get(&block_start)
            .copied()
            .unwrap_or(fold == Fold::Collapsed)
    }

    pub fn toggle(&mut self, block_start: usize, fold: Fold) {
        let next = !self.is_collapsed(block_start, fold);
        self.collapsed.insert(block_start, next);
    }

    pub fn map_through(&mut self, edits: &[TextEdit]) {
        if edits.is_empty() || self.collapsed.is_empty() {
            return;
        }
        let previous = mem::take(&mut self.collapsed);
        for (position, collapsed) in previous {
            self.collapsed
                .insert(map_position(position, edits), collapsed);
        }
    }
}

fn map_position(position: usize, edits: &[TextEdit]) -> usize {
    let mut shift = 0;
    let mut base = position;
    for edit in edits {
        let span = edit.span;
        if span.end < position || (span.end == position && span.start < position) {
            shift += signed(edit.insert.len()) - signed(span.len());
        } else if span.start < position {
            base = span.start;
        }
    }
    base.saturating_add_signed(shift)
}
fn signed(value: usize) -> isize {
    isize::try_from(value).expect("note text fits in isize")
}

#[cfg(test)]
mod tests {
    use margin_markdown::Span;

    use super::*;

    #[test]
    fn toggling_flips_the_default_fold() {
        let mut folds = CalloutFolds::new();
        assert!(folds.is_collapsed(4, Fold::Collapsed));
        folds.toggle(4, Fold::Collapsed);
        assert!(!folds.is_collapsed(4, Fold::Collapsed));
        folds.toggle(9, Fold::Expanded);
        assert!(folds.is_collapsed(9, Fold::Expanded));
    }

    #[test]
    fn positions_follow_edits_before_them() {
        let edits = [
            TextEdit::new(Span::new(0, 2), "abcd"),
            TextEdit::new(Span::new(20, 20), "x"),
        ];
        assert_eq!(map_position(10, &edits), 12);
        assert_eq!(map_position(1, &edits), 0);
        assert_eq!(map_position(20, &edits), 22);
        assert_eq!(map_position(0, &edits), 0);
        let mut folds = CalloutFolds::new();
        folds.toggle(10, Fold::Expanded);
        folds.map_through(&edits);
        assert!(folds.is_collapsed(12, Fold::Expanded));
    }
}
