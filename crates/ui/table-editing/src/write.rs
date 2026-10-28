use margin_markdown::{Align, CellPlace, Document, Span, TableModel, TextEdit, rewrite};
use margin_text_buffer::{Change, Selection};

use crate::locate::extent;

pub(crate) struct Written {
    pub edits: Vec<TextEdit>,
    pub text: String,
    pub cell_starts: Vec<Vec<usize>>,
}

impl Written {
    pub(crate) fn cell_start(&self, table: &TableModel, place: CellPlace) -> usize {
        let local = self
            .cell_starts
            .get(place.row)
            .and_then(|row| row.get(place.col))
            .copied()
            .unwrap_or(0);
        table.span.start + local
    }

    pub(crate) fn end(&self, table: &TableModel) -> usize {
        table.span.start + self.text.len()
    }
}

pub(crate) fn rows_of(table: &TableModel) -> Vec<Vec<String>> {
    table.text_rows()
}

pub(crate) fn render(
    document: &Document,
    table: &TableModel,
    rows: &[Vec<String>],
    align: &[Align],
) -> Written {
    let rewritten = rewrite(table, document.text(), rows, align);
    let old = extent(table).slice(document.text());
    Written {
        edits: replace(table.span.start, old, &rewritten.text),
        text: rewritten.text,
        cell_starts: rewritten.cell_starts,
    }
}

pub(crate) fn write_table(
    document: &Document,
    table: &TableModel,
    rows: &[Vec<String>],
    align: &[Align],
    select: Option<CellPlace>,
) -> Change {
    let written = render(document, table, rows, align);
    let change = Change::new(written.edits.clone());
    match select {
        Some(place) => {
            let text = rows
                .get(place.row)
                .and_then(|row| row.get(place.col))
                .map_or(0, String::len);
            let caret = written.cell_start(table, place) + text;
            change.with_selections(vec![Selection::cursor(caret)])
        }
        None => change,
    }
}

pub(crate) fn replace(base: usize, old: &str, new: &str) -> Vec<TextEdit> {
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();
    if old_lines.len() != new_lines.len() {
        return diff(old, new)
            .map(|edit| shifted(edit, base))
            .into_iter()
            .collect();
    }
    let mut edits = Vec::new();
    let mut at = base;
    for (before, after) in old_lines.iter().zip(&new_lines) {
        if let Some(edit) = diff(before, after) {
            edits.push(shifted(edit, at));
        }
        at += before.len() + 1;
    }
    edits
}

fn shifted(edit: TextEdit, by: usize) -> TextEdit {
    TextEdit::new(
        Span::new(edit.span.start + by, edit.span.end + by),
        edit.insert,
    )
}

fn diff(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let prefix = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, a), b)| a != b)
        .map_or_else(|| old.len().min(new.len()), |((index, _), _)| index);
    let limit = (old.len() - prefix).min(new.len() - prefix);
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .scan(0, |sum, len| {
            *sum += len;
            Some(*sum)
        })
        .take_while(|sum| *sum <= limit)
        .last()
        .unwrap_or(0);
    Some(TextEdit::new(
        Span::new(prefix, old.len() - suffix),
        &new[prefix..new.len() - suffix],
    ))
}

#[cfg(test)]
pub(crate) fn applied(source: &str, change: &Change) -> String {
    use std::cmp::Reverse;

    let mut edits = change.edits.clone();
    edits.sort_by_key(|edit| Reverse(edit.span.start));
    let mut text = source.to_owned();
    for edit in edits {
        text.replace_range(edit.span.start..edit.span.end, &edit.insert);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_only_the_changed_middle() {
        let edit = diff("| a  | b |", "| abc | b |").expect("changed");
        assert_eq!(
            (edit.span.start, edit.span.end, edit.insert.as_str()),
            (3, 4, "bc")
        );
    }

    #[test]
    fn diffs_per_line_when_line_count_is_stable() {
        let edits = replace(10, "ab\ncd", "ab\nxd");
        assert_eq!(edits, vec![TextEdit::new(Span::new(13, 14), "x")]);
    }
}
