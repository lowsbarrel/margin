use margin_markdown::{BlockKind, Document, HeadingStyle};

#[must_use]
pub(crate) fn note_headings(text: &str) -> Vec<String> {
    let document = Document::parse(text);
    document
        .blocks()
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                BlockKind::Heading {
                    style: HeadingStyle::Atx,
                    ..
                }
            )
        })
        .map(|block| block.content.slice(text).trim().to_owned())
        .filter(|title| !title.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_atx_headings_outside_code_and_frontmatter() {
        let text =
            "---\ntitle: # no\n---\n# One\n\nbody\n## Two ##\n```\n# code\n```\n#\nSetext\n===\n";
        assert_eq!(note_headings(text), ["One", "Two"]);
    }
}
