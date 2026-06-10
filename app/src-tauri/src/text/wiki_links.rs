use serde::{Deserialize, Serialize};

#[derive(Deserialize, specta::Type)]
pub struct TextNode {
    pub text: String,
    /// ProseMirror position of the first character of this text node.
    pub pos: u32,
}

#[derive(Serialize, specta::Type)]
pub struct WikiLinkMatch {
    pub from: u32,
    pub to: u32,
    pub title: String,
}

/// A single parsed `[[title]]` wiki-link: byte offsets into the source text
/// (`start` points at the first `[`, `end` is just past the closing `]]`) plus
/// the trimmed title. This is the single source of truth for wiki-link parsing
/// rules, shared by the PM-node extractor here and the file-based scan in
/// `fs/mod.rs::read_link_batch`.
pub struct ParsedWikiLink {
    pub start: usize,
    pub end: usize,
    pub title: String,
}

/// Parse all `[[title]]` wiki-links from `text`, rejecting `![[image embeds]]`,
/// empty titles, and titles containing `[`, `]` or a newline.
pub fn parse_wiki_links(text: &str) -> Vec<ParsedWikiLink> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    if len < 4 {
        return results; // minimum: [[x]]
    }

    let mut i = 0;
    while i + 3 < len {
        // Look for [[ not preceded by !
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if i > 0 && bytes[i - 1] == b'!' {
                i += 2;
                continue;
            }
            if let Some(close) = find_close_brackets(bytes, i + 2) {
                let title_bytes = &bytes[i + 2..close];
                // Reject if title contains [ or ] or newline
                if !title_bytes
                    .iter()
                    .any(|&b| b == b'[' || b == b']' || b == b'\n')
                {
                    if let Ok(title) = std::str::from_utf8(title_bytes) {
                        let title = title.trim();
                        if !title.is_empty() {
                            results.push(ParsedWikiLink {
                                start: i,
                                end: close + 2, // past the ]]
                                title: title.to_string(),
                            });
                        }
                    }
                }
                i = close + 2;
                continue;
            }
        }
        i += 1;
    }

    results
}

/// Extract `[[title]]` wiki-links from a batch of ProseMirror text nodes.
///
/// Each `TextNode` carries the node's text content and its ProseMirror start
/// position. Returns `(from, to, title)` triples in PM position space.
///
/// This replaces the per-node regex scan in JS — a single IPC call handles
/// all text nodes at once.
#[tauri::command]
#[specta::specta]
pub fn extract_wiki_links(nodes: Vec<TextNode>) -> Vec<WikiLinkMatch> {
    let mut results = Vec::new();

    for node in &nodes {
        for link in parse_wiki_links(&node.text) {
            // Convert byte offsets to char offsets for correct PM mapping.
            let char_start = node.text[..link.start].chars().count();
            let char_end = node.text[..link.end].chars().count();
            results.push(WikiLinkMatch {
                from: node.pos + char_start as u32,
                to: node.pos + char_end as u32,
                title: link.title,
            });
        }
    }

    results
}

/// Find the position of `]]` starting from `start` in `bytes`.
fn find_close_brackets(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiki_links() {
        let nodes = vec![TextNode {
            text: "see [[My Note]] and [[Other]]".to_string(),
            pos: 10,
        }];
        let results = extract_wiki_links(nodes);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "My Note");
        assert_eq!(results[0].from, 14);
        assert_eq!(results[0].to, 25);
        assert_eq!(results[1].title, "Other");
    }

    #[test]
    fn test_wiki_links_skips_image_embeds() {
        let nodes = vec![TextNode {
            text: "![[image.png]] and [[real link]]".to_string(),
            pos: 0,
        }];
        let results = extract_wiki_links(nodes);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "real link");
    }
}
