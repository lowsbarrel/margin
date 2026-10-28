use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use margin_kernel::{EntryKind, FileSystem, RelativePath, UnixMillis};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub const MAX_WALK_DEPTH: usize = 10;

const MAX_INDEXED_PREFIX: usize = 16;
const RANK_NAME_PREFIX: u8 = 0;
const RANK_TOKEN_PREFIX: u8 = 1;
const RANK_NAME_SUBSTRING: u8 = 2;
const RANK_PATH_SUBSTRING: u8 = 3;
const RANK_FUZZY: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Date,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: RelativePath,
    pub modified: UnixMillis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub name: String,
    pub path: RelativePath,
    pub is_dir: bool,
    pub modified: UnixMillis,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkAction {
    Recurse,
    Skip,
}

#[derive(Clone, Debug)]
pub struct WalkItem {
    pub name: String,
    pub path: RelativePath,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub modified: UnixMillis,
}

#[must_use]
pub(crate) fn join(directory: &RelativePath, relative: &str) -> RelativePath {
    directory.join(relative).expect("a relative path joins")
}

pub fn walk(
    files: &dyn FileSystem,
    directory: &RelativePath,
    visit: &mut impl FnMut(&WalkItem) -> WalkAction,
) {
    let Ok(entries) = files.list(directory) else {
        return;
    };
    for entry in entries {
        let is_dir = entry.metadata.kind == EntryKind::Directory;
        let item = WalkItem {
            path: join(directory, &entry.name),
            name: entry.name,
            is_dir,
            is_symlink: entry.metadata.kind == EntryKind::Symlink,
            modified: entry.metadata.modified,
        };
        let action = visit(&item);
        if is_dir && matches!(action, WalkAction::Recurse) {
            walk(files, &item.path, visit);
        }
    }
}

#[must_use]
pub(crate) fn listing(files: &dyn FileSystem, directory: &RelativePath) -> Vec<FsEntry> {
    let mut items: Vec<WalkItem> = Vec::new();
    walk(files, directory, &mut |item| {
        if !item.name.starts_with('.') {
            items.push(item.clone());
        }
        WalkAction::Skip
    });
    sort_items(&mut items, SortBy::Name);
    items
        .into_iter()
        .map(|item| FsEntry {
            name: item.name,
            is_dir: item.is_dir,
            path: item.path,
            modified: item.modified,
        })
        .collect()
}

pub(crate) fn visible_tree(
    files: &dyn FileSystem,
    directory: &RelativePath,
    depth: usize,
    expanded: &HashSet<RelativePath>,
    sort_by: SortBy,
    hidden: &HashSet<RelativePath>,
    rows: &mut Vec<TreeRow>,
) {
    let mut items: Vec<WalkItem> = Vec::new();
    walk(files, directory, &mut |item| {
        if !item.name.starts_with('.') {
            items.push(item.clone());
        }
        WalkAction::Skip
    });
    sort_items(&mut items, sort_by);
    for item in items {
        if hidden.contains(&item.path) {
            continue;
        }
        rows.push(TreeRow {
            name: item.name,
            path: item.path.clone(),
            is_dir: item.is_dir,
            modified: item.modified,
            depth,
        });
        if item.is_dir && expanded.contains(&item.path) {
            visible_tree(
                files,
                &item.path,
                depth + 1,
                expanded,
                sort_by,
                hidden,
                rows,
            );
        }
    }
}

