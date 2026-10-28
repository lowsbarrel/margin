use margin_markdown::{Document, Inline, InlineKind, Span};

use crate::block_path::path_of;
use crate::inline_live::{is_inline_link, link_tail, wiki_parts};
use crate::render::leaf_inlines;
use crate::row::Reveal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HiddenRun {
    span: Span,
    opens: bool,
}

#[must_use]
pub fn outside_hidden(document: &Document, offset: usize, reveal: &Reveal) -> usize {
    let line = document.line_span(document.line_of(offset));
    let path = path_of(document.blocks(), line);
    let mut runs = Vec::new();
    for inlines in leaf_inlines(path.last().copied(), line) {
        collect(document.text(), inlines, reveal, &mut runs);
    }
    runs.sort_by_key(|run| run.span.start);
    let mut merged: Vec<HiddenRun> = Vec::with_capacity(runs.len());
    for run in runs {
        match merged.last_mut() {
            Some(last) if last.span.end == run.span.start => last.span.end = run.span.end,
            _ => merged.push(run),
        }
    }
    merged
        .iter()
        .find(|run| run.span.contains(offset))
        .map_or(offset, |run| {
            if run.opens {
                run.span.start
            } else {
                run.span.end
            }
        })
}

fn push(runs: &mut Vec<HiddenRun>, owner: Span, span: Span) {
    if !span.is_empty() {
        runs.push(HiddenRun {
            span,
            opens: span.start == owner.start,
        });
    }
}

fn collect(source: &str, inlines: &[Inline], reveal: &Reveal, runs: &mut Vec<HiddenRun>) {
    for inline in inlines {
        let owner = inline.span;
        let hidden = !reveal.touches(owner);
        match &inline.kind {
            InlineKind::Emphasis
            | InlineKind::Strong
            | InlineKind::Strikethrough
            | InlineKind::Highlight
            | InlineKind::Code
            | InlineKind::Autolink { .. }
                if hidden =>
            {
                for marker in &inline.markers {
                    push(runs, owner, *marker);
                }
            }
            InlineKind::Link(_) if hidden && is_inline_link(inline) => {
                if let Some(open) = inline.markers.first() {
                    push(runs, owner, *open);
                }
                if let Some(tail) = link_tail(inline) {
                    push(runs, owner, tail);
                }
            }
            InlineKind::WikiLink(_) if hidden => {
                if let Some(parts) = wiki_parts(source, inline) {
                    push(runs, owner, parts.open);
                    if let Some(pipe) = parts.pipe {
                        push(runs, owner, Span::new(parts.target.start, pipe.end));
                    }
                    push(runs, owner, parts.close);
                }
            }
            _ => {}
        }
        collect(source, &inline.children, reveal, runs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_inside_hidden_markup_snap_outside_it() {
        let document = Document::parse("a **b** [l](u) c");
        let reveal = Reveal::default();
        assert_eq!(outside_hidden(&document, 3, &reveal), 2);
        assert_eq!(outside_hidden(&document, 6, &reveal), 7);
        assert_eq!(outside_hidden(&document, 11, &reveal), 14);
        assert_eq!(outside_hidden(&document, 15, &reveal), 15);
        assert_eq!(outside_hidden(&document, 3, &Reveal::new(vec![4])), 3);
    }
}
