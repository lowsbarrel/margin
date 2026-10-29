use std::path::PathBuf;
use std::{fmt, fs};

use margin_kernel::{RelativePath, StoreError, UnixMillis};
use margin_search::{
    Backlink, IndexedNote, IndexedNoteMeta, SearchHit, SearchIndex, TagInfo, backlink_target,
    extract_tags, parse_wiki_links,
};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, Params, params};

const SCHEMA_VERSION: i64 = 4;

const PRAGMAS: &str = "PRAGMA busy_timeout = 5000;
     PRAGMA journal_mode = WAL;
     PRAGMA synchronous = NORMAL;
     PRAGMA cache_size = -16384;";

const DROP_TABLES: &str = "DROP TABLE IF EXISTS notes;
     DROP TABLE IF EXISTS notes_fts;
     DROP TABLE IF EXISTS tags;
     DROP TABLE IF EXISTS links;";

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS notes (
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
     CREATE INDEX IF NOT EXISTS links_target_lc ON links (target_lc);";

pub struct SqliteIndex {
    connection: Mutex<Connection>,
}

impl SqliteIndex {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the composition root hands the adapter an owned vault index path"
    )]
    pub fn open(db_path: PathBuf) -> Result<Self, StoreError> {
        if let Some(parent) = db_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| {
                StoreError::Other(format!("failed to create the index directory: {error}"))
            })?;
        }
        let connection = Connection::open(&db_path).map_err(db_error)?;
        connection.execute_batch(PRAGMAS).map_err(db_error)?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);
        if version < SCHEMA_VERSION {
            connection.execute_batch(DROP_TABLES).map_err(db_error)?;
            connection
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(db_error)?;
        }
        connection.execute_batch(SCHEMA).map_err(db_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

impl SearchIndex for SqliteIndex {
    fn upsert(&self, notes: &[IndexedNote]) -> Result<(), StoreError> {
        let mut guard = self.connection.lock();
        let transaction = guard.transaction().map_err(db_error)?;
        for note in notes {
            upsert_note(
                &transaction,
                &note.path,
                &note.body,
                stored_mtime(note.modified),
                stored_size(note.size),
            )?;
        }
        transaction.commit().map_err(db_error)
    }

    fn remove(&self, path: &RelativePath) -> Result<(), StoreError> {
        let guard = self.connection.lock();
        let pattern = subtree_pattern(path);
        for statement in [
            "DELETE FROM notes_fts WHERE rowid IN (SELECT rowid FROM notes WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\')",
            "DELETE FROM notes WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\'",
            "DELETE FROM tags WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\'",
            "DELETE FROM links WHERE src = ?1 OR src LIKE ?2 ESCAPE '\\'",
        ] {
            guard
                .execute(statement, params![path.as_str(), pattern])
                .map_err(db_error)?;
        }
        Ok(())
    }

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        if from == to {
            return Ok(());
        }
        let guard = self.connection.lock();
        let existing = guard
            .query_row(
                "SELECT notes_fts.body, notes.mtime, notes.size
                 FROM notes JOIN notes_fts ON notes_fts.rowid = notes.rowid
                 WHERE notes.path = ?1",
                params![from.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(db_error)?;
        let Some((body, mtime, size)) = existing else {
            return Ok(());
        };
        delete_note_rows(&guard, from.as_str())?;
        upsert_note(&guard, to, &body, mtime, size)
    }

    fn search(&self, match_query: &str, limit: u32) -> Result<Vec<SearchHit>, StoreError> {
        if match_query.is_empty() {
            return Ok(Vec::new());
        }
        let guard = self.connection.lock();
        let mut statement = guard
            .prepare_cached(
                "SELECT path, name, snippet(notes_fts, -1, '', '', '…', 12)
                 FROM notes_fts
                 WHERE notes_fts MATCH ?1 AND rank MATCH 'bm25(0.0, 10.0, 1.0)'
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map(params![match_query, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(db_error)?;

        let mut hits = Vec::new();
        for row in rows {
            let (path, name, snippet) = row.map_err(db_error)?;
            hits.push(SearchHit {
                path: stored_path(&path)?,
                name,
                snippet,
            });
        }
        Ok(hits)
    }

    fn tags(&self) -> Result<Vec<TagInfo>, StoreError> {
        let guard = self.connection.lock();
        let mut statement = guard
            .prepare("SELECT tag, path FROM tags ORDER BY tag, path")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;

        let mut tags: Vec<TagInfo> = Vec::new();
        for row in rows {
            let (tag, path) = row.map_err(db_error)?;
            let path = stored_path(&path)?;
            match tags.last_mut() {
                Some(last) if last.tag == tag => {
                    last.count += 1;
                    last.files.push(path);
                }
                _ => tags.push(TagInfo {
                    tag,
                    count: 1,
                    files: vec![path],
                }),
            }
        }
        Ok(tags)
    }

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<Backlink>, StoreError> {
        let guard = self.connection.lock();
        let mut statement = guard
            .prepare(
                "SELECT notes.path, notes.name
                 FROM links JOIN notes ON notes.path = links.src
                 WHERE links.target_lc = ?1 AND links.src <> ?2
                 ORDER BY notes.name",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map(params![backlink_target(path), path.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;

        let mut backlinks = Vec::new();
        for row in rows {
            let (path, name) = row.map_err(db_error)?;
            backlinks.push(Backlink {
                path: stored_path(&path)?,
                name,
            });
        }
        Ok(backlinks)
    }

    fn indexed(&self) -> Result<Vec<IndexedNoteMeta>, StoreError> {
        let guard = self.connection.lock();
        let mut statement = guard
            .prepare("SELECT path, mtime, size FROM notes")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(db_error)?;

        let mut indexed = Vec::new();
        for row in rows {
            let (path, mtime, size) = row.map_err(db_error)?;
            indexed.push(IndexedNoteMeta {
                path: stored_path(&path)?,
                modified: UnixMillis::from_millis(u64::try_from(mtime).unwrap_or(0)),
                size: u64::try_from(size).unwrap_or(0),
            });
        }
        Ok(indexed)
    }

    fn clear(&self) -> Result<(), StoreError> {
        let guard = self.connection.lock();
        guard
            .execute_batch(
                "DELETE FROM notes_fts;
                 DELETE FROM notes;
                 DELETE FROM tags;
                 DELETE FROM links;",
            )
            .map_err(db_error)
    }
}

impl fmt::Debug for SqliteIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteIndex").finish_non_exhaustive()
    }
}

fn upsert_note(
    connection: &Connection,
    path: &RelativePath,
    body: &str,
    mtime: i64,
    size: i64,
) -> Result<(), StoreError> {
    let name = path.file_name();
    let rowid: i64 = connection
        .prepare_cached(
            "INSERT INTO notes (path, name, mtime, size) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET name = ?2, mtime = ?3, size = ?4
             RETURNING rowid",
        )
        .and_then(|mut statement| {
            statement.query_row(params![path.as_str(), name, mtime, size], |row| row.get(0))
        })
        .map_err(db_error)?;
    run(
        connection,
        "DELETE FROM notes_fts WHERE rowid = ?1",
        params![rowid],
    )?;
    run(
        connection,
        "INSERT INTO notes_fts (rowid, path, name, body) VALUES (?1, ?2, ?3, ?4)",
        params![rowid, path.as_str(), name, body],
    )?;
    run(
        connection,
        "DELETE FROM tags WHERE path = ?1",
        params![path.as_str()],
    )?;
    for tag in extract_tags(body) {
        run(
            connection,
            "INSERT OR IGNORE INTO tags (path, tag) VALUES (?1, ?2)",
            params![path.as_str(), tag],
        )?;
    }
    run(
        connection,
        "DELETE FROM links WHERE src = ?1",
        params![path.as_str()],
    )?;
    for title in parse_wiki_links(body) {
        run(
            connection,
            "INSERT OR IGNORE INTO links (src, target_lc) VALUES (?1, ?2)",
            params![path.as_str(), title.to_lowercase()],
        )?;
    }
    Ok(())
}

fn run(connection: &Connection, sql: &str, values: impl Params) -> Result<(), StoreError> {
    connection
        .prepare_cached(sql)
        .and_then(|mut statement| statement.execute(values))
        .map(drop)
        .map_err(db_error)
}

fn delete_note_rows(connection: &Connection, path: &str) -> Result<(), StoreError> {
    for statement in [
        "DELETE FROM notes_fts WHERE rowid IN (SELECT rowid FROM notes WHERE path = ?1)",
        "DELETE FROM notes WHERE path = ?1",
        "DELETE FROM tags WHERE path = ?1",
        "DELETE FROM links WHERE src = ?1",
    ] {
        connection
            .execute(statement, params![path])
            .map_err(db_error)?;
    }
    Ok(())
}

fn stored_path(path: &str) -> Result<RelativePath, StoreError> {
    RelativePath::new(path).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn stored_mtime(modified: UnixMillis) -> i64 {
    i64::try_from(modified.as_millis()).unwrap_or(i64::MAX)
}

fn stored_size(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

fn subtree_pattern(path: &RelativePath) -> String {
    let prefix = format!("{path}/");
    let escaped = prefix
        .replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_");
    format!("{escaped}%")
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "map_err hands the rusqlite error over by value"
)]
fn db_error(error: rusqlite::Error) -> StoreError {
    StoreError::Other(error.to_string())
}