fn sort_items(items: &mut [WalkItem], sort_by: SortBy) {
    match sort_by {
        SortBy::Date => items.sort_unstable_by(|left, right| match (left.is_dir, right.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            (false, false) => right.modified.cmp(&left.modified),
        }),
        SortBy::Name => items.sort_unstable_by(|left, right| match (left.is_dir, right.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        }),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SearchEntry {
    name: String,
    name_lc: String,
    rel_lc: String,
    path: RelativePath,
    modified: UnixMillis,
}

struct Scored {
    rank: u8,
    score: u32,
    id: u32,
}

#[derive(Debug)]
pub(crate) struct VaultIndex {
    entries: Vec<SearchEntry>,
    prefixes: HashMap<String, Vec<u32>>,
}

impl VaultIndex {
    pub(crate) fn build(files: &dyn FileSystem, root: &RelativePath) -> Self {
        let mut index = Self {
            entries: Vec::new(),
            prefixes: HashMap::new(),
        };
        let mut level: Vec<RelativePath> = vec![root.clone()];
        for _ in 0..MAX_WALK_DEPTH {
            let mut children: Vec<RelativePath> = Vec::new();
            for directory in &level {
                walk(files, directory, &mut |item| {
                    if item.name.starts_with('.') {
                        return WalkAction::Skip;
                    }
                    if item.is_dir {
                        children.push(item.path.clone());
                        return WalkAction::Skip;
                    }
                    index.push(item);
                    WalkAction::Skip
                });
            }
            level = children;
        }
        index
    }

    fn push(&mut self, item: &WalkItem) {
        let id = u32::try_from(self.entries.len())
            .expect("a vault holds fewer than four billion entries");
        let name_lc = item.name.to_lowercase();
        self.insert(&name_lc, id);
        for token in interior_tokens(&name_lc) {
            self.insert(token, id);
        }
        self.entries.push(SearchEntry {
            name: item.name.clone(),
            rel_lc: item.path.as_str().to_lowercase(),
            path: item.path.clone(),
            name_lc,
            modified: item.modified,
        });
    }

    fn insert(&mut self, token: &str, id: u32) {
        let characters: Vec<char> = token.chars().take(MAX_INDEXED_PREFIX).collect();
        for length in 1..=characters.len() {
            let prefix: String = characters[..length].iter().collect();
            let ids = self.prefixes.entry(prefix).or_default();
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
    }

    fn candidates(&self, prefix: &str) -> Option<&[u32]> {
        let prefix: String = prefix.chars().take(MAX_INDEXED_PREFIX).collect();
        self.prefixes.get(&prefix).map(Vec::as_slice)
    }

    fn entry(&self, id: u32) -> &SearchEntry {
        let index = usize::try_from(id).expect("an index id fits in a usize");
        &self.entries[index]
    }

    #[must_use]
    pub(crate) fn search(&self, query: &str, limit: usize) -> Vec<FsEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let leading: String = query
            .chars()
            .take_while(|character| character.is_alphanumeric())
            .collect();
        let mut scorer = Scorer {
            query: &query,
            matcher: Matcher::new(Config::DEFAULT),
            pattern: Pattern::parse(&query, CaseMatching::Ignore, Normalization::Smart),
            buffer: Vec::new(),
        };
        let mut ranked: Vec<Scored> = Vec::new();
        let candidates = self.candidates(&leading).unwrap_or_default();
        let token_prefix = leading.len() == query.len();
        for &id in candidates {
            let entry = self.entry(id);
            if let Some(found) = scorer.rank(entry, id, token_prefix) {
                ranked.push(found);
            }
        }
        if candidates.is_empty() || ranked.len() < limit {
            ranked.clear();
            for (index, entry) in self.entries.iter().enumerate() {
                let id =
                    u32::try_from(index).expect("a vault holds fewer than four billion entries");
                if let Some(found) = scorer.rank(entry, id, false) {
                    ranked.push(found);
                }
            }
        }
        ranked.sort_by(|left, right| {
            left.rank
                .cmp(&right.rank)
                .then(right.score.cmp(&left.score))
                .then_with(|| {
                    self.entry(left.id)
                        .name_lc
                        .cmp(&self.entry(right.id).name_lc)
                })
        });
        ranked.truncate(limit);
        ranked
            .into_iter()
            .map(|found| {
                let entry = self.entry(found.id);
                FsEntry {
                    name: entry.name.clone(),
                    is_dir: false,
                    path: entry.path.clone(),
                    modified: entry.modified,
                }
            })
            .collect()
    }
}

struct Scorer<'query> {
    query: &'query str,
    matcher: Matcher,
    pattern: Pattern,
    buffer: Vec<char>,
}

impl Scorer<'_> {
    fn rank(&mut self, entry: &SearchEntry, id: u32, token_prefix: bool) -> Option<Scored> {
        let rank = rank_of(entry, self.query, token_prefix);
        self.buffer.clear();
        let matched = self.pattern.score(
            Utf32Str::new(&entry.name_lc, &mut self.buffer),
            &mut self.matcher,
        );
        if rank == RANK_FUZZY && matched.is_none() {
            return None;
        }
        Some(Scored {
            rank,
            score: matched.unwrap_or(0),
            id,
        })
    }
}

fn rank_of(entry: &SearchEntry, query: &str, token_prefix: bool) -> u8 {
    if entry.name_lc.starts_with(query) {
        RANK_NAME_PREFIX
    } else if token_prefix {
        RANK_TOKEN_PREFIX
    } else if entry.name_lc.contains(query) {
        RANK_NAME_SUBSTRING
    } else if entry.rel_lc.contains(query) {
        RANK_PATH_SUBSTRING
    } else {
        RANK_FUZZY
    }
}

fn interior_tokens(name: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (index, character) in name.char_indices() {
        if character.is_alphanumeric() {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(found) = start.take()
            && found > 0
        {
            tokens.push(&name[found..index]);
        }
    }
    if let Some(found) = start
        && found > 0
    {
        tokens.push(&name[found..]);
    }
    tokens
}

pub(crate) fn replace_in_text(
    content: &str,
    search: &str,
    replace: &str,
    case_sensitive: bool,
) -> (String, usize) {
    if search.is_empty() {
        return (content.to_owned(), 0);
    }
    if case_sensitive {
        let count = content.matches(search).count();
        return (content.replace(search, replace), count);
    }
    let mut result = String::with_capacity(content.len());
    let lowercase_search = search.to_ascii_lowercase();
    let lowercase_content = content.to_ascii_lowercase();
    let mut last_end = 0;
    let mut count = 0_usize;
    while let Some(found) = lowercase_content[last_end..].find(&lowercase_search) {
        let start = last_end + found;
        result.push_str(&content[last_end..start]);
        result.push_str(replace);
        last_end = start + search.len();
        count += 1;
    }
    result.push_str(&content[last_end..]);
    (result, count)
}
