use margin_markdown::{Block, Document, Span};

use crate::row::Reveal;

pub(crate) fn path_of(blocks: &[Block], line: Span) -> Vec<&Block> {
    let mut path = Vec::new();
    let mut level = blocks;
    loop {
        let index = level.partition_point(|block| block.span.end < line.start);
        let Some(block) = level
            .get(index)
            .filter(|block| block.span.start <= line.end)
        else {
            break;
        };
        path.push(block);
        level = &block.children;
    }
    path
}

pub(crate) fn first_line(document: &Document, span: Span) -> usize {
    document.line_of(span.start)
}

pub(crate) fn last_line(document: &Document, span: Span) -> usize {
    document.line_of(span.end.max(span.start))
}

pub(crate) fn lines_span(document: &Document, first: usize, last: usize) -> Span {
    Span::new(
        document.line_span(first).start,
        document.line_span(last).end,
    )
}

pub(crate) fn block_touched(document: &Document, reveal: &Reveal, span: Span) -> bool {
    let first = first_line(document, span);
    let last = last_line(document, span);
    reveal.touches(lines_span(document, first, last))
}

pub(crate) fn trim_end_spaces(source: &str, span: Span) -> Span {
    let trimmed = span.slice(source).trim_end();
    Span::new(span.start, span.start + trimmed.len())
}

pub(crate) fn leading_space(source: &str, line: Span) -> Span {
    let text = line.slice(source);
    let lead = text.len() - text.trim_start_matches([' ', '\t']).len();
    Span::new(line.start, line.start + lead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_descends_to_the_leaf_of_a_line() {
        let document = Document::parse("> - a\n>   b\n\ntext");
        let path = path_of(document.blocks(), document.line_span(1));
        assert_eq!(path.len(), 4);
        let path = path_of(document.blocks(), document.line_span(2));
        assert!(path.is_empty());
    }
}
