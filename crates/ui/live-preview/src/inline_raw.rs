use margin_markdown::{Inline, InlineKind};

use crate::inline_live::{link_tail, wiki_parts};
use crate::line_builder::{LineBuilder, Mark};
use crate::line_view::SyntaxRole;

pub(crate) fn visit(builder: &mut LineBuilder<'_>, inlines: &[Inline]) {
    for inline in inlines {
        if builder.intersects(inline.span) {
            one(builder, inline);
        }
    }
}

fn markers(builder: &mut LineBuilder<'_>, inline: &Inline) {
    for marker in &inline.markers {
        builder.mark(*marker, Mark::Markup);
    }
}

fn one(builder: &mut LineBuilder<'_>, inline: &Inline) {
    let span = inline.span;
    let mark = match &inline.kind {
        InlineKind::Emphasis => Mark::Italic,
        InlineKind::Strong => Mark::Bold,
        InlineKind::Strikethrough => Mark::Strike,
        InlineKind::Highlight | InlineKind::Math => Mark::Highlight,
        InlineKind::Code => Mark::Syntax(SyntaxRole::Attribute),
        InlineKind::Link(_) | InlineKind::Image(_) | InlineKind::FootnoteReference { .. } => {
            builder.mark(span, Mark::Link);
            markers(builder, inline);
            if let Some(tail) = link_tail(inline) {
                builder.mark(tail, Mark::Markup);
            }
            visit(builder, &inline.children);
            return;
        }
        InlineKind::Autolink { .. } => Mark::Markup,
        InlineKind::Tag { .. } => Mark::Tag,
        InlineKind::WikiLink(_) | InlineKind::Embed(_) => {
            if let Some(parts) = wiki_parts(builder.source(), inline) {
                builder.mark(parts.open, Mark::Markup);
                builder.mark(parts.close, Mark::Markup);
                if let Some(pipe) = parts.pipe {
                    builder.mark(pipe, Mark::Markup);
                }
            }
            return;
        }
        InlineKind::Text
        | InlineKind::Comment
        | InlineKind::Html { .. }
        | InlineKind::SoftBreak
        | InlineKind::HardBreak
        | InlineKind::Escape => return,
    };
    builder.mark(span, mark);
    markers(builder, inline);
    visit(builder, &inline.children);
}
