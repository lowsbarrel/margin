use serde::Serialize;

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
    let idx = gaps.partition_point(|&g| g < lo);
    idx < gaps.len() && gaps[idx] < hi
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
}
