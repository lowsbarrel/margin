use similar::{Algorithm, DiffTag};

const MAX_DIFF_LINES: usize = 20_000;
const MAX_CHANGED_LINES: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineKind {
    Same,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffLine {
    kind: LineKind,
    text: String,
    before: Option<usize>,
    after: Option<usize>,
}

impl DiffLine {
    pub(crate) const fn kind(&self) -> LineKind {
        self.kind
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn before(&self) -> Option<usize> {
        self.before
    }

    pub(crate) const fn after(&self) -> Option<usize> {
        self.after
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LineDiff {
    lines: Vec<DiffLine>,
    added: usize,
    removed: usize,
}

impl LineDiff {
    pub(crate) fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    pub(crate) const fn added(&self) -> usize {
        self.added
    }

    pub(crate) const fn removed(&self) -> usize {
        self.removed
    }

    pub(crate) fn between(before: &str, after: &str) -> Option<Self> {
        let old = split_lines(before);
        let new = split_lines(after);
        if old.len() > MAX_DIFF_LINES
            || new.len() > MAX_DIFF_LINES
            || old.len().abs_diff(new.len()) > MAX_CHANGED_LINES
        {
            return None;
        }
        let mut diff = Self::default();
        for op in similar::capture_diff_slices(Algorithm::Myers, &old, &new) {
            let (tag, old_range, new_range) = op.as_tag_tuple();
            if tag == DiffTag::Equal {
                for (before, after) in old_range.zip(new_range) {
                    diff.push(LineKind::Same, old[before], Some(before), Some(after));
                }
                continue;
            }
            for before in old_range {
                diff.push(LineKind::Removed, old[before], Some(before), None);
            }
            for after in new_range {
                diff.push(LineKind::Added, new[after], None, Some(after));
            }
        }
        (diff.added + diff.removed <= MAX_CHANGED_LINES).then_some(diff)
    }

    fn push(&mut self, kind: LineKind, text: &str, before: Option<usize>, after: Option<usize>) {
        match kind {
            LineKind::Added => self.added += 1,
            LineKind::Removed => self.removed += 1,
            LineKind::Same => {}
        }
        self.lines.push(DiffLine {
            kind,
            text: text.to_owned(),
            before: before.map(|index| index + 1),
            after: after.map(|index| index + 1),
        });
    }
}

fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: LineKind, text: &str, before: Option<usize>, after: Option<usize>) -> DiffLine {
        DiffLine {
            kind,
            text: text.to_owned(),
            before,
            after,
        }
    }

    #[test]
    fn numbers_each_side_and_orders_removals_before_additions() {
        let diff = LineDiff::between("a\nb\nc", "a\nB\nc\nd").expect("small diff");
        assert_eq!(
            diff.lines,
            vec![
                line(LineKind::Same, "a", Some(1), Some(1)),
                line(LineKind::Removed, "b", Some(2), None),
                line(LineKind::Added, "B", None, Some(2)),
                line(LineKind::Same, "c", Some(3), Some(3)),
                line(LineKind::Added, "d", None, Some(4)),
            ]
        );
        assert_eq!((diff.added, diff.removed), (2, 1));
    }

    #[test]
    fn crlf_line_endings_compare_equal_to_lf() {
        let diff = LineDiff::between("one\r\ntwo", "one\ntwo").expect("small diff");
        assert_eq!((diff.added, diff.removed), (0, 0));
        assert_eq!(diff.lines.len(), 2);
    }

    #[test]
    fn refuses_inputs_too_large_to_compare() {
        let huge = "x\n".repeat(MAX_DIFF_LINES + 1);
        assert_eq!(LineDiff::between(&huge, ""), None);
        let before = (0..1500)
            .map(|n| format!("old {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let after = (0..1500)
            .map(|n| format!("new {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(LineDiff::between(&before, &after), None);
    }
}
