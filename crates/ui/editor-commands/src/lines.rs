use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, Assoc, RangeEdit};
use crate::text::{self, Line};
use crate::{prefix, tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    Move,
    Extend,
}

fn in_fence(document: &Document, pos: usize) -> bool {
    tree::fenced_code_at(document, pos).is_some()
}

fn content_start_of(document: &Document, head: usize, row_start: usize) -> Option<usize> {
    let line = Line::at(document, head);
    if in_fence(document, line.start) {
        return None;
    }
    let start = line.start + prefix::line_prefix(line.text).len();
    if start == line.start || row_start > start {
        return None;
    }
    Some(if head > start { start } else { line.start })
}

#[must_use]
pub fn line_content_start(
    document: &Document,
    selections: &[Selection],
    mode: SelectionMode,
    visual_row_start: impl Fn(usize) -> usize,
) -> Option<Change> {
    let targets: Vec<Option<usize>> = selections
        .iter()
        .map(|range| content_start_of(document, range.head, visual_row_start(range.head)))
        .collect();
    if targets.iter().all(Option::is_none) {
        return None;
    }
    let moved = selections
        .iter()
        .zip(targets)
        .map(|(range, target)| {
            let target = target.unwrap_or_else(|| visual_row_start(range.head));
            match mode {
                SelectionMode::Move => Selection::cursor(target),
                SelectionMode::Extend => Selection::new(range.anchor, target),
            }
        })
        .collect();
    Some(Change::default().with_selections(moved))
}

#[must_use]
pub fn delete_to_line_start(
    document: &Document,
    selections: &[Selection],
    visual_row_start: impl Fn(usize) -> usize,
) -> Option<Change> {
    let [range] = selections else {
        return None;
    };
    if !range.is_empty() {
        return None;
    }
    let line = Line::at(document, range.head);
    if in_fence(document, line.start) {
        return None;
    }
    let start = line.start + prefix::line_prefix(line.text).len();
    let row = visual_row_start(range.head);
    if row == line.start || row > start {
        return None;
    }
    let edits = vec![TextEdit::new(Span::new(line.start, range.head), "")];
    Some(Change::new(edits).with_selections(vec![Selection::cursor(line.start)]))
}

#[must_use]
pub fn soft_break(document: &Document, selections: &[Selection]) -> Option<Change> {
    if selections
        .iter()
        .any(|range| in_fence(document, range.start()))
    {
        return None;
    }
    mapping::by_range(selections, |range| {
        let found = prefix::line_prefix(Line::at(document, range.start()).text);
        let width = if found.marker.is_empty() {
            0
        } else {
            found.marker.len() + found.gap.len() + found.task.len()
        };
        let insert = format!("\n{}{}", found.head, " ".repeat(width));
        let cursor = range.start() + insert.len();
        Some(RangeEdit::replace(range.span(), insert, cursor))
    })
}

#[must_use]
pub fn toggle_checklist(document: &Document, selections: &[Selection]) -> Change {
    let mut edits = Vec::new();
    let mut seen = Vec::new();
    for range in selections {
        for index in text::selected_lines(document, *range) {
            if seen.contains(&index) {
                continue;
            }
            seen.push(index);
            let line = Line::numbered(document, index);
            let found = prefix::line_prefix(line.text);
            let after_marker = line.start + found.head.len() + found.marker.len() + found.gap.len();
            if let Some(state) = found.task.chars().nth(1) {
                let flipped = if state == ' ' { "x" } else { " " };
                edits.push(TextEdit::new(
                    Span::new(after_marker + 1, after_marker + 2),
                    flipped,
                ));
            } else if found.marker.is_empty() {
                edits.push(TextEdit::new(
                    Span::empty(line.start + found.head.len()),
                    "- [ ] ",
                ));
            } else {
                edits.push(TextEdit::new(Span::empty(after_marker), "[ ] "));
            }
        }
    }
    let edits = mapping::sorted(edits);
    let moved = selections
        .iter()
        .map(|range| {
            if range.is_empty() {
                Selection::cursor(mapping::map_pos(&edits, range.head, Assoc::After))
            } else {
                mapping::map_range(&edits, *range)
            }
        })
        .collect();
    Change::new(edits).with_selections(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    #[test]
    fn home_jumps_to_the_content_after_the_marker() {
        let home = |mode| {
            move |d: &Document, s: &[Selection]| {
                line_content_start(d, s, mode, |pos| Line::at(d, pos).start)
            }
        };
        assert_eq!(
            run("- [ ] ta|sk", home(SelectionMode::Move)).as_deref(),
            Some("- [ ] |task")
        );
        assert_eq!(
            run("> - |a", home(SelectionMode::Move)).as_deref(),
            Some("|> - a")
        );
        assert_eq!(
            run("- ta|sk", home(SelectionMode::Extend)).as_deref(),
            Some("- «ta»sk")
        );
        assert_eq!(run("pla|in", home(SelectionMode::Move)), None);
    }

    #[test]
    fn deletes_to_the_line_start_from_a_wrapped_row() {
        let delete = |d: &Document, s: &[Selection]| delete_to_line_start(d, s, |_| 2);
        assert_eq!(run("- ab|c", delete).as_deref(), Some("|c"));
        let first_row = |d: &Document, s: &[Selection]| {
            delete_to_line_start(d, s, |pos| Line::at(d, pos).start)
        };
        assert_eq!(run("- ab|c", first_row), None);
    }

    #[test]
    fn soft_breaks_align_under_the_item_text() {
        assert_eq!(
            run("- [ ] a|b", soft_break).as_deref(),
            Some("- [ ] a\n      |b")
        );
        assert_eq!(run("> quote|", soft_break).as_deref(), Some("> quote\n> |"));
        assert_eq!(run("```\nco|de\n```", soft_break), None);
    }

    #[test]
    fn cycles_checklists() {
        let cycle = |d: &Document, s: &[Selection]| Some(toggle_checklist(d, s));
        assert_eq!(run("ta|sk", cycle).as_deref(), Some("- [ ] ta|sk"));
        assert_eq!(run("- ta|sk", cycle).as_deref(), Some("- [ ] ta|sk"));
        assert_eq!(run("- [ ] ta|sk", cycle).as_deref(), Some("- [x] ta|sk"));
        assert_eq!(
            run("> 1. [X] ta|sk", cycle).as_deref(),
            Some("> 1. [ ] ta|sk")
        );
    }
}
