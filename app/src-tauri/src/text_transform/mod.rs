mod image_paths;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico", "tiff", "tif",
];

// In Tauri 2, custom URI schemes on Windows/Android are served under
// `http://<scheme>.localhost/…` — the raw `scheme://…` form is rejected
// by WebView2 with ERR_UNKNOWN_URL_SCHEME. Keep these two prefixes in
// sync with the JS side (`image-url.ts`) and the CSP in tauri.conf.json.
#[cfg(any(target_os = "windows", target_os = "android"))]
const LOCALFILE_URL_PREFIX: &str = "http://localfile.localhost";
#[cfg(not(any(target_os = "windows", target_os = "android")))]
const LOCALFILE_URL_PREFIX: &str = "localfile://localhost";

// Legacy prefix — markdown files saved before the Windows fix may still
// contain this form. Unresolve and the Windows rewrite step both accept
// it so old notes keep working.
const LEGACY_LOCALFILE_PREFIX: &str = "localfile://localhost";

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct FuzzyEntry {
    pub name: String,
    pub path: String,
}

#[tauri::command]
#[specta::specta]
// `limit` is u32 (not usize) so specta can export it; a result cap never
// approaches u32::MAX.
//
// Fuzzy matching/ranking is delegated to the maintained `nucleo-matcher`
// crate (the engine behind helix/nucleo) instead of a hand-rolled scorer.
// The display `name` has its `.md` suffix stripped before matching so the
// extension never skews scores.
pub fn fuzzy_filter_files(files: Vec<FuzzyEntry>, query: String, limit: u32) -> Vec<FuzzyEntry> {
    let limit = limit as usize;

    // Empty query: nothing to rank, so preserve the previous behaviour of
    // returning entries (with `.md` stripped) sorted by name, capped at `limit`.
    if query.trim().is_empty() {
        let mut names: Vec<FuzzyEntry> = files
            .into_iter()
            .map(|entry| FuzzyEntry {
                name: entry
                    .name
                    .strip_suffix(".md")
                    .unwrap_or(&entry.name)
                    .to_string(),
                path: entry.path,
            })
            .collect();
        names.sort_by(|a, b| a.name.cmp(&b.name));
        names.truncate(limit);
        return names;
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&query, CaseMatching::Ignore, Normalization::Smart);

    // Reusable scratch buffer for the `Utf32Str` conversion the matcher needs.
    let mut haystack_buf: Vec<char> = Vec::new();

    let mut scored: Vec<(FuzzyEntry, u32)> = files
        .into_iter()
        .filter_map(|entry| {
            let name = entry
                .name
                .strip_suffix(".md")
                .unwrap_or(&entry.name)
                .to_string();

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

    // Best matches first; ties broken by name for a stable, deterministic order.
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

    scored
        .into_iter()
        .take(limit)
        .map(|(entry, _)| entry)
        .collect()
}

#[tauri::command]
#[specta::specta]
pub fn transform_image_paths(
    markdown: String,
    vault_path: Option<String>,
    attachment_folder: Option<String>,
    mode: String,
) -> String {
    let folder = attachment_folder.unwrap_or_else(|| "attachments".to_string());

    match mode.as_str() {
        "resolve" => {
            let md = image_paths::resolve_wiki_embeds(&markdown, &folder);
            match vault_path {
                Some(ref vp) => image_paths::resolve_image_paths(&md, vp),
                None => md,
            }
        }
        "unresolve" => match vault_path {
            Some(ref vp) => image_paths::unresolve_image_paths(&markdown, vp),
            None => markdown,
        },
        _ => markdown,
    }
}
