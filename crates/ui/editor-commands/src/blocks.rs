use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, RangeEdit};
use crate::prefix;
use crate::text::{self, Line};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    Bullet,
    Ordered,
    Task,
}

fn primary_lines<'document>(
    document: &'document Document,
    selections: &[Selection],
    primary: usize,
) -> Vec<Line<'document>> {
    selections
        .get(primary)
        .or_else(|| selections.last())
        .map(|selection| {
            text::selected_lines(document, *selection)
                .map(|index| Line::numbered(document, index))
                .collect()
        })
        .unwrap_or_default()
}

fn line_change(edits: Vec<TextEdit>) -> Option<Change> {
    (!edits.is_empty()).then(|| Change::new(edits))
}

#[must_use]
pub fn toggle_heading(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    level: u8,
) -> Option<Change> {
    let wanted = format!("{} ", "#".repeat(usize::from(level.clamp(1, 6))));
    let mut edits = Vec::new();
    for line in primary_lines(document, selections, primary) {
        let edit = match prefix::heading(line.text) {
            None => TextEdit::new(Span::empty(line.start), wanted.as_str()),
            Some((hashes, length)) if hashes == usize::from(level) => {
                TextEdit::new(Span::new(line.start, line.start + length), "")
            }
            Some((_, length)) => {
                TextEdit::new(Span::new(line.start, line.start + length), wanted.as_str())
            }
        };
        edits.push(edit);
    }
    line_change(edits)
}

#[must_use]
pub fn toggle_list(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    kind: ListKind,
) -> Option<Change> {
    let marker = if kind == ListKind::Ordered { "1." } else { "-" };
    let wanted = if kind == ListKind::Task {
        format!("{marker} [ ] ")
    } else {
        format!("{marker} ")
    };
    let mut edits = Vec::new();
    for line in primary_lines(document, selections, primary) {
        let Some(found) = prefix::spaced_marker(line.text) else {
            edits.push(TextEdit::new(Span::empty(line.start), wanted.as_str()));
            continue;
        };
        let is_task = !found.task.is_empty();
        let current = if found.is_ordered() {
            ListKind::Ordered
        } else {
            ListKind::Bullet
        };
        let same = if kind == ListKind::Task {
            is_task
        } else {
            current == kind && !is_task
        };
        let insert = if same {
            found.indent.to_owned()
        } else {
            format!("{}{wanted}", found.indent)
        };
        edits.push(TextEdit::new(
            Span::new(line.start, line.start + found.len()),
            insert,
        ));
    }
    line_change(edits)
}

#[must_use]
pub fn toggle_quote(
    document: &Document,
    selections: &[Selection],
    primary: usize,
) -> Option<Change> {
    let lines = primary_lines(document, selections, primary);
    let quoted = lines.iter().all(|line| prefix::quote(line.text).is_some());
    let mut edits = Vec::new();
    for line in lines {
        if !quoted {
            edits.push(TextEdit::new(Span::empty(line.start), "> "));
        } else if let Some(length) = prefix::quote(line.text) {
            edits.push(TextEdit::new(
                Span::new(line.start, line.start + length),
                "",
            ));
        }
    }
    line_change(edits)
}

#[must_use]
pub fn exit_empty_quote(
    document: &Document,
    selections: &[Selection],
    primary: usize,
) -> Option<Change> {
    let range = *selections.get(primary).or_else(|| selections.last())?;
    if !range.is_empty() {
        return None;
    }
    let line = Line::at(document, range.head);
    if range.head != line.end || !prefix::is_empty_quote(line.text) {
        return None;
    }
    let mut moved = selections.to_vec();
    let delta = 1 - mapping::signed(line.end - line.start);
    for selection in &mut moved {
        *selection = if *selection == range {
            Selection::cursor(line.start + 1)
        } else if selection.start() >= line.end {
            Selection::new(
                selection.anchor.saturating_add_signed(delta),
                selection.head.saturating_add_signed(delta),
            )
        } else {
            *selection
        };
    }
    Some(Change::new(vec![TextEdit::new(line.span(), "\n")]).with_selections(moved))
}

