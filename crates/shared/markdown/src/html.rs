use crate::block::{Block, BlockKind};
use crate::document::Document;
use crate::escape::escape_html;
use crate::footnotes::FootnoteIndex;
use crate::html_inline::render_inlines;
use crate::table::{Align, TableModel};
use crate::text::{push_fmt, strip_columns};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HtmlOptions {
    pub raw_html: bool,
}

impl HtmlOptions {
    #[must_use]
    pub const fn new(raw_html: bool) -> Self {
        Self { raw_html }
    }
}

#[must_use]
pub fn to_html(document: &Document, options: HtmlOptions) -> String {
    let mut out = String::with_capacity(document.text().len() + document.text().len() / 2);
    let footnotes = document.footnotes();
    for block in document.blocks() {
        render_block(&mut out, document.text(), options, &footnotes, block, false);
    }
    out
}

fn deindent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let strip = strip_columns(line, 4);
        out.push_str(line.get(strip..).unwrap_or_default());
    }
    out
}

fn render_block(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    block: &Block,
    tight: bool,
) {
    match &block.kind {
        BlockKind::Frontmatter(_) | BlockKind::LinkReference { .. } | BlockKind::Comment => {}
        BlockKind::Heading { level, .. } => {
            push_fmt(out, format_args!("<h{level}>"));
            render_inlines(out, source, options, footnotes, &block.inlines);
            push_fmt(out, format_args!("</h{level}>\n"));
        }
        BlockKind::Paragraph => {
            if !tight {
                out.push_str("<p>");
            }
            render_inlines(out, source, options, footnotes, &block.inlines);
            if !tight {
                out.push_str("</p>");
            }
            out.push('\n');
        }
        BlockKind::BlockQuote => {
            out.push_str("<blockquote>\n");
            render_children(out, source, options, footnotes, block);
            out.push_str("</blockquote>\n");
        }
        BlockKind::Callout { kind, title, .. } => {
            push_fmt(
                out,
                format_args!("<blockquote class=\"callout callout-{}\">\n", kind.name()),
            );
            render_title(out, title);
            render_children(out, source, options, footnotes, block);
            out.push_str("</blockquote>\n");
        }
        BlockKind::ColonCallout { kind, title, .. } => {
            push_fmt(
                out,
                format_args!("<div class=\"callout callout-{}\">\n", kind.name()),
            );
            render_title(out, title);
            render_children(out, source, options, footnotes, block);
            out.push_str("</div>\n");
        }
        BlockKind::BulletList | BlockKind::OrderedList { .. } => {
            render_list(out, source, options, footnotes, block);
        }
        BlockKind::ListItem { .. } => render_item(out, source, options, footnotes, block, tight),
        other => render_leaf(out, source, options, footnotes, block, other),
    }
}

fn render_title(out: &mut String, title: &str) {
    if !title.is_empty() {
        push_fmt(
            out,
            format_args!("<p class=\"callout-title\">{}</p>\n", escape_html(title)),
        );
    }
}

