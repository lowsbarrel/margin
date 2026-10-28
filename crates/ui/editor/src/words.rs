use margin_markdown::Document;

pub(crate) fn count(document: &Document) -> usize {
    let text = document.text();
    let body_start = document
        .frontmatter()
        .map_or(0, |frontmatter| frontmatter.span.end.min(text.len()));
    text.get(body_start..)
        .map_or(0, |body| body.split_whitespace().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_body_words_without_frontmatter() {
        let document = Document::parse("---\ntitle: A long title\n---\nOne two\nthree");
        assert_eq!(count(&document), 3);
    }
}