#[must_use]
pub fn insert_link(document: &Document, selections: &[Selection]) -> Change {
    let source = document.text();
    mapping::by_range(selections, |range| {
        let label = range.span().slice(source);
        let offset = if label.is_empty() { 1 } else { label.len() + 3 };
        Some(RangeEdit::replace(
            range.span(),
            format!("[{label}]()"),
            range.start() + offset,
        ))
    })
    .unwrap_or_default()
}

fn existing_label(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('[')?.strip_suffix(')')?;
    let (label, destination) = rest.split_once("](")?;
    (!label.contains(']') && !destination.contains(')')).then_some(label)
}

#[must_use]
pub fn set_link(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    url: &str,
) -> Option<Change> {
    let range = *selections.get(primary).or_else(|| selections.last())?;
    let selected = range.span().slice(document.text());
    let label = existing_label(selected).unwrap_or(selected);
    let insert = format!("[{label}]({url})");
    let cursor = range.start() + insert.len();
    let edits = vec![TextEdit::new(range.span(), insert)];
    Some(Change::new(edits).with_selections(vec![Selection::cursor(cursor)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{primary, run};

    #[test]
    fn toggles_headings_per_line() {
        let heading =
            |level| move |d: &Document, s: &[Selection]| toggle_heading(d, s, primary(s), level);
        assert_eq!(run("ti|tle", heading(2)).as_deref(), Some("## ti|tle"));
        assert_eq!(run("## ti|tle", heading(2)).as_deref(), Some("ti|tle"));
        assert_eq!(
            run("# «a\nb»", heading(3)).as_deref(),
            Some("### «a\n### b»")
        );
    }

    #[test]
    fn toggles_lists() {
        let list = |kind| move |d: &Document, s: &[Selection]| toggle_list(d, s, primary(s), kind);
        assert_eq!(
            run("it|em", list(ListKind::Bullet)).as_deref(),
            Some("- it|em")
        );
        assert_eq!(
            run("- it|em", list(ListKind::Bullet)).as_deref(),
            Some("it|em")
        );
        assert_eq!(
            run("  - it|em", list(ListKind::Ordered)).as_deref(),
            Some("  1. it|em")
        );
        assert_eq!(
            run("- [x] it|em", list(ListKind::Task)).as_deref(),
            Some("it|em")
        );
        assert_eq!(
            run("3. it|em", list(ListKind::Task)).as_deref(),
            Some("- [ ] it|em")
        );
    }

    #[test]
    fn toggles_quotes_for_all_lines() {
        let quote = |d: &Document, s: &[Selection]| toggle_quote(d, s, primary(s));
        assert_eq!(run("«a\n> b»", quote).as_deref(), Some("> «a\n> > b»"));
        assert_eq!(run("> «a\n>b»", quote).as_deref(), Some("«a\nb»"));
    }

    #[test]
    fn leaves_an_empty_quote_line() {
        let exit = |d: &Document, s: &[Selection]| exit_empty_quote(d, s, primary(s));
        assert_eq!(run("> a\n> |", exit).as_deref(), Some("> a\n\n|"));
        assert_eq!(run("> a\n> b|", exit), None);
    }

    #[test]
    fn inserts_and_sets_links() {
        assert_eq!(
            run("a «word»", |d, s| Some(insert_link(d, s))).as_deref(),
            Some("a [word](|)")
        );
        assert_eq!(
            run("a |", |d, s| Some(insert_link(d, s))).as_deref(),
            Some("a [|]()")
        );
        let link = |d: &Document, s: &[Selection]| set_link(d, s, primary(s), "https://x.y");
        assert_eq!(
            run("«[old](a)»", link).as_deref(),
            Some("[old](https://x.y)|")
        );
        assert_eq!(run("«text»", link).as_deref(), Some("[text](https://x.y)|"));
    }
}
