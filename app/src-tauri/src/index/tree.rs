use crate::fs::{FsEntry, WalkAction, WalkItem, path_to_string, walk_dir_capped};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

#[cfg(test)]
mod tests;

const MAX_INDEXED_PREFIX: usize = 16;

struct Entry {
    name: String,
    name_lc: String,
    rel_lc: String,
    path: String,
    is_dir: bool,
    modified: u64,
}

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, u32>,
    ids: Vec<u32>,
}

struct VaultTree {
    root: String,
    entries: Vec<Entry>,
    trie: Vec<TrieNode>,
}

// The run at offset 0 is excluded: the whole name is indexed separately and shares that prefix.
fn interior_tokens(name: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in name.char_indices() {
        if ch.is_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take()
            && s > 0
        {
            out.push(&name[s..i]);
        }
    }
    if let Some(s) = start
        && s > 0
    {
        out.push(&name[s..]);
    }
    out
}

impl VaultTree {
    fn build(root: &str) -> Self {
        let mut tree = VaultTree {
            root: root.to_string(),
            entries: Vec::new(),
            trie: vec![TrieNode::default()],
        };

        let root_prefix = format!("{}/", crate::fs::normalise_slashes(root).to_lowercase());

        walk_dir_capped(
            Path::new(root),
            0,
            crate::fs::MAX_WALK_DEPTH,
            &mut |item: &WalkItem| {
                if item.name.starts_with('.') {
                    return WalkAction::Skip;
                }
                let path = path_to_string(item.path.clone());
                let path_lc = path.to_lowercase();
                let rel_lc = path_lc
                    .strip_prefix(&root_prefix)
                    .unwrap_or(&path_lc)
                    .to_string();
                let name_lc = item.name.to_lowercase();

                let id = tree.entries.len() as u32;

                tree.insert(&name_lc, id);
                for token in interior_tokens(&name_lc) {
                    tree.insert(token, id);
                }

                tree.entries.push(Entry {
                    name: item.name.clone(),
                    name_lc,
                    rel_lc,
                    path,
                    is_dir: item.is_dir,
                    modified: item.modified,
                });

                if item.is_dir {
                    WalkAction::Recurse
                } else {
                    WalkAction::Skip
                }
            },
        );

        for node in &mut tree.trie {
            node.ids.shrink_to_fit();
        }
        tree
    }

    fn insert(&mut self, token: &str, id: u32) {
        let mut node = 0usize;
        for ch in token.chars().take(MAX_INDEXED_PREFIX) {
            node = match self.trie[node].children.get(&ch) {
                Some(&n) => n as usize,
                None => {
                    let n = self.trie.len() as u32;
                    self.trie.push(TrieNode::default());
                    self.trie[node].children.insert(ch, n);
                    n as usize
                }
            };
            let ids = &mut self.trie[node].ids;
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
    }

    fn candidates(&self, prefix: &str) -> Option<&[u32]> {
        let mut node = 0usize;
        for ch in prefix.chars().take(MAX_INDEXED_PREFIX) {
            node = *self.trie[node].children.get(&ch)? as usize;
        }
        Some(&self.trie[node].ids)
    }
}

struct Cache {
    tree: Option<VaultTree>,
    dirty: bool,
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| {
    Mutex::new(Cache {
        tree: None,
        dirty: false,
    })
});

pub fn invalidate() {
    if let Ok(mut c) = CACHE.lock() {
        c.dirty = true;
    }
}

const RANK_NAME_PREFIX: u8 = 0;
const RANK_TOKEN_PREFIX: u8 = 1;
const RANK_NAME_SUBSTRING: u8 = 2;
const RANK_PATH_SUBSTRING: u8 = 3;
const RANK_FUZZY: u8 = 4;

struct Scored {
    rank: u8,
    score: u32,
    id: u32,
}

fn rank_of(entry: &Entry, q: &str, token_prefix: bool) -> u8 {
    if entry.name_lc.starts_with(q) {
        RANK_NAME_PREFIX
    } else if token_prefix {
        RANK_TOKEN_PREFIX
    } else if entry.name_lc.contains(q) {
        RANK_NAME_SUBSTRING
    } else if entry.rel_lc.contains(q) {
        RANK_PATH_SUBSTRING
    } else {
        RANK_FUZZY
    }
}

pub fn search(root: &str, query: &str, limit: usize) -> Vec<FsEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut guard = match CACHE.lock() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };

    let stale = guard.dirty || !matches!(guard.tree.as_ref(), Some(t) if t.root == root);
    if stale {
        guard.tree = Some(VaultTree::build(root));
        guard.dirty = false;
    }
    let Some(tree) = guard.tree.as_ref() else {
        return Vec::new();
    };

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&q, CaseMatching::Ignore, Normalization::Smart);
    let mut buf: Vec<char> = Vec::new();
    let mut scored: Vec<Scored> = Vec::new();

    let q_prefix: String = q.chars().take_while(|c| c.is_alphanumeric()).collect();
    let query_fully_consumed = q_prefix.len() == q.len();
    let mut satisfied = false;

    if !q_prefix.is_empty()
        && let Some(ids) = tree.candidates(&q_prefix)
    {
        for &id in ids {
            let entry = &tree.entries[id as usize];
            if entry.is_dir {
                continue;
            }
            let rank = rank_of(entry, &q, query_fully_consumed);
            buf.clear();
            let score = pattern.score(Utf32Str::new(&entry.name_lc, &mut buf), &mut matcher);
            if rank == RANK_FUZZY && score.is_none() {
                continue;
            }
            scored.push(Scored {
                rank,
                score: score.unwrap_or(0),
                id,
            });
        }
        // The trie is trusted only when it filled the page: `proj` must still find `myproject.md`, whose single token starts `my`.
        satisfied = scored.len() >= limit;
    }

    if !satisfied {
        scored.clear();
        for (i, entry) in tree.entries.iter().enumerate() {
            if entry.is_dir {
                continue;
            }
            let rank = rank_of(entry, &q, false);
            buf.clear();
            let score = pattern.score(Utf32Str::new(&entry.name_lc, &mut buf), &mut matcher);
            if rank == RANK_FUZZY && score.is_none() {
                continue;
            }
            scored.push(Scored {
                rank,
                score: score.unwrap_or(0),
                id: i as u32,
            });
        }
    }

    scored.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then(b.score.cmp(&a.score))
            .then_with(|| {
                tree.entries[a.id as usize]
                    .name_lc
                    .cmp(&tree.entries[b.id as usize].name_lc)
            })
    });
    scored.truncate(limit);

    scored
        .into_iter()
        .map(|s| {
            let e = &tree.entries[s.id as usize];
            FsEntry {
                name: e.name.clone(),
                is_dir: false,
                path: e.path.clone(),
                modified: e.modified,
            }
        })
        .collect()
}
