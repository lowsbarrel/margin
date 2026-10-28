use margin_markdown::{Block, BlockKind, CalloutKind, Document, Fold, Span};

use crate::block_path::{
    block_touched, first_line, last_line, leading_space, lines_span, trim_end_spaces,
};
use crate::inline_live::Live;
use crate::line_builder::{LineBuilder, Mark};
use crate::line_view::{
    AtomKind, CalloutHeader, CodeLine, CodeLinePosition, Container, ContainerKind, LineDecor,
    ListDecor, ListMarker,
};
use crate::yaml_syntax;

#[derive(Debug)]
pub(crate) struct LineCx<'a> {
    pub document: &'a Document,
    pub line: usize,
    pub touched: bool,
    pub live: Live<'a>,
}

pub(crate) fn decorate(
    cx: &mut LineCx<'_>,
    path: &[&Block],
    builder: &mut LineBuilder<'_>,
) -> LineDecor {
    let mut decor = LineDecor::default();
    let mut depth = 0;
    let mut parent: Option<&Block> = None;
    for (index, block) in path.iter().enumerate() {
        match &block.kind {
            BlockKind::BlockQuote => {
                container(cx, block, ContainerKind::Quote, &mut decor);
                quote_markers(cx, &block.markers, builder);
            }
            BlockKind::Callout { kind, fold, title } => {
                let header = Header {
                    kind: *kind,
                    fold: *fold,
                    title,
                };
                callout(cx, block, &header, builder, &mut decor);
            }
            BlockKind::ColonCallout { kind, title, .. } => {
                let header = Header {
                    kind: *kind,
                    fold: Fold::None,
                    title,
                };
                colon_callout(cx, block, &header, builder, &mut decor);
            }
            BlockKind::ListItem { .. } => {
                let ordered =
                    parent.is_some_and(|list| matches!(list.kind, BlockKind::OrderedList { .. }));
                let child = path.get(index + 1).copied();
                decor.list = list_item(cx, block, ordered, depth, child, builder);
                depth += 1;
            }
            BlockKind::Heading { level, .. } => {
                decor.heading = Some(*level);
                builder.mark_line(Mark::Heading(*level));
                for marker in &block.markers {
                    reveal_line_markup(cx, builder, *marker);
                }
            }
            BlockKind::CodeBlock { fenced, .. } => {
                decor.code = Some(code_line(cx, block, *fenced, builder));
            }
            BlockKind::ThematicBreak => {
                if !cx.live.reveal.touches(block.span) {
                    builder.hide(block.span);
                    decor.rule = true;
                }
            }
            BlockKind::MathBlock { .. } => builder.mark(block.span, Mark::Markup),
            BlockKind::Comment => {
                decor.comment = true;
                builder.mark(block.span, Mark::Markup);
            }
            BlockKind::FootnoteDefinition { id } => {
                if cx.line == first_line(cx.document, block.span) {
                    decor.footnote_definition = Some(id.clone());
                    footnote_definition(cx, block, id, builder);
                }
            }
            BlockKind::HtmlBlock => builder.mark(block.span, Mark::Code),
            BlockKind::Frontmatter(_) => {
                let fence = cx.line == first_line(cx.document, block.span)
                    || cx.line == last_line(cx.document, block.span);
                yaml_syntax::highlight(builder, fence);
            }
            BlockKind::Paragraph
            | BlockKind::BulletList
            | BlockKind::OrderedList { .. }
            | BlockKind::Table(_)
            | BlockKind::LinkReference { .. } => {}
        }
        parent = Some(block);
    }
    decor
}

struct Header<'a> {
    kind: CalloutKind,
    fold: Fold,
    title: &'a str,
}

fn reveal_line_markup(cx: &LineCx<'_>, builder: &mut LineBuilder<'_>, span: Span) {
    if cx.touched {
        builder.mark(span, Mark::Markup);
    } else {
        builder.hide(span);
    }
}

fn quote_markers(cx: &LineCx<'_>, markers: &[Span], builder: &mut LineBuilder<'_>) {
    let line = builder.line();
    let from = markers.partition_point(|marker| marker.end < line.start);
    for marker in markers.get(from..).unwrap_or_default() {
        if marker.start > line.end {
            break;
        }
        reveal_line_markup(cx, builder, *marker);
    }
}

fn container(cx: &LineCx<'_>, block: &Block, kind: ContainerKind, decor: &mut LineDecor) {
    decor.containers.push(Container {
        kind,
        first: cx.line == first_line(cx.document, block.span),
        last: cx.line == last_line(cx.document, block.span),
    });
}

fn callout(
    cx: &LineCx<'_>,
    block: &Block,
    header: &Header<'_>,
    builder: &mut LineBuilder<'_>,
    decor: &mut LineDecor,
) {
    container(cx, block, ContainerKind::Callout(header.kind), decor);
    let source = builder.source();
    let marker = block
        .markers
        .last()
        .copied()
        .filter(|marker| marker.slice(source).starts_with("[!"));
    let quotes = match marker {
        Some(_) => block.markers.split_last().map_or(&[][..], |(_, rest)| rest),
        None => &block.markers,
    };
    quote_markers(cx, quotes, builder);
    let Some(marker) = marker else {
        return;
    };
    if cx.line != first_line(cx.document, block.span) {
        return;
    }
    let rest = Span::new(marker.start, builder.line().end);
    if block_touched(cx.document, cx.live.reveal, block.span) {
        builder.mark(rest, Mark::Markup);
        return;
    }
    builder.hide(rest);
    decor.callout_header = Some(callout_header(block, header));
}

