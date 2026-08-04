//! Derived full-text search index.
//!
//! Markdown files in the vault remain the source of truth; this module keeps a
//! disposable SQLite FTS5 index at `.margin/index.sqlite` so vault-wide content
//! search is a ranked index lookup instead of re-reading every `.md` on each
//! query (the old `fs::search_file_contents` linear scan).
//!
//! Freshness:
//! - `set_vault_directory` spawns a background `rebuild` when a vault opens.
//! - The frontend calls `index_rebuild` on `vault-fs-changed` (covers in-app
//!   saves, sync, git, and external edits).
//!
//! `rebuild` skips files whose (mtime, size) are unchanged, so a rebuild after a
//! single save only re-reads that one file.

pub mod tree;

use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One ranked search result (a whole note, best matches first).
#[derive(Serialize, specta::Type)]
pub struct SearchHit {
    pub path: String,
    pub name: String,
    /// Plain-text excerpt around the best match, for display (highlighted on
    /// the frontend). Empty for filename-only matches.
    pub snippet: String,
}

fn db_path(root: &str) -> std::path::PathBuf {
    Path::new(root).join(".margin").join("index.sqlite")
}

/// Open (creating if needed) the per-vault index database, configure it for
/// concurrent access, and ensure the schema exists.
fn open_db(root: &str) -> Result<Connection, String> {
    let path = db_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create .margin: {e}"))?;
    }
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open index db: {e}"))?;
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS notes (
             path  TEXT PRIMARY KEY,
             name  TEXT NOT NULL,
             mtime INTEGER NOT NULL,
             size  INTEGER NOT NULL
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5 (
             path UNINDEXED,
             name,
             body,
             tokenize = 'unicode61 remove_diacritics 2'
         );",
    )
    .map_err(|e| format!("Failed to init index schema: {e}"))?;
    Ok(conn)
}

/// File modification time as integer nanoseconds since the epoch. Nanosecond
/// resolution avoids the same-second collision a seconds-only mtime would have
/// when a note is edited twice within one second.
fn mtime_nanos(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Insert or replace one note's metadata + FTS row.
fn upsert(
    conn: &Connection,
    path: &str,
    name: &str,
    body: &str,
    mtime: i64,
    size: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO notes (path, name, mtime, size) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(path) DO UPDATE SET name = ?2, mtime = ?3, size = ?4",
        params![path, name, mtime, size],
    )?;
    // notes_fts has no rowid we track, so replace the row by path.
    conn.execute("DELETE FROM notes_fts WHERE path = ?1", params![path])?;
    conn.execute(
        "INSERT INTO notes_fts (path, name, body) VALUES (?1, ?2, ?3)",
        params![path, name, body],
    )?;
    Ok(())
}

/// The string a path is stored under in the index, or `None` if it does not
/// belong there.
///
/// The mutating fs commands hand us the path returned by `ensure_in_vault`,
/// which is **canonicalized** — on Windows that is the `\\?\C:\…` verbatim form.
/// `rebuild`, by contrast, walks from the raw `root` and stores
/// `path_to_string` of that. Comparing or storing the two forms directly would
/// mean incremental updates silently miss (`starts_with` fails) or, worse, land
/// under a second spelling of the same file and leave the rebuilt row orphaned.
///
/// So: verify containment against the canonical root, then re-express the path
/// relative to the *raw* root, producing exactly the string `rebuild` uses.
/// Returns `None` for an empty root (no vault open), a non-markdown file such as
/// an attachment, or anything outside the vault — all of which are ignored
/// rather than treated as errors.
fn index_path_string(root: &str, path: &Path) -> Option<String> {
    if root.is_empty() || path.extension().and_then(|e| e.to_str()) != Some("md") {
        return None;
    }
    let canonical_root = Path::new(root).canonicalize().ok()?;
    let rel = path.strip_prefix(&canonical_root).ok()?;
    Some(crate::fs::path_to_string(Path::new(root).join(rel)))
}

/// As [`index_path_string`], but for a directory: yields the stored-path prefix
/// of everything beneath it. No markdown-extension check, since a directory
/// holds notes rather than being one.
fn index_dir_prefix(root: &str, dir: &Path) -> Option<String> {
    if root.is_empty() {
        return None;
    }
    let canonical_root = Path::new(root).canonicalize().ok()?;
    let rel = dir.strip_prefix(&canonical_root).ok()?;
    Some(format!(
        "{}/",
        crate::fs::path_to_string(Path::new(root).join(rel))
    ))
}

