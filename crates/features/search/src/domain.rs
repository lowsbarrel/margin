use margin_kernel::{RelativePath, UnixMillis};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

const MARKDOWN_SUFFIX: &str = ".md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub path: RelativePath,
    pub name: String,
    pub snippet: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub files: Vec<RelativePath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlink {
    pub path: RelativePath,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedNote {
    pub path: RelativePath,
    pub body: String,
    pub modified: UnixMillis,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedNoteMeta {
    pub path: RelativePath,
    pub modified: UnixMillis,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyEntry {
    pub name: String,
    pub path: RelativePath,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    All,
    Any,
    Phrase,
}

fn terms(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|raw| {
            raw.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\"*"))
        .collect()
}

#[must_use]
pub fn build_match_query(input: &str) -> String {
    terms(input).join(" ")
}

#[must_use]
pub fn build_match_for(input: &str, mode: SearchMode) -> String {
    match mode {
        SearchMode::Phrase => {
            let phrase = input.replace('"', " ").trim().to_owned();
            if phrase.is_empty() {
                String::new()
            } else {
                format!("\"{phrase}\"")
            }
        }
        SearchMode::Any => terms(input).join(" OR "),
        SearchMode::All => build_match_query(input),
    }
}

#[must_use]
pub fn backlink_target(path: &RelativePath) -> String {
    path.file_stem().to_lowercase()
}

#[must_use]
pub fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();
        if bytes.starts_with(b"```") || bytes.starts_with(b"~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || is_heading(bytes) {
            continue;
        }
        let mut i = 0;
        while i < bytes.len() {
            if bytes.get(i) == Some(&b'#')
                && tag_can_start_at(bytes, i)
                && bytes.get(i + 1).is_some_and(u8::is_ascii_alphabetic)
            {
                let start = i + 1;
                let mut j = start;
                while bytes.get(j).is_some_and(|b| {
                    b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_' || *b == b'/'
                }) {
                    j += 1;
                }
                if let Some(slice) = bytes.get(start..j) {
                    let tag = str::from_utf8(slice)
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                }
                i = j;
                continue;
            }
            i += 1;
        }
    }
    tags
}

fn tag_can_start_at(bytes: &[u8], index: usize) -> bool {
    index == 0
        || bytes.get(index - 1).is_some_and(|previous| {
            !previous.is_ascii_alphanumeric() && *previous != b'_' && *previous != b'#'
        })
}

fn is_heading(bytes: &[u8]) -> bool {
    if bytes.first() != Some(&b'#') {
        return false;
    }
    let hashes = bytes.iter().take_while(|&&b| b == b'#').count();
    bytes.get(hashes).is_none_or(|&b| b == b' ')
}

