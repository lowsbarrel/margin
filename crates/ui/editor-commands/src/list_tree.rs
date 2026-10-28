use margin_markdown::{Block, BlockKind, Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::text::{self, Line, TAB_SIZE};
use crate::{mapping, prefix, tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListItem {
    pub from: usize,
    pub to: usize,
    pub mark: usize,
    pub level: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListBlock {
    pub from: usize,
    pub to: usize,
    pub roots: Vec<usize>,
    pub items: Vec<ListItem>,
}

impl ListBlock {
    pub(crate) fn item(&self, index: usize) -> &ListItem {
        self.items
            .get(index)
            .expect("list item indices come from the same list")
    }

    pub(crate) fn siblings(&self, index: usize) -> &[usize] {
        match self.item(index).parent {
            Some(parent) => &self.item(parent).children,
            None => &self.roots,
        }
    }

    pub(crate) fn sibling(&self, index: usize, step: isize) -> Option<usize> {
        let siblings = self.siblings(index);
        let at = siblings.iter().position(|entry| *entry == index)?;
        siblings.get(at.checked_add_signed(step)?).copied()
    }

    pub(crate) fn item_at(&self, pos: usize) -> Option<usize> {
        let mut found: Option<usize> = None;
        for (index, item) in self.items.iter().enumerate() {
            let deeper = found.is_none_or(|best| item.level > self.item(best).level);
            if item.from <= pos && pos <= item.to && deeper {
                found = Some(index);
            }
        }
        found
    }

    pub(crate) fn index_of_mark(&self, mark: usize) -> Option<usize> {
        self.items.iter().position(|item| item.mark == mark)
    }
}

pub(crate) fn line_span(document: &Document, from: usize, to: usize) -> Span {
    let first = Line::at(document, from);
    let mut last = Line::at(document, to);
    if to == last.start && last.index > first.index {
        last = Line::numbered(document, last.index - 1);
    }
    Span::new(first.start, last.end)
}

pub(crate) fn indent_of<'text>(document: &'text Document, item: &ListItem) -> &'text str {
    text::slice(document.text(), item.from, item.mark)
}

fn collect(
    document: &Document,
    list: &Block,
    parent: Option<usize>,
    level: usize,
    out: &mut Vec<ListItem>,
) -> Vec<usize> {
    let mut siblings = Vec::new();
    for node in &list.children {
        let Some(mark) = node.markers.first().map(|marker| marker.start) else {
            continue;
        };
        let line = Line::at(document, node.span.start);
        if line.start + text::leading_space(line.text).len() != mark {
            continue;
        }
        let span = line_span(document, node.span.start, node.span.end);
        let index = out.len();
        out.push(ListItem {
            from: span.start,
            to: span.end,
            mark,
            level,
            parent,
            children: Vec::new(),
        });
        let mut children = Vec::new();
        for child in node
            .children
            .iter()
            .filter(|child| tree::is_list(&child.kind))
        {
            children.extend(collect(document, child, Some(index), level + 1, out));
        }
        if let Some(item) = out.get_mut(index) {
            item.children = children;
        }
        siblings.push(index);
    }
    siblings
}

fn top_list(document: &Document, pos: usize) -> Option<&Block> {
    document.blocks().iter().find(|block| {
        tree::is_list(&block.kind) && block.span.start <= pos && pos <= block.span.end
    })
}

pub(crate) fn list_at(document: &Document, pos: usize) -> Option<ListBlock> {
    let top = top_list(document, pos)?;
    let mut items = Vec::new();
    let roots = collect(document, top, None, 1, &mut items);
    let span = line_span(document, top.span.start, top.span.end);
    (!roots.is_empty()).then_some(ListBlock {
        from: span.start,
        to: span.end,
        roots,
        items,
    })
}

#[must_use]
pub fn detect_indent_unit(text: &str) -> &'static str {
    for line in text.lines() {
        let indent = text::leading_space(line);
        let indented = !indent.is_empty()
            && prefix::marker(line).is_some_and(|found| {
                !found.space.is_empty() || found.indent.len() + found.token.len() == line.len()
            });
        if indented {
            return if indent.starts_with(' ') { "  " } else { "\t" };
        }
    }
    "\t"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Indent {
    Tabs,
    Spaces,
}

pub(crate) fn indent_style(document: &Document, indents: &[&str]) -> Indent {
    if detect_indent_unit(document.text()) == "\t"
        || indents.iter().any(|indent| indent.contains('\t'))
    {
        Indent::Tabs
    } else {
        Indent::Spaces
    }
}

pub(crate) fn indent_text(columns: usize, indent: Indent) -> String {
    match indent {
        Indent::Tabs => format!(
            "{}{}",
            "\t".repeat(columns / TAB_SIZE),
            " ".repeat(columns % TAB_SIZE)
        ),
        Indent::Spaces => " ".repeat(columns),
    }
}

fn first_line<'text>(document: &'text Document, item: &ListItem) -> &'text str {
    Line::at(document, item.from).text
}

pub(crate) fn child_column(document: &Document, item: &ListItem, indent: Indent) -> usize {
    let own = text::columns(indent_of(document, item));
    if indent == Indent::Tabs {
        return own + TAB_SIZE;
    }
    match prefix::marker(first_line(document, item)) {
        None => own + 2,
        Some(found) => {
            let space = found.space.len();
            own + found.token.len() + if space == 0 || space > 4 { 1 } else { space }
        }
    }
}

