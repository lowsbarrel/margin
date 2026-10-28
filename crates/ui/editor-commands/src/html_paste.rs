use std::iter;
use std::rc::Rc;

use htmd::element_handler::{HandlerResult, Handlers};
use htmd::options::{BulletListMarker, CodeBlockFence, CodeBlockStyle, HeadingStyle, Options};
use htmd::{Element, HtmlToMarkdown};
use markup5ever_rcdom::Handle;

use crate::{html_dom, html_prepare, paste};

fn walk(handlers: &dyn Handlers, node: &Handle) -> String {
    handlers.walk_children(node).content
}

fn flanked(content: &str, marker: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lead = &content[..content.len() - content.trim_start().len()];
    let tail = &content[content.trim_end().len()..];
    format!("{lead}{marker}{trimmed}{marker}{tail}")
}

fn italic(handlers: &dyn Handlers, node: &Handle) -> String {
    flanked(&walk(handlers, node), "_")
}

fn strike(handlers: &dyn Handlers, node: &Handle) -> String {
    let content = walk(handlers, node);
    if content.trim().is_empty() {
        content
    } else {
        format!("~~{content}~~")
    }
}

fn link(handlers: &dyn Handlers, node: &Handle) -> String {
    let content = walk(handlers, node);
    let href = html_dom::attribute(node, "href").unwrap_or_default();
    if href.is_empty() {
        return content;
    }
    if content.trim().is_empty() {
        return String::new();
    }
    paste::markdown_link(&content, &href.replace(' ', "%20"))
}

fn list(handlers: &dyn Handlers, node: &Handle) -> String {
    let content = walk(handlers, node);
    let nested_last = html_dom::parent(node).is_some_and(|parent| {
        html_dom::is(&parent, &["li"])
            && html_dom::element_children(&parent)
                .last()
                .is_some_and(|last| Rc::ptr_eq(last, node))
    });
    if nested_last {
        format!("\n{content}")
    } else {
        format!("\n\n{content}\n\n")
    }
}

fn list_item(handlers: &dyn Handlers, node: &Handle) -> String {
    let content = walk(handlers, node);
    let mut body = content.trim().to_owned();
    for checkbox in ["[ ]", "[x]"] {
        if let Some(rest) = body.strip_prefix(checkbox)
            && rest.starts_with(' ')
        {
            body = format!("{checkbox} {}", rest.trim_start_matches(' '));
        }
    }
    let body = body.replace('\n', "\n    ");
    let prefix = match html_dom::parent(node).filter(|parent| html_dom::is(parent, &["ol"])) {
        Some(parent) => {
            let index = html_dom::element_children(&parent)
                .iter()
                .position(|child| Rc::ptr_eq(child, node))
                .unwrap_or(0);
            let start = html_dom::attribute(&parent, "start")
                .and_then(|start| start.trim().parse::<usize>().ok());
            format!("{}. ", start.map_or(index + 1, |start| start + index))
        }
        None => "- ".to_owned(),
    };
    let tail = if html_dom::has_following_content(node) {
        "\n"
    } else {
        ""
    };
    format!("{prefix}{body}{tail}")
}

fn checkbox(handlers: &dyn Handlers, node: &Handle) -> String {
    let in_item = html_dom::parent(node).is_some_and(|parent| html_dom::is(&parent, &["li"]));
    if html_dom::attribute(node, "type").as_deref() != Some("checkbox") || !in_item {
        return walk(handlers, node);
    }
    let checked = html_dom::attribute(node, "checked").is_some();
    if checked { "[x] " } else { "[ ] " }.to_owned()
}

fn highlighted_code(handlers: &dyn Handlers, element: Element<'_>) -> Option<HandlerResult> {
    let class = html_dom::attribute(element.node, "class").unwrap_or_default();
    let language = class.split_whitespace().find_map(|name| {
        name.strip_prefix("highlight-text-")
            .or_else(|| name.strip_prefix("highlight-source-"))
    });
    let first = html_dom::element_children(element.node).into_iter().next();
    match (
        language,
        first.filter(|first| html_dom::is(first, &["pre"])),
    ) {
        (Some(language), Some(pre)) => {
            let code = html_dom::text_content(&pre);
            Some(format!("\n\n```{language}\n{code}\n```\n\n").into())
        }
        _ => handlers.fallback(element),
    }
}

