pub fn parse_wiki_links(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    if len < 4 {
        return results;
    }

    let mut i = 0;
    while i + 3 < len {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if i > 0 && bytes[i - 1] == b'!' {
                i += 2;
                continue;
            }
            if let Some(close) = find_close_brackets(bytes, i + 2) {
                let title_bytes = &bytes[i + 2..close];
                if !title_bytes
                    .iter()
                    .any(|&b| b == b'[' || b == b']' || b == b'\n')
                    && let Ok(title) = std::str::from_utf8(title_bytes)
                {
                    let title = title.trim();
                    if !title.is_empty() {
                        results.push(title.to_string());
                    }
                }
                i = close + 2;
                continue;
            }
        }
        i += 1;
    }

    results
}

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
    fn wiki_links_are_parsed_in_order() {
        let links = parse_wiki_links("see [[My Note]] and [[Other]]");
        assert_eq!(links.as_slice(), ["My Note", "Other"]);
    }

    #[test]
    fn image_embeds_are_not_wiki_links() {
        let links = parse_wiki_links("![[image.png]] and [[real link]]");
        assert_eq!(links.as_slice(), ["real link"]);
    }
}