/// Index a single file, replacing any existing row for it.
///
/// This is what makes a save cheap. Previously every write triggered a full
/// `rebuild`, which walked the vault and stat'd every note just to discover the
/// one that had changed — O(vault) work for an O(1) edit. Best-effort: the
/// markdown on disk is the source of truth, so a failed index write degrades
/// search until the next rebuild rather than failing the save.
pub fn upsert_path(root: &str, path: &Path) {
    let Some(path_str) = index_path_string(root, path) else {
        return;
    };
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    let Ok(body) = std::fs::read_to_string(path) else {
        return;
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    if let Ok(conn) = open_db(root) {
        let _ = upsert(
            &conn,
            &path_str,
            &name,
            &body,
            mtime_nanos(&meta),
            meta.len() as i64,
        );
    }
}

/// Drop a single file from the index. Best-effort, like [`upsert_path`].
pub fn remove_path(root: &str, path: &Path) {
    let Some(path_str) = index_path_string(root, path) else {
        return;
    };
    if let Ok(conn) = open_db(root) {
        let _ = conn.execute("DELETE FROM notes WHERE path = ?1", params![path_str]);
        let _ = conn.execute("DELETE FROM notes_fts WHERE path = ?1", params![path_str]);
    }
}

/// Drop every indexed file under `dir` — used when a folder is deleted or
/// renamed, so the index does not keep serving hits for paths that are gone.
pub fn remove_prefix(root: &str, dir: &Path) {
    // The trailing slash confines the sweep to the directory's contents, so a
    // sibling sharing its name as a prefix ("notes" vs "notes-archive") is safe.
    let Some(prefix) = index_dir_prefix(root, dir) else {
        return;
    };
    // `%` and `_` are LIKE wildcards and `_` in particular is common in real
    // filenames — unescaped, deleting `notes_old/` would also sweep `notesXold/`.
    // Escape the escape character first so it cannot double-escape the others.
    let escaped = prefix
        .replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_");
    let pattern = format!("{escaped}%");
    if let Ok(conn) = open_db(root) {
        let _ = conn.execute(
            r"DELETE FROM notes WHERE path LIKE ?1 ESCAPE '\'",
            params![pattern],
        );
        let _ = conn.execute(
            r"DELETE FROM notes_fts WHERE path LIKE ?1 ESCAPE '\'",
            params![pattern],
        );
    }
}

/// Rebuild the index from the markdown files under `root`, skipping files whose
/// (mtime, size) match what is already indexed and dropping rows for files that
/// no longer exist. Returns the number of notes present in the index.
pub fn rebuild(root: &str) -> Result<u32, String> {
    let mut md_paths = Vec::new();
    crate::fs::collect_md_paths(Path::new(root), &mut md_paths, 0, crate::fs::MAX_WALK_DEPTH);

    let mut conn = open_db(root)?;

    // Snapshot what is already indexed so unchanged files can be skipped.
    let mut existing: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT path, mtime, size FROM notes")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            existing.insert(row.0, (row.1, row.2));
        }
    }

    let mut present: HashSet<String> = HashSet::with_capacity(md_paths.len());
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for path in &md_paths {
        let path_str = crate::fs::path_to_string(path.clone());
        present.insert(path_str.clone());

        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = mtime_nanos(&meta);
        let size = meta.len() as i64;

        if let Some(&(emt, esz)) = existing.get(&path_str) {
            if emt == mtime && esz == size {
                continue; // unchanged — leave the existing FTS row in place
            }
        }

        let body = std::fs::read_to_string(path).unwrap_or_default();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        upsert(&tx, &path_str, &name, &body, mtime, size).map_err(|e| e.to_string())?;
    }

    // Drop rows for files that have disappeared since the last build.
    for path_str in existing.keys() {
        if !present.contains(path_str) {
            tx.execute("DELETE FROM notes WHERE path = ?1", params![path_str])
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![path_str])
                .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(present.len() as u32)
}