pub(crate) fn token_for(
    document: &Document,
    reference: Option<&ListItem>,
    item: &ListItem,
) -> Option<String> {
    let own = prefix::marker(first_line(document, item))?;
    let Some(reference) = reference else {
        return own
            .number
            .filter(|number| *number != 1)
            .map(|_| format!("1{}", own.delimiter()));
    };
    let theirs = prefix::marker(first_line(document, reference))?;
    (theirs.token != own.token).then(|| theirs.token.to_owned())
}

pub(crate) struct Relevel {
    pub text: String,
    pub old_starts: Vec<usize>,
    pub new_starts: Vec<usize>,
    pub old_prefix: Vec<usize>,
    pub new_prefix: Vec<usize>,
}

impl Relevel {
    pub(crate) fn map(&self, offset: usize) -> usize {
        let index = self
            .old_starts
            .iter()
            .rposition(|start| *start <= offset)
            .unwrap_or(0);
        let entry = |values: &[usize]| values.get(index).copied().unwrap_or(0);
        let within = offset - entry(&self.old_starts);
        let (before, after) = (entry(&self.old_prefix), entry(&self.new_prefix));
        let moved = if within >= before {
            within - before + after
        } else {
            within.min(after)
        };
        entry(&self.new_starts) + moved
    }
}

pub(crate) fn relevel(
    document: &Document,
    item: &ListItem,
    columns: usize,
    indent: Indent,
    token: Option<&str>,
) -> Relevel {
    let source = text::slice(document.text(), item.from, item.to);
    let old_token = prefix::marker(source).map_or("", |found| found.token);
    let new_token = token.unwrap_or(old_token);
    let shift =
        mapping::signed(columns) - mapping::signed(text::columns(indent_of(document, item)));
    let widen = mapping::signed(new_token.len()) - mapping::signed(old_token.len());
    let mut out = Relevel {
        text: String::with_capacity(source.len()),
        old_starts: Vec::new(),
        new_starts: Vec::new(),
        old_prefix: Vec::new(),
        new_prefix: Vec::new(),
    };
    let (mut old_at, mut new_at) = (0, 0);
    for (index, line) in source.split('\n').enumerate() {
        let space = text::leading_space(line);
        let rest = line.get(space.len()..).unwrap_or_default();
        let (next, before, after) = if index == 0 {
            let lead = indent_text(columns, indent);
            let body = rest.get(old_token.len()..).unwrap_or_default();
            let after = lead.len() + new_token.len();
            (
                format!("{lead}{new_token}{body}"),
                space.len() + old_token.len(),
                after,
            )
        } else if rest.is_empty() {
            (line.to_owned(), space.len(), space.len())
        } else {
            let target = mapping::signed(text::columns(space)) + shift + widen;
            let lead = indent_text(usize::try_from(target.max(0)).unwrap_or(0), indent);
            let length = lead.len();
            (format!("{lead}{rest}"), space.len(), length)
        };
        if index > 0 {
            out.text.push('\n');
        }
        out.old_starts.push(old_at);
        out.new_starts.push(new_at);
        out.old_prefix.push(before);
        out.new_prefix.push(after);
        old_at += line.len() + 1;
        new_at += next.len() + 1;
        out.text.push_str(&next);
    }
    out
}

fn renumber_list(document: &Document, list: &Block, edits: &mut Vec<TextEdit>) {
    let ordered = matches!(list.kind, BlockKind::OrderedList { .. });
    let mut numbers: Vec<(Span, u64)> = Vec::new();
    for item in &list.children {
        let Some(mark) = item.markers.first() else {
            continue;
        };
        if ordered && let Some(found) = prefix::marker(mark.slice(document.text())) {
            let digits = found.digits();
            if let Ok(value) = digits.parse::<u64>() {
                numbers.push((Span::new(mark.start, mark.start + digits.len()), value));
            }
        }
        for child in item
            .children
            .iter()
            .filter(|child| tree::is_list(&child.kind))
        {
            renumber_list(document, child, edits);
        }
    }
    let Some(first) = numbers.first().map(|(_, value)| *value) else {
        return;
    };
    if numbers.len() < 2 || numbers.len() >= 3 && numbers.iter().all(|(_, value)| *value == first) {
        return;
    }
    let start = numbers
        .iter()
        .map(|(_, value)| *value)
        .min()
        .unwrap_or(first);
    for ((span, value), want) in numbers.into_iter().zip(start..) {
        if value != want {
            edits.push(TextEdit::new(span, want.to_string()));
        }
    }
}

fn renumber_at(document: &Document, positions: &[usize]) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut done = Vec::new();
    for pos in positions {
        let Some(top) = top_list(document, (*pos).min(document.text().len())) else {
            continue;
        };
        if done.contains(&top.span.start) {
            continue;
        }
        done.push(top.span.start);
        renumber_list(document, top, &mut edits);
    }
    mapping::sorted(edits)
}

pub(crate) fn list_edit(
    document: &Document,
    edits: Vec<TextEdit>,
    touched: impl FnOnce(&[TextEdit]) -> Vec<usize>,
    selections: impl FnOnce(&[TextEdit]) -> Vec<Selection>,
) -> Option<Change> {
    let edits = mapping::sorted(edits);
    let middle = mapping::apply(document.text(), &edits);
    if middle == document.text() {
        return None;
    }
    let next = Document::parse(&middle);
    let second = renumber_at(&next, &touched(&edits));
    Some(mapping::rewrite(
        document.text(),
        &edits,
        &second,
        selections(&edits),
    ))
}
