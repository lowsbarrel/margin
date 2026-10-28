use margin_markdown::{BlockKind, Document};
use margin_text_buffer::Selection;

pub(crate) fn find(document: &Document, anchor: &str) -> Option<Selection> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return None;
    }
    if let Some(id) = anchor.strip_prefix('^') {
        return block_reference(document, id);
    }
    heading(document, anchor).or_else(|| first_occurrence(document.text(), anchor))
}

fn heading(document: &Document, anchor: &str) -> Option<Selection> {
    let wanted = normalize(anchor);
    for line in 0..document.line_count() {
        let Some(block) = document.block_at(document.line_span(line).start) else {
            continue;
        };
        if !matches!(block.kind, BlockKind::Heading { .. })
            || block.span.start != document.line_span(line).start
        {
            continue;
        }
        if normalize(block.content.slice(document.text())) == wanted {
            return Some(Selection::cursor(block.content.start));
        }
    }
    None
}

fn block_reference(document: &Document, id: &str) -> Option<Selection> {
    let marker = format!("^{id}");
    for line in 0..document.line_count() {
        let text = document.line_text(line);
        if text.trim_end().ends_with(&marker) {
            return Some(Selection::cursor(document.line_span(line).start));
        }
    }
    None
}

fn first_occurrence(text: &str, needle: &str) -> Option<Selection> {
    let lower_text = text.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if lower_text.len() != text.len() || lower_needle.len() != needle.len() {
        let start = text.find(needle)?;
        return Some(Selection::new(start, start + needle.len()));
    }
    let start = lower_text.find(&lower_needle)?;
    Some(Selection::new(start, start + needle.len()))
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_heading_then_block_id_then_text() {
        let document = Document::parse("# Intro\n\nSome Words here ^abc\n\n## Next  Part\n");
        assert_eq!(find(&document, "next part"), Some(Selection::cursor(34)));
        assert_eq!(find(&document, "^abc"), Some(Selection::cursor(9)));
        assert_eq!(find(&document, "words"), Some(Selection::new(14, 19)));
        assert_eq!(find(&document, "missing"), None);
    }
}
