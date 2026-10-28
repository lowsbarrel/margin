use std::collections::BTreeMap;
use std::sync::Arc;

use margin_ask::{Error, LineMatch, RecentNote, SearchHit, SearchMode, TagSummary, VaultReader};
use margin_kernel::{RelativePath, StoreError, UnixMillis};
use parking_lot::Mutex;
use regex::Regex;

#[derive(Clone, Debug)]
struct Note {
    body: String,
    modified: UnixMillis,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryVaultReader {
    notes: Arc<Mutex<BTreeMap<RelativePath, Note>>>,
}

impl MemoryVaultReader {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, path: &str, body: &str) {
        self.insert_modified(path, body, UnixMillis::default());
    }

    pub fn insert_modified(&self, path: &str, body: &str, modified: UnixMillis) {
        let path = RelativePath::new(path).expect("fixture paths are valid");
        self.notes.lock().insert(
            path,
            Note {
                body: body.to_owned(),
                modified,
            },
        );
    }
}

impl VaultReader for MemoryVaultReader {
    fn search(&self, query: &str, mode: SearchMode, limit: usize) -> Result<Vec<SearchHit>, Error> {
        let phrase = query.trim().to_lowercase();
        if phrase.is_empty() {
            return Ok(Vec::new());
        }
        let terms = tokens(&phrase);
        let mut hits = Vec::new();
        for (path, note) in self.notes.lock().iter() {
            let haystack = tokens(&note.body);
            let hits_term = |term: &String| haystack.iter().any(|token| token.starts_with(term));
            let found = match mode {
                SearchMode::Phrase => has_sequence(&haystack, &terms),
                SearchMode::Any => terms.iter().any(hits_term),
                SearchMode::All => terms.iter().all(hits_term),
            };
            if !found {
                continue;
            }
            let needle = terms.first().map_or(phrase.as_str(), String::as_str);
            hits.push(SearchHit {
                path: path.clone(),
                snippet: snippet(&note.body, needle),
            });
            if hits.len() >= limit {
                break;
            }
        }
        Ok(hits)
    }

    fn grep(
        &self,
        pattern: &Regex,
        folder: Option<&RelativePath>,
        max_file_bytes: u64,
        limit: usize,
    ) -> Result<Vec<LineMatch>, Error> {
        let mut matches = Vec::new();
        for (path, note) in self.notes.lock().iter() {
            if folder.is_some_and(|folder| !path.starts_with(folder)) {
                continue;
            }
            if u64::try_from(note.body.len()).unwrap_or(u64::MAX) > max_file_bytes {
                continue;
            }
            for (index, line) in note.body.lines().enumerate() {
                if matches.len() >= limit {
                    return Ok(matches);
                }
                if pattern.is_match(line) {
                    matches.push(LineMatch {
                        path: path.clone(),
                        line: index + 1,
                        text: line.to_owned(),
                    });
                }
            }
        }
        Ok(matches)
    }

    fn find_notes(&self, name: &str, limit: usize) -> Result<Vec<RelativePath>, Error> {
        let needle = name.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .notes
            .lock()
            .keys()
            .filter(|path| path.file_stem().to_lowercase().contains(&needle))
            .take(limit)
            .cloned()
            .collect())
    }

    fn read_note(&self, path: &RelativePath) -> Result<String, Error> {
        self.notes
            .lock()
            .get(path)
            .map(|note| note.body.clone())
            .ok_or_else(|| Error::Store(StoreError::NotFound(path.to_string())))
    }

    fn tags(&self) -> Result<Vec<TagSummary>, Error> {
        let mut by_tag: BTreeMap<String, Vec<RelativePath>> = BTreeMap::new();
        for (path, note) in self.notes.lock().iter() {
            for tag in extract_tags(&note.body) {
                by_tag.entry(tag).or_default().push(path.clone());
            }
        }
        Ok(by_tag
            .into_iter()
            .map(|(tag, paths)| TagSummary { tag, paths })
            .collect())
    }

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<RelativePath>, Error> {
        let target = path.file_stem().to_lowercase();
        Ok(self
            .notes
            .lock()
            .iter()
            .filter(|(candidate, note)| {
                *candidate != path && wiki_links(&note.body).iter().any(|link| link == &target)
            })
            .map(|(candidate, _)| candidate.clone())
            .collect())
    }

    fn recent_notes(&self, since: UnixMillis) -> Result<Vec<RecentNote>, Error> {
        Ok(self
            .notes
            .lock()
            .iter()
            .filter(|(_, note)| note.modified >= since)
            .map(|(path, note)| RecentNote {
                path: path.clone(),
                modified: note.modified,
            })
            .collect())
    }
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn has_sequence(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    (0..=haystack.len() - needle.len()).any(|start| {
        haystack[start..start + needle.len()]
            .iter()
            .zip(needle)
            .all(|(token, wanted)| token == wanted)
    })
}

fn snippet(body: &str, needle: &str) -> String {
    body.lines()
        .find(|line| line.to_lowercase().contains(needle))
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn wiki_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    for (start, _) in body.match_indices("[[") {
        let rest = &body[start + 2..];
        let Some(end) = rest.find("]]") else {
            continue;
        };
        let name = rest[..end]
            .split(['|', '#'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if !name.is_empty() {
            links.push(name);
        }
    }
    links
}

fn extract_tags(body: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || is_heading(trimmed) {
            continue;
        }
        tags.extend(tags_in_line(trimmed));
    }
    tags
}

fn is_heading(line: &str) -> bool {
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    hashes > 0
        && line
            .chars()
            .nth(hashes)
            .is_none_or(|character| character == ' ')
}

fn tags_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tags = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' {
            let boundary = index == 0
                || !(bytes[index - 1].is_ascii_alphanumeric()
                    || bytes[index - 1] == b'_'
                    || bytes[index - 1] == b'#');
            if boundary && bytes.get(index + 1).is_some_and(u8::is_ascii_alphabetic) {
                let start = index + 1;
                let mut end = start;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric()
                        || matches!(bytes[end], b'-' | b'_' | b'/'))
                {
                    end += 1;
                }
                tags.push(line[start..end].to_ascii_lowercase());
                index = end;
                continue;
            }
        }
        index += 1;
    }
    tags
}
