use margin_markdown::Document;
use margin_text_buffer::{Change, Selection};

use crate::mapping;

pub(crate) fn parse_cursor(source: &str) -> (Document, Vec<Selection>) {
    let mut text = String::with_capacity(source.len());
    let mut selections = Vec::new();
    let mut anchor = None;
    for character in source.chars() {
        match character {
            '|' => selections.push(Selection::cursor(text.len())),
            '«' => anchor = Some(text.len()),
            '»' => selections.push(Selection::new(
                anchor.take().expect("a selection opens before it closes"),
                text.len(),
            )),
            _ => text.push(character),
        }
    }
    (Document::parse(&text), selections)
}

pub(crate) fn apply(document: &Document, change: &Change) -> String {
    mapping::apply(document.text(), &mapping::sorted(change.edits.clone()))
}

pub(crate) fn cursor_text(text: &str, selections: &[Selection]) -> String {
    let mut marks: Vec<(usize, char)> = Vec::new();
    for selection in selections {
        if selection.is_empty() {
            marks.push((selection.head, '|'));
        } else {
            marks.push((selection.start(), '«'));
            marks.push((selection.end(), '»'));
        }
    }
    marks.sort_by_key(|(at, mark)| (*at, *mark == '«'));
    let mut out = text.to_owned();
    for (at, mark) in marks.into_iter().rev() {
        out.insert(at, mark);
    }
    out
}

pub(crate) fn primary(selections: &[Selection]) -> usize {
    selections.len().saturating_sub(1)
}

pub(crate) fn run(
    source: &str,
    command: impl FnOnce(&Document, &[Selection]) -> Option<Change>,
) -> Option<String> {
    let (document, selections) = parse_cursor(source);
    let change = command(&document, &selections)?;
    let edits = mapping::sorted(change.edits.clone());
    let after = change.selections.clone().unwrap_or_else(|| {
        selections
            .iter()
            .map(|selection| mapping::map_range(&edits, *selection))
            .collect()
    });
    Some(cursor_text(&apply(&document, &change), &after))
}