#[must_use]
pub fn parse_wiki_links(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut results = Vec::new();
    if bytes.len() < 4 {
        return results;
    }
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes.get(i) == Some(&b'[') && bytes.get(i + 1) == Some(&b'[') {
            if i > 0 && bytes.get(i - 1) == Some(&b'!') {
                i += 2;
                continue;
            }
            if let Some(close) = find_close_brackets(bytes, i + 2) {
                if let Some(title_bytes) = bytes.get(i + 2..close)
                    && !title_bytes
                        .iter()
                        .any(|&b| b == b'[' || b == b']' || b == b'\n')
                    && let Ok(title) = str::from_utf8(title_bytes)
                {
                    let target = title.split(['|', '#']).next().unwrap_or(title).trim();
                    let target = target.strip_suffix('\\').unwrap_or(target).trim();
                    if !target.is_empty() {
                        results.push(target.to_owned());
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
        if bytes.get(i) == Some(&b']') && bytes.get(i + 1) == Some(&b']') {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn display_name(name: &str) -> String {
    name.strip_suffix(MARKDOWN_SUFFIX)
        .unwrap_or(name)
        .to_owned()
}

#[must_use]
pub fn filter_files(files: &[FuzzyEntry], query: &str, limit: usize) -> Vec<FuzzyEntry> {
    if query.trim().is_empty() {
        let mut names: Vec<FuzzyEntry> = files
            .iter()
            .map(|entry| FuzzyEntry {
                name: display_name(&entry.name),
                path: entry.path.clone(),
            })
            .collect();
        names.sort_by(|a, b| a.name.cmp(&b.name));
        names.truncate(limit);
        return names;
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buffer: Vec<char> = Vec::new();

    let mut scored: Vec<(FuzzyEntry, u32)> = files
        .iter()
        .filter_map(|entry| {
            let name = display_name(&entry.name);
            buffer.clear();
            let score = pattern.score(Utf32Str::new(&name, &mut buffer), &mut matcher)?;
            Some((
                FuzzyEntry {
                    name,
                    path: entry.path.clone(),
                },
                score,
            ))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    scored.truncate(limit);
    scored.into_iter().map(|(entry, _)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> FuzzyEntry {
        FuzzyEntry {
            name: name.to_owned(),
            path: RelativePath::new(path).expect("valid path"),
        }
    }

    #[test]
    fn wiki_links_are_parsed_in_order() {
        assert_eq!(
            parse_wiki_links("see [[My Note]] and [[Other]]").as_slice(),
            ["My Note", "Other"]
        );
    }

    #[test]
    fn image_embeds_are_not_wiki_links() {
        assert_eq!(
            parse_wiki_links("![[image.png]] and [[real link]]").as_slice(),
            ["real link"]
        );
    }

    #[test]
    fn aliases_headings_and_escaped_pipes_reduce_to_the_target() {
        assert_eq!(
            parse_wiki_links("[[Project|the plan]] [[Project#Goals]] [[Project\\|x]]").as_slice(),
            ["Project", "Project", "Project"]
        );
    }

    #[test]
    fn tags_use_the_obsidian_alphabet_below_headings_and_outside_code() {
        assert_eq!(
            extract_tags("# heading\n#real #nested/tag #a-b_c"),
            ["real", "nested/tag", "a-b_c"]
        );
        assert_eq!(extract_tags("```\n#fenced\n```\ntext #after"), ["after"]);
        assert_eq!(extract_tags("word#no #yes"), ["yes"]);
    }

    #[test]
    fn the_default_query_ands_quoted_prefix_terms() {
        assert_eq!(build_match_query("Hello, World!"), "\"Hello\"* \"World\"*");
        assert_eq!(build_match_query("  ###  "), "");
    }

    #[test]
    fn phrase_and_any_modes_change_the_join() {
        assert_eq!(build_match_for("a b", SearchMode::Any), "\"a\"* OR \"b\"*");
        assert_eq!(build_match_for("a b", SearchMode::Phrase), "\"a b\"");
        assert_eq!(build_match_for("\"a\"", SearchMode::Phrase), "\"a\"");
        assert_eq!(build_match_for("   ", SearchMode::Phrase), "");
    }

    #[test]
    fn backlink_targets_are_lowercased_stems() {
        assert_eq!(
            backlink_target(&RelativePath::new("notes/Road Map.md").expect("valid")),
            "road map"
        );
    }

    #[test]
    fn an_empty_fuzzy_query_lists_names_without_the_suffix() {
        let files = [entry("b.md", "b.md"), entry("a.md", "a.md")];
        let filtered = filter_files(&files, "  ", 10);
        assert_eq!(
            filtered.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(filter_files(&files, "", 1).len(), 1);
    }

    #[test]
    fn a_fuzzy_query_keeps_only_matches() {
        let files = [
            entry("roadmap.md", "roadmap.md"),
            entry("readme.md", "readme.md"),
        ];
        assert_eq!(filter_files(&files, "rm", 10).len(), 2);
        assert_eq!(
            filter_files(&files, "road", 10)
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["roadmap"]
        );
        assert!(filter_files(&files, "zzzz", 10).is_empty());
    }
}
