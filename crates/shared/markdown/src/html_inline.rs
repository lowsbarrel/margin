use crate::escape::escape_html;
use crate::footnotes::FootnoteIndex;
use crate::html::HtmlOptions;
use crate::inline::{Inline, InlineKind};
use crate::text::push_fmt;

pub(crate) fn render_inlines(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    inlines: &[Inline],
) {
    for inline in inlines {
        render_inline(out, source, options, footnotes, inline);
    }
}

fn render_inline(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    inline: &Inline,
) {
    match &inline.kind {
        InlineKind::Text | InlineKind::Tag { .. } => {
            out.push_str(&escape_html(inline.span.slice(source)));
        }
        InlineKind::Emphasis => wrap(out, source, options, footnotes, inline, "em"),
        InlineKind::Strong => wrap(out, source, options, footnotes, inline, "strong"),
        InlineKind::Strikethrough => wrap(out, source, options, footnotes, inline, "s"),
        InlineKind::Highlight => wrap(out, source, options, footnotes, inline, "mark"),
        InlineKind::Code => {
            let code = inline.content.slice(source).replace('\n', " ");
            push_fmt(out, format_args!("<code>{}</code>", escape_html(&code)));
        }
        InlineKind::Link(target) => {
            push_fmt(
                out,
                format_args!("<a href=\"{}\"", escape_html(&target.destination)),
            );
            if let Some(title) = &target.title {
                push_fmt(out, format_args!(" title=\"{}\"", escape_html(title)));
            }
            out.push('>');
            render_inlines(out, source, options, footnotes, &inline.children);
            out.push_str("</a>");
        }
        InlineKind::Autolink { destination, .. } => push_fmt(
            out,
            format_args!(
                "<a href=\"{}\">{}</a>",
                escape_html(destination),
                escape_html(inline.content.slice(source))
            ),
        ),
        InlineKind::Image(target) => {
            let mut alt = String::new();
            for child in &inline.children {
                child.collect_text(source, &mut alt);
            }
            push_fmt(
                out,
                format_args!(
                    "<img src=\"{}\" alt=\"{}\"",
                    escape_html(&target.destination),
                    escape_html(&alt)
                ),
            );
            if let Some(title) = &target.title {
                push_fmt(out, format_args!(" title=\"{}\"", escape_html(title)));
            }
            out.push('>');
        }
        InlineKind::WikiLink(link) => push_fmt(
            out,
            format_args!(
                "<a class=\"wiki-link\" href=\"#\">{}</a>",
                escape_html(link.label())
            ),
        ),
        InlineKind::Embed(embed) => push_fmt(
            out,
            format_args!(
                "<img class=\"wiki-embed\" src=\"{}\" alt=\"{}\">",
                escape_html(&embed.target),
                escape_html(&embed.target)
            ),
        ),
        InlineKind::Math => push_fmt(
            out,
            format_args!(
                "<span class=\"math-inline\">{}</span>",
                escape_html(inline.content.slice(source))
            ),
        ),
        InlineKind::Comment => {}
        InlineKind::FootnoteReference { id } => match footnotes.number(id) {
            Some(number) => push_fmt(
                out,
                format_args!(
                    "<sup class=\"footnote-ref\"><a href=\"#fn-{number}\" id=\"fnref-{number}\">{number}</a></sup>"
                ),
            ),
            None => out.push_str(&escape_html(inline.span.slice(source))),
        },
        InlineKind::Html { .. } => {
            let raw = inline.span.slice(source);
            if options.raw_html {
                out.push_str(raw);
            } else {
                out.push_str(&escape_html(raw));
            }
        }
        InlineKind::SoftBreak => out.push('\n'),
        InlineKind::HardBreak => out.push_str("<br>\n"),
        InlineKind::Escape => out.push_str(&escape_html(inline.content.slice(source))),
    }
}

fn wrap(
    out: &mut String,
    source: &str,
    options: HtmlOptions,
    footnotes: &FootnoteIndex,
    inline: &Inline,
    tag: &str,
) {
    push_fmt(out, format_args!("<{tag}>"));
    render_inlines(out, source, options, footnotes, &inline.children);
    push_fmt(out, format_args!("</{tag}>"));
}