fn render_leaf(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    block: &Block,
    kind: &BlockKind,
) {
    match kind {
        BlockKind::CodeBlock { info, fenced } => {
            let raw = block.content.slice(source);
            let deindented = (!*fenced).then(|| deindent(raw));
            let code = deindented.as_deref().unwrap_or(raw);
            let code = code.strip_suffix('\n').unwrap_or(code);
            let language = info.split_whitespace().next().unwrap_or("");
            if language.eq_ignore_ascii_case("mermaid") {
                push_fmt(
                    out,
                    format_args!(
                        "<div data-type=\"mermaid\" data-mermaid=\"{}\"></div>\n",
                        escape_html(code)
                    ),
                );
            } else if language.is_empty() {
                push_fmt(
                    out,
                    format_args!("<pre><code>{}</code></pre>\n", escape_html(code)),
                );
            } else {
                push_fmt(
                    out,
                    format_args!(
                        "<pre><code class=\"language-{}\">{}</code></pre>\n",
                        escape_html(language),
                        escape_html(code)
                    ),
                );
            }
        }
        BlockKind::ThematicBreak => out.push_str("<hr>\n"),
        BlockKind::MathBlock { .. } => {
            let latex = block.content.slice(source).trim();
            push_fmt(
                out,
                format_args!("<div class=\"math-block\">{}</div>\n", escape_html(latex)),
            );
        }
        BlockKind::Table(table) => render_table(out, source, options, footnotes, table),
        BlockKind::HtmlBlock => {
            let raw = block.span.slice(source);
            if options.raw_html {
                out.push_str(raw);
            } else {
                out.push_str(&escape_html(raw));
            }
            out.push('\n');
        }
        BlockKind::FootnoteDefinition { id } => {
            match footnotes.number(id) {
                Some(number) => {
                    push_fmt(
                        out,
                        format_args!("<div class=\"footnote-def\" id=\"fn-{number}\">\n"),
                    );
                }
                None => out.push_str("<div class=\"footnote-def\">\n"),
            }
            render_children(out, source, options, footnotes, block);
            out.push_str("</div>\n");
        }
        _ => {}
    }
}

fn render_children(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    block: &Block,
) {
    for child in &block.children {
        render_block(out, source, options, footnotes, child, false);
    }
}

fn loose(block: &Block) -> bool {
    block
        .children
        .iter()
        .any(|child| matches!(child.kind, BlockKind::ListItem { loose: true, .. }))
}

fn render_list(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    block: &Block,
) {
    let ordered = matches!(block.kind, BlockKind::OrderedList { .. });
    let tasks = block
        .children
        .iter()
        .any(|child| matches!(child.kind, BlockKind::ListItem { task: Some(_), .. }));
    let start = match block.kind {
        BlockKind::OrderedList { start } => start,
        _ => 1,
    };
    if ordered {
        if start == 1 {
            out.push_str("<ol>\n");
        } else {
            push_fmt(out, format_args!("<ol start=\"{start}\">\n"));
        }
    } else if tasks {
        out.push_str("<ul class=\"task-list\">\n");
    } else {
        out.push_str("<ul>\n");
    }
    let tight = !loose(block);
    for child in &block.children {
        render_block(out, source, options, footnotes, child, tight);
    }
    out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
}

fn render_item(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    block: &Block,
    tight: bool,
) {
    let task = match block.kind {
        BlockKind::ListItem { task, .. } => task,
        _ => None,
    };
    if let Some(checked) = task {
        out.push_str("<li class=\"task-item\">");
        out.push_str("<input class=\"task-checkbox\" type=\"checkbox\" disabled");
        if checked {
            out.push_str(" checked");
        }
        out.push('>');
    } else {
        out.push_str("<li>");
    }
    for child in &block.children {
        render_block(out, source, options, footnotes, child, tight);
    }
    out.push_str("</li>\n");
}

fn render_table(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    table: &TableModel,
) {
    out.push_str("<table>\n<thead>\n");
    render_row(out, source, options, footnotes, table, 0, "th");
    out.push_str("</thead>\n");
    if table.rows.len() > 1 {
        out.push_str("<tbody>\n");
        for index in 1..table.rows.len() {
            render_row(out, source, options, footnotes, table, index, "td");
        }
        out.push_str("</tbody>\n");
    }
    out.push_str("</table>\n");
}

fn render_row(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    table: &TableModel,
    index: usize,
    tag: &str,
) {
    let Some(row) = table.rows.get(index) else {
        return;
    };
    out.push_str("<tr>");
    for (col, cell) in row.cells.iter().enumerate() {
        push_fmt(out, format_args!("<{tag}"));
        if let Some(align) = table.align.get(col).copied().and_then(Align::css) {
            push_fmt(out, format_args!(" style=\"text-align:{align}\""));
        }
        out.push('>');
        render_inlines(out, source, options, footnotes, &cell.inlines);
        push_fmt(out, format_args!("</{tag}>"));
    }
    out.push_str("</tr>\n");
}
