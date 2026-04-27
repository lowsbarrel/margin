mod image_paths;

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

#[derive(Serialize, Deserialize, Clone)]
pub struct FuzzyEntry {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub fn fuzzy_filter_files(files: Vec<FuzzyEntry>, query: String, limit: usize) -> Vec<FuzzyEntry> {
    let q = query.to_lowercase();
    let mut scored: Vec<(FuzzyEntry, f64)> = Vec::new();

    for entry in files {
        let name = entry
            .name
            .strip_suffix(".md")
            .unwrap_or(&entry.name)
            .to_string();
        let name_lower = name.to_lowercase();
        let score: f64;

        if q.is_empty() {
            score = 0.0;
        } else if let Some(idx) = name_lower.find(&q) {
            score = 1000.0 - idx as f64 + (q.len() as f64 / name_lower.len() as f64) * 500.0;
        } else {
            let q_chars: Vec<char> = q.chars().collect();
            let mut qi = 0;
            let mut s = 0.0;
            let mut last_match: Option<usize> = None;
            for (ti, tc) in name_lower.chars().enumerate() {
                if qi < q_chars.len() && tc == q_chars[qi] {
                    s += 10.0;
                    if let Some(lm) = last_match {
                        if lm + 1 == ti {
                            s += 15.0;
                        }
                    }
                    last_match = Some(ti);
                    qi += 1;
                }
            }
            if qi == q_chars.len() {
                score = s;
            } else {
                continue;
            }
        }

        scored.push((
            FuzzyEntry {
                name,
                path: entry.path,
            },
            score,
        ));
    }

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });

    scored
        .into_iter()
        .take(limit)
        .map(|(entry, _)| entry)
        .collect()
}

#[tauri::command]
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
