use margin_markdown::{Block, BlockKind, Span, TableModel};

use crate::block_live::fence_chars;
use crate::block_path::trim_end_spaces;
use crate::line_builder::{LineBuilder, Mark};
use crate::line_view::SyntaxRole;

pub(crate) fn decorate(path: &[&Block], builder: &mut LineBuilder<'_>) {
    let source = builder.source();
    for block in path {
        match &block.kind {
            BlockKind::Heading { .. } => {
                builder.mark(block.span, Mark::Bold);
                markup(builder, &block.markers);
            }
            BlockKind::BlockQuote => {
                builder.mark(block.span, Mark::Quote);
                markup(builder, &block.markers);
            }
            BlockKind::Callout { .. } => {
                builder.mark(block.span, Mark::Quote);
                for marker in &block.markers {
                    if !marker.slice(source).starts_with("[!") {
                        builder.mark(*marker, Mark::Markup);
                    }
                }
            }
            BlockKind::ListItem { .. } => {
                if let Some(marker) = block.markers.first() {
                    builder.mark(trim_end_spaces(source, *marker), Mark::Markup);
                }
                if let Some(task) = block.markers.get(1) {
                    builder.mark(*task, Mark::Tag);
                }
            }
            BlockKind::CodeBlock { fenced, .. } => {
                builder.mark(block.content, Mark::Syntax(SyntaxRole::Attribute));
                if *fenced {
                    for marker in &block.markers {
                        builder.mark(fence_chars(source, *marker), Mark::Markup);
                    }
                }
            }
            BlockKind::ThematicBreak | BlockKind::MathBlock { .. } => {
                markup(builder, &block.markers);
            }
            BlockKind::Table(table) => table_line(table, builder),
            _ => {}
        }
    }
}

fn markup(builder: &mut LineBuilder<'_>, markers: &[Span]) {
    for marker in markers {
        builder.mark(*marker, Mark::Markup);
    }
}

fn table_line(table: &TableModel, builder: &mut LineBuilder<'_>) {
    let line = builder.line();
    if builder.intersects(table.delimiter) {
        builder.mark(table.delimiter, Mark::Markup);
        return;
    }
    if table
        .rows
        .first()
        .is_some_and(|header| builder.intersects(header.span))
    {
        builder.mark_line(Mark::Bold);
    }
    let text = line.slice(builder.source());
    let mut escaped = false;
    for (at, byte) in text.bytes().enumerate() {
        if byte == b'|' && !escaped {
            builder.mark(
                Span::new(line.start + at, line.start + at + 1),
                Mark::Markup,
            );
        }
        escaped = byte == b'\\' && !escaped;
    }
}
