use std::ops::Range;

use margin_trash::TrashItem;

pub(crate) fn query_terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

pub(crate) fn matches(item: &TrashItem, terms: &[String]) -> bool {
    let name = item.name.to_lowercase();
    let path = item.path.as_str().to_lowercase();
    terms
        .iter()
        .all(|term| name.contains(term.as_str()) || path.contains(term.as_str()))
}

pub(crate) fn highlight_ranges(text: &str, terms: &[String]) -> Vec<Range<usize>> {
    let lowered = text.to_lowercase();
    if lowered.len() != text.len() {
        return Vec::new();
    }
    let mut ranges: Vec<Range<usize>> = terms
        .iter()
        .filter(|term| !term.is_empty())
        .flat_map(|term| {
            lowered
                .match_indices(term.as_str())
                .map(|(start, found)| start..start + found.len())
        })
        .collect();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use margin_kernel::{RelativePath, UnixMillis};

    use super::*;

    fn item(path: &str) -> TrashItem {
        let path = RelativePath::new(path).expect("valid path");
        TrashItem {
            id: String::from("1-0"),
            name: path.file_name().to_owned(),
            path,
            is_dir: false,
            deleted_at: UnixMillis::from_millis(1),
            has_history: false,
        }
    }

    #[test]
    fn every_term_must_match_the_name_or_the_folder() {
        let roadmap = item("work/plans/Roadmap.md");
        assert!(matches(&roadmap, &query_terms("  ROAD  plans ")));
        assert!(matches(&roadmap, &query_terms("")));
        assert!(!matches(&roadmap, &query_terms("road home")));
    }

    #[test]
    fn highlights_merge_overlapping_matches_in_text_order() {
        assert_eq!(
            highlight_ranges("Roadmap road", &query_terms("road dma")),
            vec![0..6, 8..12]
        );
        assert_eq!(
            highlight_ranges("İstanbul", &query_terms("i")),
            Vec::<Range<usize>>::new()
        );
    }
}
