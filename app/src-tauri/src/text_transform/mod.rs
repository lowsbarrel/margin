use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct FuzzyEntry {
    pub name: String,
    pub path: String,
}

fn display_name(name: &str) -> String {
    name.strip_suffix(".md").unwrap_or(name).to_string()
}

// `limit` is u32 rather than usize so specta can export it.
#[tauri::command]
#[specta::specta]
pub fn fuzzy_filter_files(files: Vec<FuzzyEntry>, query: String, limit: u32) -> Vec<FuzzyEntry> {
    let limit = limit as usize;

    if query.trim().is_empty() {
        let mut names: Vec<FuzzyEntry> = files
            .into_iter()
            .map(|entry| FuzzyEntry {
                name: display_name(&entry.name),
                path: entry.path,
            })
            .collect();
        names.sort_by(|a, b| a.name.cmp(&b.name));
        names.truncate(limit);
        return names;
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&query, CaseMatching::Ignore, Normalization::Smart);

    let mut haystack_buf: Vec<char> = Vec::new();

    let mut scored: Vec<(FuzzyEntry, u32)> = files
        .into_iter()
        .filter_map(|entry| {
            let name = display_name(&entry.name);

            haystack_buf.clear();
            let haystack = Utf32Str::new(&name, &mut haystack_buf);
            let score = pattern.score(haystack, &mut matcher)?;

            Some((
                FuzzyEntry {
                    name,
                    path: entry.path,
                },
                score,
            ))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

    scored
        .into_iter()
        .take(limit)
        .map(|(entry, _)| entry)
        .collect()
}
