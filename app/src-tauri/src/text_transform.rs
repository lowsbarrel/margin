use serde::{Deserialize, Serialize};

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico", "tiff", "tif",
];

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

        scored.push((FuzzyEntry { name, path: entry.path }, score));
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

fn resolve_wiki_embeds(md: &str, folder: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut pos = 0;

    while pos < md.len() {
        if let Some(start) = md[pos..].find("![[") {
            let abs_start = pos + start;
            result.push_str(&md[pos..abs_start]);

            if let Some(close) = md[abs_start + 3..].find(']') {
                let bracket_pos = abs_start + 3 + close;
                if bracket_pos + 1 < md.len() && md.as_bytes()[bracket_pos + 1] == b']' {
                    let filename = &md[abs_start + 3..bracket_pos];
                    let end_pos = bracket_pos + 2;
                    let mut transformed = false;

                    if let Some(dot_pos) = filename.rfind('.') {
                        let ext = filename[dot_pos + 1..].to_lowercase();
                        if IMAGE_EXTS.contains(&ext.as_str()) {
                            let rel_path = if filename.contains('/') {
                                filename.to_string()
                            } else {
                                format!("{}/{}", folder, filename)
                            };
                            let safe_path = rel_path.replace(' ', "%20");
                            result.push_str(&format!("![{}]({})", filename, safe_path));
                            transformed = true;
                        }
                    }

                    if !transformed {
                        result.push_str(&md[abs_start..end_pos]);
                    }
                    pos = end_pos;
                } else {
                    result.push_str("![[");
                    pos = abs_start + 3;
                }
            } else {
                result.push_str(&md[abs_start..]);
                break;
            }
        } else {
            result.push_str(&md[pos..]);
            break;
        }
    }

    result
}

fn resolve_image_paths(md: &str, vault_path: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut pos = 0;

    while pos < md.len() {
        if let Some(start) = md[pos..].find("![") {
            let abs_start = pos + start;

            if abs_start + 2 < md.len() && md.as_bytes()[abs_start + 2] == b'[' {
                result.push_str(&md[pos..abs_start + 2]);
                pos = abs_start + 2;
                continue;
            }

            if let Some(close_bracket) = md[abs_start + 2..].find(']') {
                let alt_end = abs_start + 2 + close_bracket;
                let alt = &md[abs_start + 2..alt_end];

                if alt_end + 1 < md.len() && md.as_bytes()[alt_end + 1] == b'(' {
                    let url_start = alt_end + 2;
                    if let Some(close_paren) = md[url_start..].find(')') {
                        if close_paren > 0 {
                            let url = &md[url_start..url_start + close_paren];
                            if !url.starts_with("http://")
                                && !url.starts_with("https://")
                                && !url.starts_with("data:")
                                && !url.starts_with("localfile://")
                            {
                                result.push_str(&md[pos..abs_start]);
                                result.push_str(&format!(
                                    "![{}](localfile://localhost{}/{})",
                                    alt, vault_path, url
                                ));
                                pos = url_start + close_paren + 1;
                                continue;
                            }
                        }
                    }
                }
            }

            result.push_str(&md[pos..abs_start + 2]);
            pos = abs_start + 2;
        } else {
            result.push_str(&md[pos..]);
            break;
        }
    }

    result
}

fn unresolve_image_paths(md: &str, vault_path: &str) -> String {
    let prefix = "localfile://localhost";
    let mut result = String::with_capacity(md.len());
    let mut pos = 0;

    while pos < md.len() {
        if let Some(start) = md[pos..].find("![") {
            let abs_start = pos + start;

            if abs_start + 2 < md.len() && md.as_bytes()[abs_start + 2] == b'[' {
                result.push_str(&md[pos..abs_start + 2]);
                pos = abs_start + 2;
                continue;
            }

            if let Some(close_bracket) = md[abs_start + 2..].find(']') {
                let alt_end = abs_start + 2 + close_bracket;
                let alt = &md[abs_start + 2..alt_end];

                if alt_end + 1 < md.len() && md.as_bytes()[alt_end + 1] == b'(' {
                    let url_start = alt_end + 2;
                    if let Some(close_paren) = md[url_start..].find(')') {
                        let url = &md[url_start..url_start + close_paren];
                        if let Some(abs_path) = url.strip_prefix(prefix) {
                            let vault_prefix = format!("{}/", vault_path);
                            let rel_path = if abs_path.starts_with(&vault_prefix) {
                                &abs_path[vault_prefix.len()..]
                            } else {
                                abs_path
                            };
                            result.push_str(&md[pos..abs_start]);
                            result.push_str(&format!("![{}]({})", alt, rel_path));
                            pos = url_start + close_paren + 1;
                            continue;
                        }
                    }
                }
            }

            result.push_str(&md[pos..abs_start + 2]);
            pos = abs_start + 2;
        } else {
            result.push_str(&md[pos..]);
            break;
        }
    }

    result
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
            let md = resolve_wiki_embeds(&markdown, &folder);
            match vault_path {
                Some(ref vp) => resolve_image_paths(&md, vp),
                None => md,
            }
        }
        "unresolve" => match vault_path {
            Some(ref vp) => unresolve_image_paths(&markdown, vp),
            None => markdown,
        },
        _ => markdown,
    }
}
