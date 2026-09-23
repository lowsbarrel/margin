use serde::{Deserialize, Serialize};

#[derive(Deserialize, specta::Type)]
pub struct TextNode {
    pub text: String,
    pub pos: u32,
}

#[derive(Serialize, specta::Type)]
pub struct WikiLinkMatch {
    pub from: u32,
    pub to: u32,
    pub title: String,
}

pub struct ParsedWikiLink {
    pub start: usize,
    pub end: usize,
    pub title: String,
}

pub fn parse_wiki_links(text: &str) -> Vec<ParsedWikiLink> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    if len < 4 {
        return results;
    }

    let mut i = 0;
    while i + 3 < len {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if i > 0 && bytes[i - 1] == b'!' {
                i += 2;
                continue;
            }
            if let Some(close) = find_close_brackets(bytes, i + 2) {
                let title_bytes = &bytes[i + 2..close];
                if !title_bytes
                    .iter()
                    .any(|&b| b == b'[' || b == b']' || b == b'\n')
                    && let Ok(title) = std::str::from_utf8(title_bytes)
                {
                    let title = title.trim();
                    if !title.is_empty() {
                        results.push(ParsedWikiLink {
                            start: i,
                            end: close + 2,
                            title: title.to_string(),
                        });
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

#[tauri::command]
#[specta::specta]
pub fn extract_wiki_links(nodes: Vec<TextNode>) -> Vec<WikiLinkMatch> {
    let mut results = Vec::new();

    for node in &nodes {
        for link in parse_wiki_links(&node.text) {
            // ProseMirror positions count chars while the parser works in bytes, so both ends are re-counted.
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
