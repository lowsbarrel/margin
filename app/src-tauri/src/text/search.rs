use serde::Serialize;

#[derive(Serialize, specta::Type)]
pub struct TextMatch {
    pub from: u32,
    pub to: u32,
}

// `gaps` holds sorted indices into `text` where a new block starts: a match crosses one when idx < g < idx+pat_len, and g == idx is fine.
#[tauri::command]
#[specta::specta]
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
        // ASCII-fold only: a Unicode fold changes byte length, and `idx` indexes `pm_offsets`, which is parallel to the original text.
        haystack = text.to_ascii_lowercase();
        pattern = needle.to_ascii_lowercase();
    };

    let hay = haystack.as_bytes();
    let pat = pattern.as_bytes();
    if pat.is_empty() {
        return Vec::new();
    }
    let pat_len = pat.len();
    let mut results = Vec::new();
    let mut start = 0usize;

    while start + pat_len <= hay.len() {
        let idx = match memchr::memmem::find(&hay[start..], pat) {
            Some(i) => start + i,
            None => break,
        };

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
        let text = "ab".to_string();
        let offsets = vec![0, 5];
        let gaps = vec![1];
        let results = search_in_text(text, offsets, gaps, "ab".into(), true);
        assert_eq!(results.len(), 0);
    }
}
