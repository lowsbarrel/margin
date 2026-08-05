//! In-memory vault path index — the tree behind instant filename search.
//!
//! Before this module, `fs::search_files` walked the vault from disk on *every*
//! keystroke: a recursive `read_dir` plus a `metadata()` stat per entry. On a
//! large vault that is tens of milliseconds of syscalls per character typed, and
//! the cost scales with vault size rather than query length.
//!
//! Instead the vault is walked **once** into a flat arena of entries, and a
//! prefix trie is built over the tokens of every entry name. A query then costs
//! `O(len(query))` to walk the trie down to a candidate list, plus scoring of
//! just those candidates.
//!
//! Only when the trie cannot satisfy the query — a subsequence query like `pjt`
//! for `project`, or a prefix too rare to fill the result limit — does it fall
//! back to a full pass over the arena. Even that fallback is pure memory work
//! (well under a millisecond for 10k entries) rather than a directory walk.
//!
//! Freshness: the cache carries a dirty flag set by [`invalidate`], which the
//! vault watcher calls on every debounced `vault-fs-changed` burst and which the
//! mutating fs commands call directly. A dirty cache is rebuilt lazily on the
//! next query, so a burst of N keystrokes between two edits costs one walk, not
//! N.

use crate::fs::{path_to_string, walk_dir, FsEntry, WalkAction, WalkItem};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Directory nesting the vault walk descends into. Mirrors `fs::MAX_WALK_DEPTH`.
const MAX_DEPTH: usize = 10;

/// How many leading characters of each token are indexed. Longer queries still
/// work — the trie is walked as far as it goes and the rest is verified during
/// scoring — but the structure stops growing, bounding memory at
/// `O(total token length)`.
const MAX_INDEXED_PREFIX: usize = 16;

/// One indexed vault entry. The lowercased forms are precomputed at build time
/// so the query path never allocates or re-lowercases.
struct Entry {
    name: String,
    name_lc: String,
    /// Vault-relative path, lowercased — lets `notes/todo` match by folder.
    rel_lc: String,
    path: String,
    is_dir: bool,
    modified: u64,
}

/// A trie node. `ids` lists every entry whose name has a *token* starting with
/// the prefix spelled by the root-to-here path.
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

/// Maximal alphanumeric runs within `name`, excluding the run at offset 0 (the
/// whole name is indexed separately, and its first token shares that prefix).
fn interior_tokens(name: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in name.char_indices() {
        if ch.is_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            if s > 0 {
                out.push(&name[s..i]);
            }
        }
    }
    if let Some(s) = start {
        if s > 0 {
            out.push(&name[s..]);
        }
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

        // `path_to_string` normalises separators, so the prefix must too before
        // it can be stripped to yield a vault-relative path.
        let root_prefix = format!("{}/", root.replace('\\', "/").to_lowercase());

        collect(Path::new(root), 0, &mut |item: &WalkItem| {
            let path = path_to_string(item.path.clone());
            let path_lc = path.to_lowercase();
            let rel_lc = path_lc
                .strip_prefix(&root_prefix)
                .unwrap_or(&path_lc)
                .to_string();
            let name_lc = item.name.to_lowercase();

            let id = tree.entries.len() as u32;

            // Index before moving `name_lc` into the arena: the whole name, so
            // `my-project.md` is reachable by typing it verbatim, plus each
            // interior token so `project` and `md` reach it too.
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
        });

        for node in &mut tree.trie {
            node.ids.shrink_to_fit();
        }
        tree
    }

    fn insert(&mut self, token: &str, id: u32) {
        let mut node = 0usize;
        // The root node deliberately holds no ids: an empty query never reaches
        // a lookup, and a root-wide list would just duplicate the arena.
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
            // Entries are indexed one at a time and ids only ever increase, so
            // checking the tail is enough to keep each list duplicate-free —
            // which matters for names whose tokens share a prefix ("read-readme").
            let ids = &mut self.trie[node].ids;
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
    }

    /// Entry ids whose name has a token starting with `prefix`, or `None` when
    /// the prefix does not occur in the vault at all.
    fn candidates(&self, prefix: &str) -> Option<&[u32]> {
        let mut node = 0usize;
        for ch in prefix.chars().take(MAX_INDEXED_PREFIX) {
            node = *self.trie[node].children.get(&ch)? as usize;
        }
        Some(&self.trie[node].ids)
    }
}

