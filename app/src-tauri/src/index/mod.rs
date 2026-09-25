pub mod tree;

mod store;

#[cfg(test)]
mod tests;

use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use store::{delete_note_rows, index_dir_prefix, index_path_string, mtime_nanos, open_db, upsert};

#[derive(Serialize, specta::Type)]
pub struct SearchHit {
    pub path: String,
    pub name: String,
    pub snippet: String,
}

#[derive(Serialize, specta::Type)]
pub struct TagInfo {
    pub tag: String,
    #[specta(type = u32)]
    pub count: usize,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, specta::Type)]
pub struct Backlink {
    pub path: String,
    pub name: String,
}

// Quoting keeps tokens that collide with FTS keywords (AND/OR/NOT/NEAR) literal; the filter keeps operator characters away from the parser.
fn terms(input: &str) -> Vec<String> {
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
    parts
}

fn build_match_query(input: &str) -> String {
    terms(input).join(" ")
}

pub fn build_match_for(input: &str, mode: &str) -> String {
    match mode {
        "phrase" => {
            let phrase = input.replace('"', " ").trim().to_string();
            if phrase.is_empty() {
                String::new()
            } else {
                format!("\"{phrase}\"")
            }
        }
        "any" => terms(input).join(" OR "),
        _ => build_match_query(input),
    }
}

pub fn search_match(root: &str, match_query: &str, limit: u32) -> Result<Vec<SearchHit>, String> {
    if match_query.is_empty() {
        return Ok(vec![]);
    }

    let conn = open_db(root)?;
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

pub fn remove_path(root: &str, path: &Path) {
    let Some(path_str) = index_path_string(root, path) else {
        return;
    };
    if let Ok(conn) = open_db(root) {
        let _ = delete_note_rows(&conn, &path_str);
    }
}

pub fn remove_prefix(root: &str, dir: &Path) {
    let Some(prefix) = index_dir_prefix(root, dir) else {
        return;
    };
    // `%` and `_` are LIKE wildcards and both occur in real filenames, so the escape character is escaped first.
    let escaped = prefix
        .replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_");
    let pattern = format!("{escaped}%");
    if let Ok(conn) = open_db(root) {
        for sql in [
            r"DELETE FROM notes WHERE path LIKE ?1 ESCAPE '\'",
            r"DELETE FROM notes_fts WHERE path LIKE ?1 ESCAPE '\'",
            r"DELETE FROM tags WHERE path LIKE ?1 ESCAPE '\'",
            r"DELETE FROM links WHERE src LIKE ?1 ESCAPE '\'",
        ] {
            let _ = conn.execute(sql, params![pattern]);
        }
    }
}

pub fn rebuild(root: &str) -> Result<u32, String> {
    let mut md_paths = Vec::new();
    crate::fs::collect_md_paths(Path::new(root), &mut md_paths);

    let mut conn = open_db(root)?;

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

        if let Some(&(emt, esz)) = existing.get(&path_str)
            && emt == mtime
            && esz == size
        {
            continue;
        }

        let body = std::fs::read_to_string(path).unwrap_or_default();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        upsert(&tx, &path_str, &name, &body, mtime, size).map_err(|e| e.to_string())?;
    }

    for path_str in existing.keys() {
        if !present.contains(path_str) {
            delete_note_rows(&tx, path_str).map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(present.len() as u32)
}

#[tauri::command(async)]
#[specta::specta]
pub fn index_search(root: &str, query: &str, limit: u32) -> Result<Vec<SearchHit>, String> {
    search_match(root, &build_match_query(query), limit)
}

#[tauri::command]
#[specta::specta]
pub async fn index_rebuild(root: String) -> Result<u32, String> {
    tokio::task::spawn_blocking(move || rebuild(&root))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command(async)]
#[specta::specta]
pub fn index_tags(root: &str) -> Result<Vec<TagInfo>, String> {
    let conn = open_db(root)?;
    let mut stmt = conn
        .prepare("SELECT tag, path FROM tags ORDER BY tag, path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut by_tag: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (tag, path) = row.map_err(|e| e.to_string())?;
        by_tag.entry(tag).or_default().push(path);
    }

    let mut out: Vec<TagInfo> = by_tag
        .into_iter()
        .map(|(tag, files)| TagInfo {
            tag,
            count: files.len(),
            files,
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.tag.cmp(&b.tag)));
    Ok(out)
}

#[tauri::command(async)]
#[specta::specta]
pub fn index_backlinks(root: &str, path: &str) -> Result<Vec<Backlink>, String> {
    let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) else {
        return Ok(vec![]);
    };
    let target = stem.to_lowercase();

    let conn = open_db(root)?;
    let mut stmt = conn
        .prepare(
            "SELECT notes.path, notes.name
             FROM links JOIN notes ON notes.path = links.src
             WHERE links.target_lc = ?1 AND links.src <> ?2
             ORDER BY notes.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![target, path], |r| {
            Ok(Backlink {
                path: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
