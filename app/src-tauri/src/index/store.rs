use rusqlite::{Connection, params};
use std::path::Path;

const SCHEMA_VERSION: i64 = 3;

fn db_path(root: &str) -> std::path::PathBuf {
    Path::new(root).join(".margin").join("index.sqlite")
}

pub(super) fn open_db(root: &str) -> Result<Connection, String> {
    let path = db_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create .margin: {e}"))?;
    }
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open index db: {e}"))?;
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )
    .map_err(|e| format!("Failed to configure index db: {e}"))?;

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if version < SCHEMA_VERSION {
        conn.execute_batch(
            "DROP TABLE IF EXISTS notes;
             DROP TABLE IF EXISTS notes_fts;
             DROP TABLE IF EXISTS tags;
             DROP TABLE IF EXISTS links;",
        )
        .map_err(|e| format!("Failed to reset index: {e}"))?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|e| format!("Failed to stamp index schema: {e}"))?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
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
         );
         CREATE TABLE IF NOT EXISTS tags (
             path TEXT NOT NULL,
             tag  TEXT NOT NULL,
             PRIMARY KEY (path, tag)
         );
         CREATE INDEX IF NOT EXISTS tags_tag ON tags (tag);
         CREATE TABLE IF NOT EXISTS links (
             src       TEXT NOT NULL,
             target_lc TEXT NOT NULL,
             PRIMARY KEY (src, target_lc)
         );
         CREATE INDEX IF NOT EXISTS links_target_lc ON links (target_lc);",
    )
    .map_err(|e| format!("Failed to init index schema: {e}"))?;

    Ok(conn)
}

pub(super) fn mtime_nanos(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

pub(super) fn upsert(
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
    conn.execute("DELETE FROM notes_fts WHERE path = ?1", params![path])?;
    conn.execute(
        "INSERT INTO notes_fts (path, name, body) VALUES (?1, ?2, ?3)",
        params![path, name, body],
    )?;

    conn.execute("DELETE FROM tags WHERE path = ?1", params![path])?;
    for tag in crate::fs::tags::extract_tags_from_content(body) {
        conn.execute(
            "INSERT OR IGNORE INTO tags (path, tag) VALUES (?1, ?2)",
            params![path, tag],
        )?;
    }

    conn.execute("DELETE FROM links WHERE src = ?1", params![path])?;
    for link in crate::text::parse_wiki_links(body) {
        conn.execute(
            "INSERT OR IGNORE INTO links (src, target_lc) VALUES (?1, ?2)",
            params![path, link.title.to_lowercase()],
        )?;
    }
    Ok(())
}

pub(super) fn delete_note_rows(conn: &Connection, path: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM notes WHERE path = ?1", params![path])?;
    conn.execute("DELETE FROM notes_fts WHERE path = ?1", params![path])?;
    conn.execute("DELETE FROM tags WHERE path = ?1", params![path])?;
    conn.execute("DELETE FROM links WHERE src = ?1", params![path])?;
    Ok(())
}

// fs commands hand us canonicalized paths while `rebuild` stores raw-root-relative ones, so a path is re-expressed before it is compared or written.
pub(super) fn index_path_string(root: &str, path: &Path) -> Option<String> {
    if root.is_empty() || path.extension().and_then(|e| e.to_str()) != Some("md") {
        return None;
    }
    let canonical_root = Path::new(root).canonicalize().ok()?;
    let rel = path.strip_prefix(&canonical_root).ok()?;
    Some(crate::fs::path_to_string(Path::new(root).join(rel)))
}

// The trailing slash anchors the sweep inside the directory, so `notes` cannot take `notes-archive` with it.
pub(super) fn index_dir_prefix(root: &str, dir: &Path) -> Option<String> {
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