/// Recursive vault walk. Hidden entries (`.git`, `.margin`, …) are skipped, and
/// symlinks are never followed — both inherited from the shared `walk_dir`.
fn collect<F: FnMut(&WalkItem)>(dir: &Path, depth: usize, visit: &mut F) {
    if depth >= MAX_DEPTH {
        return;
    }
    let mut child_dirs: Vec<std::path::PathBuf> = Vec::new();
    walk_dir(dir, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        visit(item);
        if item.is_dir {
            child_dirs.push(item.path.clone());
        }
        WalkAction::Skip
    });
    for child in child_dirs {
        collect(&child, depth + 1, visit);
    }
}

struct Cache {
    tree: Option<VaultTree>,
    dirty: bool,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            tree: None,
            dirty: false,
        })
    })
}

/// Mark the cached tree stale. Cheap enough to call from the watcher's hot path
/// and from every mutating fs command.
pub fn invalidate() {
    if let Ok(mut c) = cache().lock() {
        c.dirty = true;
    }
}

/// Match classes, best first. The class dominates the fuzzy score, so a literal
/// prefix match always outranks a scattered subsequence hit.
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

/// Classify how well `entry` matches `q`. `token_prefix` is true when the
/// candidate came from a trie hit that consumed the entire query, which is a
/// stronger signal than a bare substring.
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

/// Ranked filename search over the cached tree, best first. Directories are
/// indexed so folder names can match a path query, but are never returned —
/// callers open files.
pub fn search(root: &str, query: &str, limit: usize) -> Vec<FsEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut guard = match cache().lock() {
        Ok(g) => g,
        // A poisoned lock means an earlier build panicked. Returning empty
        // degrades search; it must not take the whole command down.
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

    // ── Fast path ──
    // The query's leading alphanumeric run is a token prefix, so the trie hands
    // back the candidate set without the arena being touched at all.
    let q_prefix: String = q.chars().take_while(|c| c.is_alphanumeric()).collect();
    let query_fully_consumed = q_prefix.len() == q.len();
    let mut satisfied = false;

    if !q_prefix.is_empty() {
        if let Some(ids) = tree.candidates(&q_prefix) {
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
            // Trust the trie only when it filled the page. Otherwise `proj`
            // would silently miss `myproject.md`, whose one token starts `my`.
            satisfied = scored.len() >= limit;
        }
    }

    // ── Fallback ──
    // Full pass over the arena: still pure memory work, no syscalls or stats.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_from(names: &[&str]) -> VaultTree {
        let mut tree = VaultTree {
            root: "/vault".into(),
            entries: Vec::new(),
            trie: vec![TrieNode::default()],
        };
        for name in names {
            let name_lc = name.to_lowercase();
            let id = tree.entries.len() as u32;
            tree.insert(&name_lc, id);
            for token in interior_tokens(&name_lc) {
                tree.insert(token, id);
            }
            tree.entries.push(Entry {
                name: (*name).into(),
                name_lc,
                rel_lc: name.to_lowercase(),
                path: format!("/vault/{name}"),
                is_dir: false,
                modified: 0,
            });
        }
        tree
    }

    #[test]
    fn interior_tokens_skips_the_leading_run() {
        // The run at offset 0 is covered by indexing the whole name.
        assert_eq!(interior_tokens("my-project.md"), vec!["project", "md"]);
        assert_eq!(interior_tokens("readme"), Vec::<&str>::new());
    }

    #[test]
    fn trie_finds_whole_name_and_interior_tokens() {
        let tree = tree_from(&["my-project.md", "readme.md"]);
        assert_eq!(tree.candidates("my"), Some(&[0u32][..]));
        assert_eq!(tree.candidates("project"), Some(&[0u32][..]));
        // Both files have an `md` token.
        assert_eq!(tree.candidates("md"), Some(&[0u32, 1][..]));
        assert_eq!(tree.candidates("zzz"), None);
    }

    #[test]
    fn shared_token_prefixes_are_not_duplicated() {
        // "read" prefixes both the whole name and the interior token "readme".
        let tree = tree_from(&["read-readme.md"]);
        assert_eq!(tree.candidates("read"), Some(&[0u32][..]));
    }

    #[test]
    fn ranks_prefer_literal_prefixes_over_fuzzy() {
        let tree = tree_from(&["project.md"]);
        let e = &tree.entries[0];
        assert_eq!(rank_of(e, "pro", true), RANK_NAME_PREFIX);
        assert_eq!(rank_of(e, "ject", false), RANK_NAME_SUBSTRING);
        assert_eq!(rank_of(e, "pjt", false), RANK_FUZZY);
    }
}
