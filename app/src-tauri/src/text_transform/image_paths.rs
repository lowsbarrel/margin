use super::{IMAGE_EXTS, LEGACY_LOCALFILE_PREFIX, LOCALFILE_URL_PREFIX};

pub(super) fn resolve_wiki_embeds(md: &str, folder: &str) -> String {
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

/// Unescape markdown image syntax like `!\[alt\](url)` → `![alt](url)`.
/// This happens when tiptap-markdown's serializer escapes `[`/`]` characters
/// after the content failed to parse as an image (e.g. URL had literal
/// spaces). Without this step, the image stays permanently broken because
/// the escape makes markdown-it treat it as plain text.
fn unescape_image_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let bytes = md.as_bytes();
    let mut pos = 0;

    while pos < md.len() {
        let escape_start = match md[pos..].find("!\\[") {
            Some(offset) => pos + offset,
            None => {
                result.push_str(&md[pos..]);
                break;
            }
        };

        result.push_str(&md[pos..escape_start]);
        let alt_start = escape_start + 3;

        // Find the first `]` or `\]` that closes the alt text, skipping
        // over nested escape pairs like `\[`.
        let mut i = alt_start;
        let mut alt_end = None;
        let mut skip_len = 0;
        while i < md.len() {
            if bytes[i] == b'\\' && i + 1 < md.len() && bytes[i + 1] == b']' {
                alt_end = Some(i);
                skip_len = 2;
                break;
            }
            if bytes[i] == b']' {
                alt_end = Some(i);
                skip_len = 1;
                break;
            }
            if bytes[i] == b'\\' && i + 1 < md.len() {
                i += 2;
                continue;
            }
            i += 1;
        }

        let (alt_end, after_close) = match alt_end {
            Some(e) => (e, e + skip_len),
            None => {
                result.push_str("!\\[");
                pos = alt_start;
                continue;
            }
        };

        if after_close < md.len() && bytes[after_close] == b'(' {
            if let Some(paren) = md[after_close + 1..].find(')') {
                let url_start = after_close + 1;
                let url_end = url_start + paren;
                let alt = &md[alt_start..alt_end];
                let url = &md[url_start..url_end];
                result.push_str(&format!("![{}]({})", alt, url));
                pos = url_end + 1;
                continue;
            }
        }

        result.push_str("!\\[");
        pos = alt_start;
    }

    result
}

fn encode_spaces_in_localfile_urls(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut pos = 0;
    while pos < md.len() {
        let next = md[pos..]
            .find("](localfile://")
            .map(|s| (pos + s, "](localfile://".len()))
            .or_else(|| {
                md[pos..]
                    .find("](http://localfile.localhost")
                    .map(|s| (pos + s, "](http://localfile.localhost".len()))
            });
        if let Some((abs_start, _)) = next {
            let url_start = abs_start + 2;
            if let Some(close) = md[url_start..].find(')') {
                let url = &md[url_start..url_start + close];
                result.push_str(&md[pos..url_start]);
                result.push_str(&url.replace(' ', "%20"));
                result.push(')');
                pos = url_start + close + 1;
                continue;
            }
        }
        result.push_str(&md[pos..]);
        break;
    }
    result
}

/// On Windows/Android, rewrite `localfile://localhost/…` URLs inside image
/// markdown to `http://localfile.localhost/…` so WebView2 can resolve them.
#[cfg(any(target_os = "windows", target_os = "android"))]
pub(super) fn rewrite_legacy_localfile_urls(md: &str) -> String {
    if !md.contains("localfile://localhost") {
        return md.to_string();
    }
    md.replace("localfile://localhost", LOCALFILE_URL_PREFIX)
}

#[cfg(not(any(target_os = "windows", target_os = "android")))]
pub(super) fn rewrite_legacy_localfile_urls(md: &str) -> String {
    md.to_string()
}

pub(super) fn resolve_image_paths(md: &str, vault_path: &str) -> String {
    // Fix previously-saved malformed localfile:// URLs where the slash between
    // "localhost" and the drive letter was missing (Windows bug).
    // e.g. localfile://localhostC: → localfile://localhost/C:
    let md = if md.contains("localfile://localhost") && !md.contains("localfile://localhost/") {
        md.replace("localfile://localhost", "localfile://localhost/")
    } else {
        md.to_string()
    };
    let md = unescape_image_markdown(&md);
    let md = rewrite_legacy_localfile_urls(&md);
    let md = encode_spaces_in_localfile_urls(&md);
    let md = md.as_str();

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
                                let sep = if vault_path.starts_with('/') { "" } else { "/" };
                                let full = format!("{}/{}", vault_path, url).replace(' ', "%20");
                                result.push_str(&format!(
                                    "![{}]({}{}{})",
                                    alt, LOCALFILE_URL_PREFIX, sep, full
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

pub(super) fn unresolve_image_paths(md: &str, vault_path: &str) -> String {
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
                        let stripped = url
                            .strip_prefix("http://localfile.localhost")
                            .or_else(|| url.strip_prefix(LEGACY_LOCALFILE_PREFIX));
                        if let Some(abs_path) = stripped {
                            // On Windows abs_path may be "/C:/Users/..." while
                            // vault_path is "C:/Users/...". Strip the leading /.
                            let norm_abs = abs_path.strip_prefix('/').unwrap_or(abs_path);
                            // Spaces may be %20-encoded in the URL
                            let decoded_abs = norm_abs.replace("%20", " ");
                            let vault_prefix = format!("{}/", vault_path);
                            let rel_path = if decoded_abs.starts_with(&vault_prefix) {
                                decoded_abs[vault_prefix.len()..].to_string()
                            } else if norm_abs.starts_with(&vault_prefix.replace(' ', "%20")) {
                                let enc_prefix = vault_prefix.replace(' ', "%20");
                                norm_abs[enc_prefix.len()..].to_string()
                            } else {
                                decoded_abs
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
