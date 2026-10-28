use margin_markdown::{BlockKind, Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::list_tree::{self, Indent};
use crate::mapping::{self, RangeEdit};
use crate::markup_context::{self, Context, ContextKind};
use crate::text::{self, Line};
use crate::{blocks, tree};

fn in_foreign_code(document: &Document, pos: usize) -> bool {
    tree::fenced_code_at(document, pos).is_some_and(
        |code| matches!(&code.kind, BlockKind::CodeBlock { info, .. } if !info.trim().is_empty()),
    )
}

fn blank_line(document: &Document, contexts: &[Context<'_>], line: &Line<'_>) -> String {
    let mut insert = String::new();
    let Some(last) = contexts.len().checked_sub(2) else {
        return insert;
    };
    for (index, context) in contexts.iter().enumerate().take(last + 1) {
        let width = (index < last).then(|| {
            let next = contexts.get(index + 1).map_or(0, |next| next.from);
            text::count_column(line.text, next).saturating_sub(insert.len())
        });
        insert.push_str(&context.blank(width, index < last));
    }
    markup_context::normalize_indent(document, &insert)
}

fn leave_list_level(
    document: &Document,
    contexts: &[Context<'_>],
    inner: &Context<'_>,
    line: &Line<'_>,
    pos: usize,
) -> RangeEdit {
    let next = contexts
        .len()
        .checked_sub(2)
        .and_then(|index| contexts.get(index));
    let (delete_to, insert) = match next {
        Some(next) if next.item.is_some() => (line.start + next.from, next.next_marker(document)),
        Some(next) => (line.start + next.to, String::new()),
        None => (line.start, String::new()),
    };
    let mut edits = vec![TextEdit::new(Span::new(delete_to, pos), insert.as_str())];
    if inner.kind == ContextKind::Ordered
        && let (Some(list), Some(item)) = (inner.list, inner.item)
    {
        markup_context::renumber_after(document, list, item, -2, &mut edits);
    }
    if let Some(next) = next
        && next.kind == ContextKind::Ordered
        && let (Some(list), Some(item)) = (next.list, next.item)
    {
        markup_context::renumber_after(document, list, item, 0, &mut edits);
    }
    RangeEdit::new(
        mapping::sorted(edits),
        Selection::cursor(delete_to + insert.len()),
    )
}

fn close_quote(
    document: &Document,
    inner: &Context<'_>,
    line: &Line<'_>,
    range: Selection,
) -> Option<RangeEdit> {
    let previous = Line::numbered(document, line.index.checked_sub(1)?);
    let trimmed = previous.text.trim_end();
    let quoted = trimmed.strip_suffix('>').map(str::len)?;
    if quoted != inner.from {
        return None;
    }
    let edits = vec![
        TextEdit::new(Span::new(previous.start + quoted, previous.end), ""),
        TextEdit::new(Span::new(line.start + inner.from, line.end), ""),
    ];
    let selection = mapping::map_range(&edits, range);
    Some(RangeEdit::new(edits, selection))
}

fn continue_range(document: &Document, range: Selection) -> Option<RangeEdit> {
    if !range.is_empty() || in_foreign_code(document, range.head) {
        return None;
    }
    let pos = range.head;
    let line = Line::at(document, pos);
    let column = pos - line.start;
    let mut contexts = markup_context::contexts(document, pos);
    while contexts.last().is_some_and(|context| context.from > column) {
        contexts.pop();
    }
    let inner = contexts.last()?.clone();
    if inner.to - inner.space_after.len() > column {
        return None;
    }
    let empty_line = column >= inner.to - inner.space_after.len()
        && text::slice(line.text, inner.to, line.text.len())
            .trim()
            .is_empty();
    if inner.item.is_some() && empty_line {
        return Some(leave_list_level(document, &contexts, &inner, &line, pos));
    }
    if inner.kind == ContextKind::Quote
        && empty_line
        && line.start > 0
        && let Some(closed) = close_quote(document, &inner, &line, range)
    {
        return Some(closed);
    }
    let mut edits = Vec::new();
    if inner.kind == ContextKind::Ordered
        && let (Some(list), Some(item)) = (inner.list, inner.item)
    {
        markup_context::renumber_after(document, list, item, 0, &mut edits);
    }
    let continued = inner
        .item
        .is_some_and(|item| markup_context::node_start(document, item) < line.start);
    let mut insert = String::new();
    let lead = line.text.len()
        - line
            .text
            .trim_start_matches(|c: char| c.is_whitespace() || "0123456789.)-+*>".contains(c))
            .len();
    if !continued || lead >= inner.to {
        let last = contexts.len() - 1;
        for (index, context) in contexts.iter().enumerate() {
            if index == last && !continued {
                insert.push_str(&context.next_marker(document));
            } else {
                let width = (index < last).then(|| {
                    let next = contexts.get(index + 1).map_or(0, |next| next.from);
                    text::count_column(line.text, next).saturating_sub(insert.len())
                });
                insert.push_str(&context.blank(width, true));
            }
        }
    }
    let mut from = pos;
    while from > line.start
        && text::char_before(document.text(), from).is_some_and(char::is_whitespace)
    {
        from -= 1;
    }
    let mut insert = markup_context::normalize_indent(document, &insert);
    if inner
        .list
        .is_some_and(|list| markup_context::non_tight_list(document, list))
    {
        insert = format!("{}\n{insert}", blank_line(document, &contexts, &line));
    }
    let cursor = from + insert.len() + 1;
    edits.push(TextEdit::new(Span::new(from, pos), format!("\n{insert}")));
    Some(RangeEdit::new(
        mapping::sorted(edits),
        Selection::cursor(cursor),
    ))
}

#[must_use]
pub fn continue_markup(document: &Document, selections: &[Selection]) -> Option<Change> {
    mapping::by_range(selections, |range| continue_range(document, range))
}

fn plain_newline(document: &Document, range: Selection) -> RangeEdit {
    let source = document.text();
    let line = Line::at(document, range.start());
    let (mut from, mut to) = (range.start(), range.end());
    let around = text::slice(source, from.saturating_sub(1), to + 1);
    let explode = range.is_empty() && matches!(around, "()" | "[]" | "{}");
    let style = if list_tree::detect_indent_unit(source) == "\t" {
        Indent::Tabs
    } else {
        Indent::Spaces
    };
    let indent = list_tree::indent_text(text::columns(text::leading_space(line.text)), style);
    while to < line.end && text::char_at(source, to).is_some_and(char::is_whitespace) {
        to += 1;
    }
    if explode {
        to = range.end();
    } else if from > line.start
        && from < line.start + 100
        && text::slice(source, line.start, from).trim().is_empty()
    {
        from = line.start;
    }
    let mut insert = format!("\n{indent}");
    let cursor = from + insert.len();
    if explode {
        insert.push('\n');
        insert.push_str(&indent);
    }
    RangeEdit::replace(Span::new(from, to), insert, cursor)
}

#[must_use]
pub fn insert_newline(document: &Document, selections: &[Selection], primary: usize) -> Change {
    blocks::exit_empty_quote(document, selections, primary)
        .or_else(|| continue_markup(document, selections))
        .or_else(|| mapping::by_range(selections, |range| Some(plain_newline(document, range))))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{primary, run};

    fn enter(source: &str) -> String {
        run(source, |d, s| Some(insert_newline(d, s, primary(s)))).expect("enter always applies")
    }

    #[test]
    fn continues_bullets_and_tasks() {
        assert_eq!(enter("- a|"), "- a\n- |");
        assert_eq!(enter("* [x] done|"), "* [x] done\n* [ ] |");
        assert_eq!(enter("- a\n  - b|"), "- a\n  - b\n  - |");
    }

    #[test]
    fn continues_and_renumbers_ordered_lists() {
        assert_eq!(enter("1. a|\n2. b"), "1. a\n2. |\n3. b");
        assert_eq!(enter("3) c|"), "3) c\n4) |");
    }

    #[test]
    fn exits_an_empty_item() {
        assert_eq!(enter("- a\n- |"), "- a\n|");
        assert_eq!(enter("- a\n  - b\n  - |"), "- a\n  - b\n- |");
        assert_eq!(enter("1. a\n2. |\n3. b"), "1. a\n|\n2. b");
    }

    #[test]
    fn continues_quotes_and_callouts() {
        assert_eq!(enter("> a|"), "> a\n> |");
        assert_eq!(
            enter("> [!note] Title\n> body|"),
            "> [!note] Title\n> body\n> |"
        );
        assert_eq!(enter("> a\n> |"), "> a\n\n|");
        assert_eq!(enter("> - a|"), "> - a\n> - |");
    }

    #[test]
    fn keeps_loose_lists_loose() {
        assert_eq!(enter("- a\n\n- b|"), "- a\n\n- b\n\n- |");
    }

    #[test]
    fn falls_back_to_a_plain_newline() {
        assert_eq!(enter("text|"), "text\n|");
        assert_eq!(enter("  text|"), "  text\n  |");
        assert_eq!(enter("f(|)"), "f(\n|\n)");
        assert_eq!(enter("```js\nlet a|\n```"), "```js\nlet a\n|\n```");
    }
}