fn callout_header(block: &Block, header: &Header<'_>) -> CalloutHeader {
    CalloutHeader {
        kind: header.kind,
        fold: header.fold,
        block: block.span,
        title: header.title.to_owned(),
    }
}

fn colon_callout(
    cx: &LineCx<'_>,
    block: &Block,
    header: &Header<'_>,
    builder: &mut LineBuilder<'_>,
    decor: &mut LineDecor,
) {
    let (Some(open), Some(close_marker)) = (block.markers.first(), block.markers.get(1)) else {
        return;
    };
    let first = first_line(cx.document, block.span);
    let close = last_line(cx.document, block.span);
    let inside = cx
        .live
        .reveal
        .touches(lines_span(cx.document, first, close));
    let last = if inside {
        close
    } else {
        close.saturating_sub(1).max(first)
    };
    decor.containers.push(Container {
        kind: ContainerKind::Callout(header.kind),
        first: cx.line == first,
        last: cx.line == last,
    });
    let line = builder.line();
    if cx.line == first {
        let rest = Span::new(open.start, line.end);
        if inside {
            builder.mark(rest, Mark::Markup);
        } else {
            builder.hide(rest);
            decor.callout_header = Some(callout_header(block, header));
        }
    } else if cx.line == close {
        if inside {
            builder.mark(*close_marker, Mark::Markup);
        } else {
            builder.hide(line);
        }
    }
}

fn list_item(
    cx: &LineCx<'_>,
    item: &Block,
    ordered: bool,
    depth: usize,
    child: Option<&Block>,
    builder: &mut LineBuilder<'_>,
) -> Option<ListDecor> {
    let marker = *item.markers.first()?;
    let source = builder.source();
    let line = builder.line();
    if cx.line != first_line(cx.document, item.span) {
        if !child.is_some_and(|child| matches!(child.kind, BlockKind::Paragraph)) {
            return None;
        }
        builder.hide(leading_space(source, line));
        return Some(ListDecor {
            depth,
            marker: None,
            marker_source: marker,
        });
    }
    let before = Span::new(line.start, marker.start).slice(source);
    let indent = before.len() - before.trim_end_matches([' ', '\t']).len();
    builder.hide(Span::new(marker.start - indent, marker.start));
    let mark = trim_end_spaces(source, marker);
    let near = cx.live.reveal.touches(mark);
    if near {
        builder.mark(marker, Mark::Markup);
    } else {
        builder.hide(marker);
    }
    let task = item.task_state().zip(item.markers.get(1).copied());
    let shown = if near {
        None
    } else if ordered {
        Some(ListMarker::Ordered(mark.slice(source).to_owned()))
    } else if let Some((checked, _)) = task {
        Some(ListMarker::Task(checked))
    } else {
        Some(ListMarker::Bullet)
    };
    if let Some((checked, task_box)) = task {
        let spaced = Span::new(task_box.end, task_box.end + 1);
        let end = if spaced.end <= line.end && matches!(spaced.slice(source), " " | "\t") {
            spaced.end
        } else {
            task_box.end
        };
        if checked && end < line.end {
            builder.mark(Span::new(end, line.end), Mark::Done);
        }
        if cx.live.reveal.touches(task_box) {
            builder.mark(task_box, Mark::Markup);
        } else {
            builder.atom(
                Span::new(task_box.start, end),
                AtomKind::Checkbox { checked },
                None,
            );
        }
    }
    Some(ListDecor {
        depth,
        marker: shown,
        marker_source: marker,
    })
}

pub(crate) fn fence_chars(source: &str, marker: Span) -> Span {
    let text = marker.slice(source);
    let len = text.len() - text.trim_start_matches(['`', '~']).len();
    Span::new(marker.start, marker.start + len)
}

fn code_line(
    cx: &LineCx<'_>,
    block: &Block,
    fenced: bool,
    builder: &mut LineBuilder<'_>,
) -> CodeLine {
    let first = first_line(cx.document, block.span);
    let last = last_line(cx.document, block.span);
    let position = if fenced && cx.line == first {
        CodeLinePosition::OpenFence
    } else if fenced && block.markers.len() > 1 && cx.line == last {
        CodeLinePosition::CloseFence
    } else {
        CodeLinePosition::Body
    };
    let fence = match position {
        CodeLinePosition::OpenFence => block.markers.first(),
        CodeLinePosition::CloseFence => block.markers.get(1),
        CodeLinePosition::Body => None,
    };
    let mut fence_hidden = false;
    if let Some(fence) = fence {
        if cx.live.reveal.touches(lines_span(cx.document, first, last)) {
            builder.mark(fence_chars(builder.source(), *fence), Mark::Markup);
        } else {
            builder.hide(*fence);
            fence_hidden = true;
        }
    }
    CodeLine {
        language: block.language().map(str::to_owned),
        position,
        block: block.span,
        content: block.content,
        fence_hidden,
    }
}

fn footnote_definition(
    cx: &mut LineCx<'_>,
    block: &Block,
    id: &str,
    builder: &mut LineBuilder<'_>,
) {
    let Some(marker) = block.markers.first() else {
        return;
    };
    let Some(close) = marker.slice(builder.source()).find("]:") else {
        return;
    };
    let label = Span::new(marker.start, marker.start + close + 2);
    if cx.touched {
        builder.mark(label, Mark::Markup);
        return;
    }
    let number = cx.live.footnotes.number(id);
    let kind = AtomKind::FootnoteDefinition {
        id: id.to_owned(),
        number,
    };
    builder.atom(label, kind, None);
}