/// Turn raw user input into a safe FTS5 MATCH expression: each whitespace token
/// becomes a quoted prefix term (`"foo"*`), ANDed together. Quoting makes tokens
/// that collide with FTS keywords (AND/OR/NOT/NEAR) literal, and filtering to
/// alphanumerics keeps the parser away from FTS operator characters. Returns an
/// empty string when the query has no usable tokens.
fn build_match_query(input: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for raw in input.split_whitespace() {
        let token: String = raw
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if token.is_empty() {
            continue;
        }
        parts.push(format!("\"{token}\"*"));
    }
    parts.join(" ")
}

/// Ranked full-text search over indexed note bodies and names. Returns up to
/// `limit` hits ordered by FTS5 relevance (bm25).
#[tauri::command]
#[specta::specta]
pub fn index_search(root: &str, query: &str, limit: u32) -> Result<Vec<SearchHit>, String> {
    let match_query = build_match_query(query);
    if match_query.is_empty() {
        return Ok(vec![]);
    }

    let conn = open_db(root)?;
    // Explicit bm25 column weights (path, name, body). A term in the note's
    // *name* is a far stronger signal of intent than the same term buried in the
    // body, so the name column is weighted 10×. `path` is UNINDEXED and can
    // never contribute, but bm25 still expects a weight per column.
    // SQLite's bm25 is negative-better, and `ORDER BY rank` sorts ascending, so
    // a heavier weight pulls those rows to the top.
    let mut stmt = conn
        .prepare(
            "SELECT path, name, snippet(notes_fts, -1, '', '', '…', 12)
             FROM notes_fts
             WHERE notes_fts MATCH ?1
             ORDER BY bm25(notes_fts, 0.0, 10.0, 1.0)
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![match_query, limit], |r| {
            Ok(SearchHit {
                path: r.get(0)?,
                name: r.get(1)?,
                snippet: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// Rebuild the index (skipping unchanged files) and return the indexed count.
/// Called by the frontend on vault open and on `vault-fs-changed`.
#[tauri::command]
#[specta::specta]
pub fn index_rebuild(root: &str) -> Result<u32, String> {
    rebuild(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp vault whose root is a *raw*, non-canonical path — the shape the
    /// frontend actually sends — so these tests exercise the same mismatch that
    /// silently no-op'd the incremental updates before `index_path_string`.
    fn temp_vault() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-index-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("notes_old")).unwrap();
        dir
    }

    #[test]
    fn stored_path_matches_the_rebuild_spelling() {
        let root = temp_vault();
        let root_str = crate::fs::path_to_string(root.clone());
        let note = root.join("notes_old").join("todo.md");
        std::fs::write(&note, "x").unwrap();

        // What the fs commands hand us: the canonicalized form.
        let canonical = note.canonicalize().unwrap();
        let stored = index_path_string(&root_str, &canonical).expect("should be indexable");

        // What `rebuild`'s walk stores for the very same file.
        let expected = crate::fs::path_to_string(note.clone());
        assert_eq!(stored, expected);
        // And on Windows specifically, the verbatim prefix must not leak through.
        assert!(!stored.contains("?"), "verbatim prefix leaked: {stored}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn non_markdown_and_outside_paths_are_ignored() {
        let root = temp_vault();
        let root_str = crate::fs::path_to_string(root.clone());

        let attachment = root.join("image.png");
        std::fs::write(&attachment, "x").unwrap();
        assert_eq!(
            index_path_string(&root_str, &attachment.canonicalize().unwrap()),
            None
        );

        // A path outside the vault must never resolve to a stored path.
        let outside = std::env::temp_dir().join("elsewhere.md");
        assert_eq!(index_path_string(&root_str, &outside), None);

        // No vault open.
        assert_eq!(index_path_string("", &attachment), None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn dir_prefix_is_anchored_with_a_trailing_slash() {
        let root = temp_vault();
        let root_str = crate::fs::path_to_string(root.clone());
        let dir = root.join("notes_old").canonicalize().unwrap();

        let prefix = index_dir_prefix(&root_str, &dir).expect("should resolve");
        assert!(prefix.ends_with("notes_old/"), "got {prefix}");
        // The anchor is what keeps a sibling like `notes_older` out of the sweep.
        assert!(!format!("{}notes_older/x.md", &prefix[..prefix.len() - 10]).starts_with(&prefix));

        std::fs::remove_dir_all(&root).ok();
    }
}
