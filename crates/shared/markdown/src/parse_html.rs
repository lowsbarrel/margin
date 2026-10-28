use crate::block::{Block, BlockKind};
use crate::span::Span;
use crate::text::{Line, indent_of, is_blank, leading_spaces};

const HTML_BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "pre",
    "script",
    "search",
    "section",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

pub(crate) fn indented_code(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    if indent_of(line.text(source)) < 4 {
        return None;
    }
    let mut at = index;
    let mut last_end = line.end;
    let mut pending_blank = 0;
    while let Some(next) = lines.get(at) {
        let text = next.text(source);
        if is_blank(text) {
            pending_blank += 1;
            at += 1;
            continue;
        }
        if indent_of(text) < 4 {
            break;
        }
        pending_blank = 0;
        last_end = next.end;
        at += 1;
    }
    let at = at.saturating_sub(pending_blank);
    let span = Span::new(line.start, last_end);
    let block = Block::new(
        BlockKind::CodeBlock {
            info: String::new(),
            fenced: false,
        },
        span,
    )
    .with_content(span);
    Some((block, at))
}

pub(crate) fn html_block_name(text: &str) -> Option<String> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let rest = rest.strip_prefix('<')?;
    if rest.starts_with("!--") || rest.starts_with('?') || rest.starts_with('!') {
        return Some(String::new());
    }
    let body = rest.strip_prefix('/').unwrap_or(rest);
    let mut name = String::new();
    for ch in body.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
        } else {
            break;
        }
    }
    HTML_BLOCK_TAGS.contains(&name.as_str()).then_some(name)
}

#[must_use]
pub(crate) fn is_html_block(text: &str) -> bool {
    html_block_name(text).is_some()
}

pub(crate) fn html_block(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let name = html_block_name(line.text(source))?;
    let closing = if name.is_empty() {
        None
    } else {
        Some(format!("</{name}>"))
    };
    let mut at = index;
    let mut last_end = line.end;
    while let Some(next) = lines.get(at) {
        let text = next.text(source);
        if at > index && is_blank(text) {
            break;
        }
        last_end = next.end;
        at += 1;
        if closing.as_deref().is_some_and(|tag| text.contains(tag)) {
            break;
        }
    }
    let span = Span::new(line.start, last_end);
    let block = Block::new(BlockKind::HtmlBlock, span)
        .with_content(span)
        .with_markers(vec![span]);
    Some((block, at))
}
