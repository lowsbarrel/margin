use std::fs;

use margin_kernel::RelativePath;
use margin_search::SearchIndex;
use margin_sqlite::SqliteIndex;
use margin_test_support::search_index_conformance;

#[test]
fn the_sqlite_index_passes_the_search_index_conformance_suite() {
    let directory = tempfile::tempdir().expect("a temp directory");
    let index = SqliteIndex::open(directory.path().join("index.sqlite")).expect("the index opens");
    search_index_conformance(&index);
}

#[test]
fn an_older_schema_is_replaced_on_open() {
    let directory = tempfile::tempdir().expect("a temp directory");
    let db_path = directory.path().join(".margin").join("index.sqlite");
    let parent = db_path.parent().expect("the db path has a parent");
    fs::create_dir_all(parent).expect("the .margin directory");
    {
        let connection = rusqlite::Connection::open(&db_path).expect("the database opens");
        connection
            .execute_batch(
                "CREATE TABLE links (src TEXT NOT NULL, target TEXT NOT NULL, target_lc TEXT NOT NULL);
                 INSERT INTO links VALUES ('stale', 'legacy', 'legacy');",
            )
            .expect("the legacy schema is written");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("the legacy version is stamped");
    }

    let index = SqliteIndex::open(db_path.clone()).expect("the index opens");

    assert!(
        index
            .backlinks(&RelativePath::new("legacy.md").expect("a valid path"))
            .expect("backlinks")
            .is_empty()
    );
    let connection = rusqlite::Connection::open(&db_path).expect("the database opens");
    let links: i64 = connection
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
        .expect("the links table exists");
    assert_eq!(links, 0);
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("the version is readable");
    assert_eq!(version, 4);
}

#[test]
fn open_creates_the_missing_index_directory() {
    let directory = tempfile::tempdir().expect("a temp directory");
    let db_path = directory.path().join(".margin").join("index.sqlite");

    let _index = SqliteIndex::open(db_path.clone()).expect("the index opens");

    assert!(db_path.exists());
}
