use std::ptr;

use margin_markdown::{Block, BlockKind, Document, Span, TextEdit};

use crate::text::{self, Line};
use crate::{list_tree, tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextKind {
    Quote,
    Bullet,
    Ordered,
    Fence,
}

#[derive(Clone, Debug)]
pub(crate) struct Context<'document> {
    pub kind: ContextKind,
    pub from: usize,
    pub to: usize,
    pub space_before: String,
    pub space_after: String,
    pub marker: String,
    pub list: Option<&'document Block>,
    pub item: Option<&'document Block>,
}

impl Context<'_> {
    pub(crate) fn blank(&self, max_width: Option<usize>, trailing: bool) -> String {
        let mut result = self.space_before.clone();
        if self.kind == ContextKind::Quote {
            result.push('>');
        }
        if let Some(width) = max_width {
            while result.len() < width {
                result.push(' ');
            }
            return result;
        }
        let pad = (self.to - self.from).saturating_sub(result.len() + self.space_after.len());
        result.push_str(&" ".repeat(pad));
        if trailing {
            result.push_str(&self.space_after);
        }
        result
    }

    pub(crate) fn next_marker(&self, document: &Document) -> String {
        let number = match (self.kind, self.item) {
            (ContextKind::Ordered, Some(item)) => item_number(document, item)
                .map_or_else(String::new, |(_, value)| (value + 1).to_string()),
            _ => String::new(),
        };
        format!(
            "{}{number}{}{}",
            self.space_before, self.marker, self.space_after
        )
    }
}

pub(crate) fn node_start(document: &Document, block: &Block) -> usize {
    let rest = document.text().get(block.span.start..).unwrap_or_default();
    block.span.start + text::leading_space(rest).len()
}

pub(crate) fn item_number(document: &Document, item: &Block) -> Option<(Span, u64)> {
    let from = node_start(document, item);
    let rest = text::slice(document.text(), from, from + 10);
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let value = rest.get(..digits)?.parse().ok()?;
    rest.get(digits..)?
        .starts_with(['.', ')'])
        .then_some((Span::new(from, from + digits), value))
}

fn run_of(text: &str, byte: u8) -> usize {
    text.bytes()
        .take_while(|candidate| *candidate == byte)
        .count()
}

fn quote_context<'document>(rest: &str, start: usize) -> Option<Context<'document>> {
    let spaces = run_of(rest, b' ');
    let after = rest.get(spaces..)?.strip_prefix('>')?;
    let space_after = if after.starts_with(' ') { " " } else { "" };
    Some(Context {
        kind: ContextKind::Quote,
        from: start,
        to: start + spaces + 1 + space_after.len(),
        space_before: String::new(),
        space_after: space_after.to_owned(),
        marker: ">".to_owned(),
        list: None,
        item: None,
    })
}

fn ordered_context(rest: &str) -> Option<(usize, String, String, String)> {
    let before = run_of(rest, b' ');
    let tail = rest.get(before..)?;
    let digits = tail.len() - tail.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let delimiter = tail
        .get(digits..)?
        .chars()
        .next()
        .filter(|c| matches!(c, '.' | ')'))?;
    let mut after = run_of(tail.get(digits + 1..)?, b' ');
    let mut length = before + digits + 1 + after;
    if after >= 4 {
        after -= 4;
        length -= 4;
    }
    Some((
        length,
        " ".repeat(before),
        " ".repeat(after),
        delimiter.to_string(),
    ))
}