fn rows_of(table: &Handle) -> Vec<Handle> {
    let mut rows = Vec::new();
    for child in html_dom::element_children(table) {
        if html_dom::is(&child, &["tr"]) {
            rows.push(child);
        } else if html_dom::is(&child, &["thead", "tbody", "tfoot"]) {
            rows.extend(
                html_dom::element_children(&child)
                    .into_iter()
                    .filter(|row| html_dom::is(row, &["tr"])),
            );
        }
    }
    rows
}

fn cells_of(row: &Handle) -> Vec<Handle> {
    html_dom::element_children(row)
        .into_iter()
        .filter(|cell| html_dom::is(cell, &["td", "th"]))
        .collect()
}

fn escape_pipes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous = None;
    for character in text.chars() {
        if character == '|' && previous != Some('\\') {
            out.push('\\');
        }
        out.push(character);
        previous = Some(character);
    }
    out
}

fn cell_text(handlers: &dyn Handlers, cell: &Handle) -> String {
    let content = walk(handlers, cell);
    let mut joined = String::new();
    for (index, part) in content
        .trim()
        .lines()
        .filter(|part| !part.is_empty())
        .enumerate()
    {
        if index > 0 {
            joined.push_str("<br>");
        }
        joined.push_str(part);
    }
    escape_pipes(&joined)
}

fn rule(cell: Option<&Handle>) -> &'static str {
    let align = cell.and_then(|cell| {
        html_dom::attribute(cell, "align").or_else(|| {
            let style = html_dom::style(cell).to_lowercase();
            style.split(';').find_map(|declaration| {
                let (name, value) = declaration.split_once(':')?;
                (name.trim() == "text-align").then(|| value.trim().to_owned())
            })
        })
    });
    match align.map(|align| align.to_lowercase()).as_deref() {
        Some("left") => ":--",
        Some("center") => ":-:",
        Some("right") => "--:",
        _ => "---",
    }
}

fn table(handlers: &dyn Handlers, node: &Handle) -> Option<String> {
    let rows = rows_of(node);
    let mut grid: Vec<Vec<String>> = Vec::new();
    for row in &rows {
        let mut cells = Vec::new();
        for cell in cells_of(row) {
            cells.push(cell_text(handlers, &cell));
            let span = html_dom::attribute(&cell, "colspan")
                .and_then(|span| span.trim().parse::<usize>().ok());
            cells.extend(iter::repeat_n(
                String::new(),
                span.unwrap_or(1).saturating_sub(1),
            ));
        }
        if cells.iter().any(|cell| !cell.is_empty()) {
            grid.push(cells);
        }
    }
    let columns = grid.iter().map(Vec::len).max()?;
    if let [only] = grid.as_slice()
        && let [cell] = only.as_slice()
    {
        return Some(format!("\n\n{cell}\n\n"));
    }
    for cells in &mut grid {
        cells.resize(columns, String::new());
    }
    let head = rows.first().map(cells_of).unwrap_or_default();
    let rules: Vec<String> = (0..columns)
        .map(|column| rule(head.get(column)).to_owned())
        .collect();
    let mut lines = Vec::with_capacity(grid.len() + 1);
    for (index, cells) in grid.iter().enumerate() {
        lines.push(format!("|{}|", cells.join("|")));
        if index == 0 {
            lines.push(format!("|{}|", rules.join("|")));
        }
    }
    Some(format!("\n\n{}\n\n", lines.join("\n")))
}

fn converter() -> HtmlToMarkdown {
    let options = Options {
        heading_style: HeadingStyle::Atx,
        code_block_style: CodeBlockStyle::Fenced,
        code_block_fence: CodeBlockFence::Backticks,
        bullet_list_marker: BulletListMarker::Dash,
        ul_bullet_spacing: 1,
        ol_number_spacing: 1,
        ..Options::default()
    };
    let text = |convert: fn(&dyn Handlers, &Handle) -> String| {
        move |handlers: &dyn Handlers, element: Element<'_>| -> Option<HandlerResult> {
            Some(convert(handlers, element.node).into())
        }
    };
    HtmlToMarkdown::builder()
        .options(options)
        .add_handler(vec!["i", "em"], text(italic))
        .add_handler(vec!["del", "s", "strike"], text(strike))
        .add_handler(vec!["a"], text(link))
        .add_handler(vec!["ul", "ol"], text(list))
        .add_handler(vec!["li"], text(list_item))
        .add_handler(vec!["input"], text(checkbox))
        .add_handler(vec!["div"], highlighted_code)
        .add_handler(
            vec!["table"],
            |handlers: &dyn Handlers, element: Element<'_>| {
                table(handlers, element.node).map(HandlerResult::from)
            },
        )
        .add_handler(vec!["br"], text(|_, _| "\n".to_owned()))
        .add_handler(vec!["hr"], text(|_, _| "\n\n---\n\n".to_owned()))
        .build()
}

fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0;
    for character in text.chars() {
        if character == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        out.push(if character == '\u{a0}' {
            ' '
        } else {
            character
        });
    }
    out
}

#[must_use]
pub fn html_to_markdown(html: &str) -> Option<String> {
    let converter = converter();
    let document = converter.html_to_tree(html).ok()?;
    if html_prepare::is_code_editor_html(html, &document) {
        return None;
    }
    html_prepare::prepare(&document);
    Some(collapse_blank_lines(&converter.tree_to_markdown(&document)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markdown(html: &str) -> String {
        html_to_markdown(html).expect("ordinary HTML converts")
    }

    #[test]
    fn converts_headings_and_inline_marks() {
        assert_eq!(
            markdown("<h2>Title</h2><p>A <strong>bold</strong> and <em>soft</em> word</p>"),
            "## Title\n\nA **bold** and _soft_ word"
        );
        assert_eq!(
            markdown("<p><del>gone</del> <code>x()</code></p>"),
            "~~gone~~ `x()`"
        );
        assert_eq!(
            markdown("<p><span style=\"font-weight:700\">loud</span></p>"),
            "**loud**"
        );
    }

    #[test]
    fn converts_links() {
        assert_eq!(
            markdown("<a href=\"https://a.b/c d\">site</a>"),
            "[site](https://a.b/c%20d)"
        );
        assert_eq!(markdown("<a href=\"#top\">top</a>"), "top");
        assert_eq!(markdown("<a href=\"https://a.b\"> </a>x"), "x");
    }

    #[test]
    fn converts_lists_and_tasks() {
        assert_eq!(
            markdown("<ul><li>one</li><li>two<ul><li>deep</li></ul></li></ul>"),
            "- one\n- two\n    - deep"
        );
        assert_eq!(
            markdown("<ol start=\"3\"><li>c</li><li>d</li></ol>"),
            "3. c\n4. d"
        );
        assert_eq!(
            markdown("<ul><li><input type=\"checkbox\" checked> done</li></ul>"),
            "- [x] done"
        );
    }

    #[test]
    fn converts_word_lists() {
        let html = "<p style=\"mso-list:l0 level1 lfo1\"><span>\u{b7}</span>first</p>\
                    <p style=\"mso-list:l0 level1 lfo1\"><span>\u{b7}</span>second</p>";
        assert_eq!(markdown(html), "- first\n- second");
    }

    #[test]
    fn converts_code_blocks() {
        assert_eq!(
            markdown("<pre><code class=\"language-rust\">fn a() {}\n</code></pre>"),
            "```rust\nfn a() {}\n```"
        );
    }

    #[test]
    fn converts_tables() {
        let html = "<table><thead><tr><th align=\"left\">A</th><th style=\"text-align: right\">B</th></tr></thead>\
                    <tbody><tr><td>1 | 2</td><td colspan=\"1\">x<br>y</td></tr></tbody></table>";
        assert_eq!(markdown(html), "|A|B|\n|:--|--:|\n|1 \\| 2|x<br>y|");
        assert_eq!(markdown("<table><tr><td>solo</td></tr></table>"), "solo");
    }

    #[test]
    fn leaves_code_editor_html_to_plain_text() {
        assert_eq!(
            html_to_markdown("<div class=\"cm-editor\"><span>let a</span></div>"),
            None
        );
        assert_eq!(
            html_to_markdown("<div style=\"white-space: pre\">x</div>"),
            None
        );
        assert_eq!(
            html_to_markdown("<span style=\"font-family: Menlo\">code</span>"),
            None
        );
    }
}
