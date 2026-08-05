/// Extract `#tag` tokens from markdown content. Called while a note is being
/// indexed — the `tags` table is the only tag scan in the app.
pub(crate) fn extract_tags_from_content(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();

        if bytes.starts_with(b"```") || bytes.starts_with(b"~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        // Skip markdown headings (# Heading, ## Heading, …)
        if bytes.first() == Some(&b'#') {
            let hash_end = bytes.iter().position(|&b| b != b'#').unwrap_or(bytes.len());
            if hash_end >= bytes.len() || bytes[hash_end] == b' ' {
                continue;
            }
        }

        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                let prev_ok = i == 0 || {
                    let p = bytes[i - 1];
                    !p.is_ascii_alphanumeric() && p != b'_' && p != b'#'
                };
                if prev_ok && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
                    let start = i + 1;
                    let mut j = start;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric()
                            || bytes[j] == b'-'
                            || bytes[j] == b'_'
                            || bytes[j] == b'/')
                    {
                        j += 1;
                    }
                    let tag = std::str::from_utf8(&bytes[start..j])
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }

    tags
}
