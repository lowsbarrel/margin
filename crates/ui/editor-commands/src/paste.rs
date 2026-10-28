use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, RangeEdit};
use crate::text::{self, Line};
use crate::{editing, html_paste, marks, prefix, tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PasteMode {
    Rich,
    Plain,
}

pub(crate) fn markdown_link(label: &str, url: &str) -> String {
    let balanced = url.matches('(').count() == url.matches(')').count();
    if balanced && !url.contains(['<', '>']) {
        format!("[{label}]({url})")
    } else {
        format!("[{label}](<{url}>)")
    }
}

fn is_web_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        !rest.is_empty() && !rest.contains(|c: char| c.is_whitespace() || c == '<' || c == '>')
    })
}

fn link_selection(document: &Document, selections: &[Selection], url: &str) -> Option<Change> {
    let single_line = |range: &Selection| {
        !range.is_empty() && Line::at(document, range.start()).end >= range.end()
    };
    if !selections.iter().all(single_line) {
        return None;
    }
    let source = document.text();
    mapping::by_range(selections, |range| {
        let insert = markdown_link(range.span().slice(source), url);
        let cursor = range.start() + insert.len();
        Some(RangeEdit::replace(range.span(), insert, cursor))
    })
}

fn bare_marker(before: &str) -> Option<&str> {
    let body = before.strip_suffix(' ')?;
    let indent = text::leading_space(body);
    let rest = body.get(indent.len()..)?;
    let marker = ["[ ]", "[x]", "[X]"]
        .iter()
        .find_map(|state| rest.strip_suffix(state)?.strip_suffix(' '))
        .unwrap_or(rest);
    let bullet = matches!(marker, "-" | "+" | "*");
    let ordered = marker
        .strip_suffix(['.', ')'])
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
    (bullet || ordered).then_some(indent)
}

fn paste_text(document: &Document, range: Selection, raw: &str) -> Change {
    let source = document.text();
    let text = editing::normalize_line_breaks(raw);
    let line = Line::at(document, range.start());
    let bare = bare_marker(text::slice(source, line.start, range.start()));
    let rest_blank =
        range.end() <= line.end && text::slice(source, range.end(), line.end).trim().is_empty();
    let (span, insert) = match bare {
        Some(indent) if rest_blank && prefix::spaced_marker(&text).is_some() => {
            let first_indent = text::leading_space(text.split('\n').next().unwrap_or_default());
            let fitted: Vec<String> = text
                .split('\n')
                .enumerate()
                .map(|(index, entry)| {
                    let own = entry
                        .strip_prefix(first_indent)
                        .unwrap_or_else(|| entry.trim_start());
                    if index == 0 || own.is_empty() {
                        own.to_owned()
                    } else {
                        format!("{indent}{own}")
                    }
                })
                .collect();
            (
                Span::new(line.start + indent.len(), range.end()),
                fitted.join("\n"),
            )
        }
        _ => (range.span(), text),
    };
    let cursor = span.start + insert.len();
    Change::new(vec![TextEdit::new(span, insert)]).with_selections(vec![Selection::cursor(cursor)])
}

fn inside_code(document: &Document, pos: usize) -> bool {
    tree::in_code_block(document, pos) || marks::in_inline_code(document, pos)
}

#[must_use]
pub fn paste(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    plain: &str,
    html: Option<&str>,
    mode: PasteMode,
) -> Option<Change> {
    let main = *selections.get(primary).or_else(|| selections.last())?;
    if inside_code(document, main.head) {
        return Some(editing::insert_text(document, selections, plain));
    }
    let url = plain.trim();
    if mode == PasteMode::Rich
        && is_web_url(url)
        && let Some(change) = link_selection(document, selections, url)
    {
        return Some(change);
    }
    let markdown = match mode {
        PasteMode::Rich => html.and_then(html_paste::html_to_markdown),
        PasteMode::Plain => None,
    };
    Some(paste_text(
        document,
        main,
        markdown.as_deref().unwrap_or(plain),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{primary, run};

    fn pasted(source: &str, plain: &str, html: Option<&str>, mode: PasteMode) -> Option<String> {
        run(source, |d, s| paste(d, s, primary(s), plain, html, mode))
    }

    #[test]
    fn links_a_selection_when_a_url_is_pasted() {
        let rich = pasted("see «docs»", "https://a.b/c ", None, PasteMode::Rich);
        assert_eq!(rich.as_deref(), Some("see [docs](https://a.b/c)|"));
        assert_eq!(
            markdown_link("x", "https://a.b/(c"),
            "[x](<https://a.b/(c>)"
        );
        let plain = pasted("see «docs»", "https://a.b/c", None, PasteMode::Plain);
        assert_eq!(plain.as_deref(), Some("see https://a.b/c|"));
    }

    #[test]
    fn nests_pasted_list_items_under_a_bare_marker() {
        let text = "- one\n  - two\n- three";
        assert_eq!(
            pasted("  - |", text, None, PasteMode::Rich).as_deref(),
            Some("  - one\n    - two\n  - three|")
        );
        assert_eq!(
            pasted("a |", "- x\r\n- y", None, PasteMode::Rich).as_deref(),
            Some("a - x\n- y|")
        );
    }

    #[test]
    fn converts_html_unless_plain_or_in_code() {
        let html = Some("<p>Hello <b>world</b></p>");
        assert_eq!(
            pasted("|", "Hello world", html, PasteMode::Rich).as_deref(),
            Some("Hello **world**|")
        );
        assert_eq!(
            pasted("|", "Hello world", html, PasteMode::Plain).as_deref(),
            Some("Hello world|")
        );
        assert_eq!(
            pasted("`a|`", "x", html, PasteMode::Rich).as_deref(),
            Some("`ax|`")
        );
    }
}
