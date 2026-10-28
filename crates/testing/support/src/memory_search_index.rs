use std::collections::{BTreeMap, BTreeSet};

use margin_kernel::{RelativePath, StoreError, UnixMillis};
use margin_search::{
    Backlink, IndexedNote, IndexedNoteMeta, SearchHit, SearchIndex, TagInfo, backlink_target,
    extract_tags, parse_wiki_links,
};
use parking_lot::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
enum MatchClause {
    Prefix(String),
    Phrase(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MatchQuery {
    any: bool,
    clauses: Vec<MatchClause>,
}

fn parse_match_query(query: &str) -> MatchQuery {
    let parts: Vec<&str> = query.split(" OR ").collect();
    MatchQuery {
        any: parts.len() > 1,
        clauses: parts.into_iter().flat_map(parse_clauses).collect(),
    }
}

fn parse_clauses(part: &str) -> Vec<MatchClause> {
    let mut clauses = Vec::new();
    let mut rest = part.trim();
    while let Some(tail) = rest.strip_prefix('"') {
        let Some((inner, after)) = tail.split_once('"') else {
            break;
        };
        let after = after.trim_start();
        let (prefix, remainder) = match after.strip_prefix('*') {
            Some(remainder) => (true, remainder),
            None => (false, after),
        };
        if !inner.is_empty() {
            let inner = inner.to_lowercase();
            clauses.push(if prefix {
                MatchClause::Prefix(inner)
            } else {
                MatchClause::Phrase(inner)
            });
        }
        rest = remainder.trim_start();
    }
    clauses
}

#[derive(Clone, Debug)]
struct Note {
    body: String,
    modified: UnixMillis,
    size: u64,
    tags: BTreeSet<String>,
    links: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct MemorySearchIndex {
    notes: Mutex<BTreeMap<RelativePath, Note>>,
}

impl MemorySearchIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SearchIndex for MemorySearchIndex {
    fn upsert(&self, note: &IndexedNote) -> Result<(), StoreError> {
        let entry = Note {
            body: note.body.clone(),
            modified: note.modified,
            size: note.size,
            tags: extract_tags(&note.body).into_iter().collect(),
            links: parse_wiki_links(&note.body)
                .into_iter()
                .map(|target| target.to_lowercase())
                .collect(),
        };
        self.notes.lock().insert(note.path.clone(), entry);
        Ok(())
    }

    fn remove(&self, path: &RelativePath) -> Result<(), StoreError> {
        self.notes.lock().retain(|key, _| !key.starts_with(path));
        Ok(())
    }

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        let mut notes = self.notes.lock();
        if let Some(note) = notes.remove(from) {
            notes.insert(to.clone(), note);
        }
        Ok(())
    }

    fn search(&self, match_query: &str, limit: u32) -> Result<Vec<SearchHit>, StoreError> {
        let query = parse_match_query(match_query);
        if query.clauses.is_empty() {
            return Ok(Vec::new());
        }
        let notes = self.notes.lock();
        let mut hits: Vec<SearchHit> = notes
            .iter()
            .filter(|(path, note)| matches_query(&query, path.file_name(), &note.body))
            .map(|(path, note)| SearchHit {
                path: path.clone(),
                name: path.file_name().to_owned(),
                snippet: snippet(&note.body, &query),
            })
            .collect();
        hits.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        hits.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(hits)
    }

    fn tags(&self) -> Result<Vec<TagInfo>, StoreError> {
        let notes = self.notes.lock();
        let mut by_tag: BTreeMap<String, Vec<RelativePath>> = BTreeMap::new();
        for (path, note) in notes.iter() {
            for tag in &note.tags {
                by_tag.entry(tag.clone()).or_default().push(path.clone());
            }
        }
        Ok(by_tag
            .into_iter()
            .map(|(tag, files)| TagInfo {
                count: files.len(),
                tag,
                files,
            })
            .collect())
    }

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<Backlink>, StoreError> {
        let target = backlink_target(path);
        let notes = self.notes.lock();
        let mut backlinks: Vec<Backlink> = notes
            .iter()
            .filter(|(source, note)| {
                source.as_str() != path.as_str() && note.links.contains(&target)
            })
            .map(|(source, _)| Backlink {
                path: source.clone(),
                name: source.file_name().to_owned(),
            })
            .collect();
        backlinks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(backlinks)
    }

    fn indexed(&self) -> Result<Vec<IndexedNoteMeta>, StoreError> {
        let notes = self.notes.lock();
        Ok(notes
            .iter()
            .map(|(path, note)| IndexedNoteMeta {
                path: path.clone(),
                modified: note.modified,
                size: note.size,
            })
            .collect())
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.notes.lock().clear();
        Ok(())
    }
}

fn matches_query(query: &MatchQuery, name: &str, body: &str) -> bool {
    let haystack = format!("{name} {body}").to_lowercase();
    let evaluate = |clause: &MatchClause| match clause {
        MatchClause::Prefix(prefix) => haystack
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|token| token.starts_with(prefix.as_str())),
        MatchClause::Phrase(phrase) => haystack.contains(phrase.as_str()),
    };
    if query.any {
        query.clauses.iter().any(evaluate)
    } else {
        query.clauses.iter().all(evaluate)
    }
}

fn snippet(body: &str, query: &MatchQuery) -> String {
    let tokens: Vec<&str> = body.split_whitespace().collect();
    let Some(index) = matching_token_index(&tokens, query) else {
        return String::new();
    };
    let start = index.saturating_sub(4);
    let end = (start + 12).min(tokens.len());
    let mut snippet = tokens.get(start..end).unwrap_or_default().join(" ");
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < tokens.len() {
        snippet.push('…');
    }
    snippet
}

fn matching_token_index(tokens: &[&str], query: &MatchQuery) -> Option<usize> {
    (0..tokens.len()).find(|&index| clause_matches_from(tokens, index, query))
}

fn clause_matches_from(tokens: &[&str], index: usize, query: &MatchQuery) -> bool {
    query.clauses.iter().any(|clause| match clause {
        MatchClause::Prefix(prefix) => tokens
            .get(index)
            .is_some_and(|token| token.to_lowercase().starts_with(prefix.as_str())),
        MatchClause::Phrase(phrase) => tokens
            .get(index..)
            .is_some_and(|rest| rest.join(" ").to_lowercase().starts_with(phrase.as_str())),
    })
}

#[cfg(test)]
mod tests {
    use margin_search::{SearchMode, build_match_for};

    use super::*;

    #[test]
    fn the_generated_match_query_parses_back_into_clauses() {
        let all = parse_match_query(&build_match_for("Alpha Beta", SearchMode::All));
        assert!(!all.any);
        assert_eq!(
            all.clauses,
            [
                MatchClause::Prefix("alpha".to_owned()),
                MatchClause::Prefix("beta".to_owned())
            ]
        );

        let any = parse_match_query(&build_match_for("Alpha Gamma", SearchMode::Any));
        assert!(any.any);
        assert_eq!(any.clauses.len(), 2);

        let phrase = parse_match_query(&build_match_for("Alpha Beta", SearchMode::Phrase));
        assert_eq!(
            phrase.clauses,
            [MatchClause::Phrase("alpha beta".to_owned())]
        );
        assert!(parse_match_query("").clauses.is_empty());
    }

    #[test]
    fn the_snippet_windows_the_body_around_the_match() {
        let body = "one two three four five six seven eight nine ten margin eleven";
        let query = parse_match_query(&build_match_for("margin", SearchMode::All));

        let window = snippet(body, &query);

        assert!(window.contains("margin"), "got {window}");
        assert!(window.contains('…'), "got {window}");
        assert!(snippet("no match here", &query).is_empty());
    }
}
