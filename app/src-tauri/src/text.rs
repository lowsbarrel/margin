use serde::{Deserialize, Serialize};

// ─── In-editor search matching ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct TextMatch {
    pub from: u32,
    pub to: u32,
}

/// Fast substring search on a flattened ProseMirror document.
///
/// `text`        – the concatenated text content of every text node.
/// `pm_offsets`  – parallel array: pm_offsets[i] is the ProseMirror position of text[i].
/// `gaps`        – sorted indices into `text` where a block boundary exists
///                 (i.e. pm_offsets[i] != pm_offsets[i-1] + 1). Matches that
///                 span a gap are rejected.
/// `needle`      – the search term.
/// `case_sensitive` – whether to compare case-sensitively.
///
/// Returns `(from, to)` pairs in ProseMirror position space.
#[tauri::command]
pub fn search_in_text(
    text: String,
    pm_offsets: Vec<u32>,
    gaps: Vec<u32>,
    needle: String,
    case_sensitive: bool,
) -> Vec<TextMatch> {
    if needle.is_empty() || text.len() < needle.len() {
        return Vec::new();
    }

    // Normalise case if needed
    let haystack: String;
    let pattern: String;
    if case_sensitive {
        haystack = text;
        pattern = needle;
    } else {
        haystack = text.to_lowercase();
        pattern = needle.to_lowercase();
    };

    let hay = haystack.as_bytes();
    let pat = pattern.as_bytes();
    let pat_len = pat.len();
    let mut results = Vec::new();
    let mut start = 0usize;

    while start + pat_len <= hay.len() {
        // Find next occurrence
        let idx = match memchr_find(hay, pat, start) {
            Some(i) => i,
            None => break,
        };

        // Check if this match spans a block boundary.
        // A gap at index g means text[g] is in a different block than text[g-1].
        // So a match [idx..idx+pat_len) crosses a gap if any gap g satisfies idx < g < idx+pat_len.
        // (gap at idx itself is fine — the match starts at a new block.)
        let crosses_gap = has_gap_in_range(&gaps, (idx + 1) as u32, (idx + pat_len) as u32);

        if !crosses_gap {
            let from = pm_offsets[idx];
            let to = pm_offsets[idx + pat_len - 1] + 1;
            results.push(TextMatch { from, to });
        }

        start = idx + 1;
    }

    results
}

/// Simple byte-level substring search (no allocations beyond the input).
/// For short patterns this is competitive with more complex algorithms.
fn memchr_find(hay: &[u8], pat: &[u8], start: usize) -> Option<usize> {
    let first = pat[0];
    let hay = &hay[start..];
    let mut i = 0;
    while i + pat.len() <= hay.len() {
        // Find next occurrence of first byte
        if let Some(j) = memchr::memchr(first, &hay[i..]) {
            let pos = i + j;
            if pos + pat.len() > hay.len() {
                return None;
            }
            if &hay[pos..pos + pat.len()] == pat {
                return Some(start + pos);
            }
            i = pos + 1;
        } else {
            return None;
        }
    }
    None
}

/// Binary-search check: is there any gap value g where lo <= g < hi?
fn has_gap_in_range(gaps: &[u32], lo: u32, hi: u32) -> bool {
    if gaps.is_empty() || lo >= hi {
        return false;
    }
    // Find first gap >= lo
    let idx = gaps.partition_point(|&g| g < lo);
    idx < gaps.len() && gaps[idx] < hi
}

// ─── Wiki-link extraction ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TextNode {
    pub text: String,
    /// ProseMirror position of the first character of this text node.
    pub pos: u32,
}

#[derive(Serialize)]
pub struct WikiLinkMatch {
    pub from: u32,
    pub to: u32,
    pub title: String,
}

/// Extract `[[title]]` wiki-links from a batch of ProseMirror text nodes.
///
/// Each `TextNode` carries the node's text content and its ProseMirror start
/// position. Returns `(from, to, title)` triples in PM position space.
///
/// This replaces the per-node regex scan in JS — a single IPC call handles
/// all text nodes at once.
#[tauri::command]
pub fn extract_wiki_links(nodes: Vec<TextNode>) -> Vec<WikiLinkMatch> {
    let mut results = Vec::new();

    for node in &nodes {
        let bytes = node.text.as_bytes();
        let len = bytes.len();
        if len < 4 {
            continue; // minimum: [[x]]
        }

        let mut i = 0;
        while i + 3 < len {
            // Look for [[ not preceded by !
            if bytes[i] == b'[' && bytes[i + 1] == b'[' {
                if i > 0 && bytes[i - 1] == b'!' {
                    i += 2;
                    continue;
                }
                // Find closing ]]
                if let Some(close) = find_close_brackets(bytes, i + 2) {
                    let title_bytes = &bytes[i + 2..close];
                    // Reject if title contains [ or ] or newline
                    if !title_bytes.iter().any(|&b| b == b'[' || b == b']' || b == b'\n') {
                        if let Ok(title) = std::str::from_utf8(title_bytes) {
                            let title = title.trim();
                            if !title.is_empty() {
                                // Match covers [[ ... ]]
                                let match_start = i;
                                let match_end = close + 2; // past the ]]
                                // Convert byte offsets to char offsets for correct PM mapping
                                let char_start = node.text[..match_start].chars().count();
                                let char_end = node.text[..match_end].chars().count();
                                results.push(WikiLinkMatch {
                                    from: node.pos + char_start as u32,
                                    to: node.pos + char_end as u32,
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
    fn test_search_basic() {
        let text = "hello world hello".to_string();
        let offsets: Vec<u32> = (0..text.len() as u32).collect();
        let gaps = vec![];
        let results = search_in_text(text, offsets, gaps, "hello".into(), true);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].from, 0);
        assert_eq!(results[0].to, 5);
        assert_eq!(results[1].from, 12);
        assert_eq!(results[1].to, 17);
    }

    #[test]
    fn test_search_case_insensitive() {
        let text = "Hello HELLO".to_string();
        let offsets: Vec<u32> = (0..text.len() as u32).collect();
        let results = search_in_text(text, offsets, vec![], "hello".into(), false);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_rejects_cross_block() {
        // "ab" with a gap at index 1 (block boundary between a and b)
        let text = "ab".to_string();
        let offsets = vec![0, 5]; // gap: positions aren't consecutive
        let gaps = vec![1]; // gap at index 1
        let results = search_in_text(text, offsets, gaps, "ab".into(), true);
        assert_eq!(results.len(), 0);
    }

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