fn bullet_context(rest: &str) -> Option<(usize, String, String, String)> {
    let before = run_of(rest, b' ');
    let tail = rest.get(before..)?;
    let bullet = tail
        .chars()
        .next()
        .filter(|c| matches!(c, '-' | '+' | '*'))?;
    let after_bullet = tail.get(1..)?;
    let gap = run_of(after_bullet, b' ');
    let mut task = String::new();
    let mut at = 1;
    if (1..=4).contains(&gap)
        && let Some(state) = after_bullet
            .get(gap..)
            .and_then(|box_| box_.strip_prefix('['))
        && state.starts_with([' ', 'x', 'X'])
        && state.get(1..2) == Some("]")
    {
        task = format!("{} [ ]", " ".repeat(gap - 1));
        at += gap + 3;
    }
    let mut after = run_of(tail.get(at..)?, b' ');
    if after == 0 {
        return None;
    }
    let mut length = before + at + after;
    if after > 4 {
        after -= 4;
        length -= 4;
    }
    Some((
        length,
        " ".repeat(before),
        " ".repeat(after),
        format!("{bullet}{task}"),
    ))
}

fn list_context<'document>(
    list: &'document Block,
    item: &'document Block,
    rest: &str,
    start: usize,
) -> Option<Context<'document>> {
    let ordered = matches!(list.kind, BlockKind::OrderedList { .. });
    let (length, space_before, space_after, marker) = if ordered {
        ordered_context(rest)?
    } else {
        bullet_context(rest)?
    };
    Some(Context {
        kind: if ordered {
            ContextKind::Ordered
        } else {
            ContextKind::Bullet
        },
        from: start,
        to: start + length,
        space_before,
        space_after,
        marker,
        list: Some(list),
        item: Some(item),
    })
}

pub(crate) fn contexts(document: &Document, pos: usize) -> Vec<Context<'_>> {
    let chain = tree::ancestors(document, pos);
    let mut out = Vec::new();
    for (depth, node) in chain.iter().enumerate() {
        let from = node_start(document, node);
        let line = Line::at(document, from);
        let start = from - line.start;
        let rest = line.text.get(start..).unwrap_or_default();
        let context = match &node.kind {
            BlockKind::CodeBlock { fenced: true, .. } => Some(Context {
                kind: ContextKind::Fence,
                from: start,
                to: start,
                space_before: String::new(),
                space_after: String::new(),
                marker: String::new(),
                list: None,
                item: None,
            }),
            kind if tree::is_quote(kind) => quote_context(rest, start),
            BlockKind::ListItem { .. } => depth
                .checked_sub(1)
                .and_then(|parent| chain.get(parent))
                .filter(|list| tree::is_list(&list.kind))
                .and_then(|list| list_context(list, node, rest, start)),
            _ => None,
        };
        out.extend(context);
    }
    out
}

pub(crate) fn renumber_after(
    document: &Document,
    list: &Block,
    after: &Block,
    offset: i64,
    edits: &mut Vec<TextEdit>,
) {
    let mut previous: Option<u64> = None;
    let start = list
        .children
        .iter()
        .position(|item| ptr::eq(item, after))
        .unwrap_or(list.children.len());
    for item in list.children.iter().skip(start) {
        let Some((span, number)) = item_number(document, item) else {
            continue;
        };
        if let Some(previous) = previous {
            if number != previous + 1 {
                return;
            }
            let value = i64::try_from(previous).unwrap_or(0) + 2 + offset;
            edits.push(TextEdit::new(span, value.to_string()));
        }
        previous = Some(number);
    }
}

pub(crate) fn non_tight_list(document: &Document, list: &Block) -> bool {
    if !tree::is_list(&list.kind) {
        return false;
    }
    let (Some(first), Some(second)) = (list.children.first(), list.children.get(1)) else {
        return false;
    };
    let line1 = Line::at(document, first.span.end);
    let line2 = Line::at(document, node_start(document, second));
    let empty = line1.text.chars().all(|c| c.is_whitespace() || c == '>');
    line1.index + usize::from(!empty) < line2.index
}

pub(crate) fn normalize_indent(document: &Document, content: &str) -> String {
    let blank = text::leading_space(content).len();
    if blank == 0 || list_tree::detect_indent_unit(document.text()) != "\t" {
        return content.to_owned();
    }
    let column = text::count_column(content, blank);
    format!(
        "{}{}{}",
        "\t".repeat(column / text::TAB_SIZE),
        " ".repeat(column % text::TAB_SIZE),
        content.get(blank..).unwrap_or_default()
    )
}
