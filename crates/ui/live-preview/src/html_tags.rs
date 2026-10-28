use margin_markdown::{Inline, InlineKind, Span};

use crate::line_builder::Mark;
use crate::line_view::Script;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HtmlRole {
    Break,
    Open { close: Span, mark: Mark },
    Close { open: Span },
    Other,
}

fn format_mark(name: &str) -> Option<Mark> {
    Some(match name {
        "kbd" => Mark::Keyboard,
        "u" | "ins" => Mark::Underline,
        "sub" => Mark::Script(Script::Subscript),
        "sup" => Mark::Script(Script::Superscript),
        "mark" => Mark::Highlight,
        "b" | "strong" => Mark::Bold,
        "i" | "em" => Mark::Italic,
        "s" | "del" => Mark::Strike,
        "small" => Mark::Small,
        _ => return None,
    })
}

fn is_break(text: &str) -> bool {
    let Some(head) = text.get(..3) else {
        return false;
    };
    if !head.eq_ignore_ascii_case("<br") {
        return false;
    }
    let rest = text.get(3..).unwrap_or_default().trim_start();
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    rest.starts_with('>')
}

fn bare_tag(text: &str) -> Option<(String, bool)> {
    let inner = text.strip_prefix('<')?.strip_suffix('>')?;
    let (closing, name) = match inner.strip_prefix('/') {
        Some(name) => (true, name),
        None => (false, inner),
    };
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    Some((name.to_ascii_lowercase(), closing))
}

pub(crate) fn roles(source: &str, inlines: &[Inline]) -> Vec<HtmlRole> {
    let mut roles = vec![HtmlRole::Other; inlines.len()];
    if !inlines
        .iter()
        .any(|inline| matches!(inline.kind, InlineKind::Html { .. }))
    {
        return roles;
    }
    let mut open: Vec<(usize, String, Mark)> = Vec::new();
    for (index, inline) in inlines.iter().enumerate() {
        if !matches!(inline.kind, InlineKind::Html { .. }) {
            continue;
        }
        let text = inline.span.slice(source);
        if is_break(text) {
            if let Some(role) = roles.get_mut(index) {
                *role = HtmlRole::Break;
            }
            continue;
        }
        let Some((name, closing)) = bare_tag(text) else {
            continue;
        };
        if !closing {
            if let Some(mark) = format_mark(&name) {
                open.push((index, name, mark));
            }
            continue;
        }
        let Some(at) = open.iter().rposition(|(_, opened, _)| *opened == name) else {
            continue;
        };
        let Some((opener, _, mark)) = open.get(at).cloned() else {
            continue;
        };
        open.truncate(at);
        let Some(open_span) = inlines.get(opener).map(|inline| inline.span) else {
            continue;
        };
        if let Some(role) = roles.get_mut(opener) {
            *role = HtmlRole::Open {
                close: inline.span,
                mark,
            };
        }
        if let Some(role) = roles.get_mut(index) {
            *role = HtmlRole::Close { open: open_span };
        }
    }
    roles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_breaks_and_bare_tags_only() {
        assert!(is_break("<br>"));
        assert!(is_break("<BR />"));
        assert!(!is_break("<bra>"));
        assert_eq!(bare_tag("<Kbd>"), Some(("kbd".to_owned(), false)));
        assert_eq!(bare_tag("</u>"), Some(("u".to_owned(), true)));
        assert_eq!(bare_tag("<u class=\"x\">"), None);
    }
}
