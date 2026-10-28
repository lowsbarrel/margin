use std::cmp::Reverse;

use margin_markdown::{Block, Document, Inline, InlineKind, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, Assoc, RangeEdit};
use crate::text;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    Bold,
    Italic,
    Strike,
    Code,
    Highlight,
    Comment,
}

impl MarkKind {
    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
            Self::Strike => "~~",
            Self::Code => "`",
            Self::Highlight => "==",
            Self::Comment => "%%",
        }
    }

    const fn of(kind: &InlineKind) -> Option<Self> {
        match kind {
            InlineKind::Strong => Some(Self::Bold),
            InlineKind::Emphasis => Some(Self::Italic),
            InlineKind::Strikethrough => Some(Self::Strike),
            InlineKind::Code => Some(Self::Code),
            InlineKind::Highlight => Some(Self::Highlight),
            InlineKind::Comment => Some(Self::Comment),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MarkSpan {
    kind: MarkKind,
    span: Span,
    open: Span,
    close: Span,
}

fn covers(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}

fn collect_inline(inline: &Inline, range: Span, out: &mut Vec<MarkSpan>) {
    if !inline.span.overlaps(range.start, range.end) {
        return;
    }
    if let Some(kind) = MarkKind::of(&inline.kind)
        && let [open, close] = inline.markers.as_slice()
        && open.end <= close.start
        && inline.span.len() > open.len() + close.len()
    {
        out.push(MarkSpan {
            kind,
            span: inline.span,
            open: *open,
            close: *close,
        });
    }
    for child in &inline.children {
        collect_inline(child, range, out);
    }
}

fn collect_block(block: &Block, range: Span, out: &mut Vec<MarkSpan>) {
    if !block.span.overlaps(range.start, range.end) {
        return;
    }
    for inline in &block.inlines {
        collect_inline(inline, range, out);
    }
    if let Some(table) = block.table() {
        for cell in table.rows.iter().flat_map(|row| &row.cells) {
            for inline in &cell.inlines {
                collect_inline(inline, range, out);
            }
        }
    }
    for child in &block.children {
        collect_block(child, range, out);
    }
}

fn mark_spans(document: &Document, range: Span) -> Vec<MarkSpan> {
    let mut out = Vec::new();
    for block in document.blocks() {
        collect_block(block, range, &mut out);
    }
    out
}

#[must_use]
pub fn active_marks(document: &Document, selection: Selection) -> Vec<MarkKind> {
    if selection.is_empty() {
        return Vec::new();
    }
    let range = selection.span();
    let mut kinds = Vec::new();
    for mark in mark_spans(document, range) {
        if (covers(mark.span, range) || covers(range, mark.span)) && !kinds.contains(&mark.kind) {
            kinds.push(mark.kind);
        }
    }
    kinds
}

pub(crate) fn in_inline_code(document: &Document, pos: usize) -> bool {
    mark_spans(document, Span::empty(pos))
        .iter()
        .any(|mark| mark.kind == MarkKind::Code && mark.span.start < pos && pos <= mark.span.end)
}

fn content_range(range: Span, spans: &[MarkSpan]) -> Span {
    let mut ordered = spans.to_vec();
    ordered.sort_by_key(|mark| Reverse(mark.span.len()));
    let mut inner = range;
    for mark in ordered {
        if covers(mark.span, inner) {
            inner = Span::new(mark.open.end, mark.close.start);
        }
    }
    inner
}

fn add_mark(kind: MarkKind, target: Span, spans: &[MarkSpan], original: Selection) -> RangeEdit {
    let inner = content_range(target, spans);
    let marker = kind.marker();
    let edits = vec![
        TextEdit::new(Span::empty(inner.start), marker),
        TextEdit::new(Span::empty(inner.end), marker),
    ];
    let map = |pos: usize| {
        let assoc = if pos == original.start() {
            Assoc::After
        } else {
            Assoc::Before
        };
        mapping::map_pos(&edits, pos, assoc)
    };
    let selection = if original.is_empty() && kind != MarkKind::Comment {
        Selection::cursor(mapping::map_pos(&edits, original.head, Assoc::After))
    } else if original.is_empty() {
        Selection::new(
            mapping::map_pos(&edits, inner.start, Assoc::After),
            mapping::map_pos(&edits, inner.end, Assoc::Before),
        )
    } else {
        Selection::new(map(original.anchor), map(original.head))
    };
    RangeEdit::new(edits, selection)
}

fn mark_range(document: &Document, kind: MarkKind, range: Selection) -> RangeEdit {
    let span = range.span();
    let spans = mark_spans(document, span);
    let active = spans.iter().find(|mark| {
        mark.kind == kind
            && if range.is_empty() {
                mark.open.end <= span.start && span.start <= mark.close.start
            } else {
                covers(mark.span, span) || covers(span, mark.span)
            }
    });
    if let Some(active) = active {
        let edits = vec![
            TextEdit::new(active.open, ""),
            TextEdit::new(active.close, ""),
        ];
        let selection = mapping::map_range(&edits, range);
        return RangeEdit::new(edits, selection);
    }
    let target = if range.is_empty() {
        text::word_at(document, range.head)
    } else {
        Some(span)
    };
    if let Some(target) = target {
        return add_mark(kind, target, &spans, range);
    }
    let marker = kind.marker();
    RangeEdit::replace(
        Span::empty(span.start),
        marker.repeat(2),
        span.start + marker.len(),
    )
}

#[must_use]
pub fn toggle_mark(document: &Document, selections: &[Selection], kind: MarkKind) -> Change {
    mapping::by_range(selections, |range| Some(mark_range(document, kind, range)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    fn toggle(source: &str, kind: MarkKind) -> String {
        run(source, |document, selections| {
            Some(toggle_mark(document, selections, kind))
        })
        .expect("toggling a mark always applies")
    }

    #[test]
    fn wraps_the_word_under_an_empty_cursor() {
        assert_eq!(toggle("say he|llo", MarkKind::Bold), "say **he|llo**");
        assert_eq!(toggle("say |", MarkKind::Bold), "say **|**");
    }

    #[test]
    fn wraps_and_unwraps_a_selection() {
        assert_eq!(toggle("a «word» b", MarkKind::Italic), "a *«word»* b");
        assert_eq!(toggle("a *«word»* b", MarkKind::Italic), "a «word» b");
        assert_eq!(toggle("a «*word*» b", MarkKind::Italic), "a «word» b");
        assert_eq!(toggle("a **wo|rd** b", MarkKind::Bold), "a wo|rd b");
    }

    #[test]
    fn nests_a_new_mark_inside_an_existing_one() {
        assert_eq!(toggle("«**bold**»", MarkKind::Italic), "«***bold***»");
        assert_eq!(toggle("x ==ma|rk== y", MarkKind::Highlight), "x ma|rk y");
        assert_eq!(toggle("`co|de`", MarkKind::Code), "co|de");
        assert_eq!(toggle("~~«gone»~~", MarkKind::Strike), "«gone»");
    }

    #[test]
    fn comments_select_the_wrapped_word() {
        assert_eq!(toggle("note he|re", MarkKind::Comment), "note %%«here»%%");
        assert_eq!(toggle("note %%he|re%%", MarkKind::Comment), "note he|re");
    }

    #[test]
    fn reports_active_marks() {
        let document = Document::parse("a **b *c* d** e");
        assert_eq!(
            active_marks(&document, Selection::new(7, 8)),
            vec![MarkKind::Bold, MarkKind::Italic]
        );
        assert!(active_marks(&document, Selection::cursor(7)).is_empty());
    }
}
