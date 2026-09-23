use super::store::{index_dir_prefix, index_path_string, open_db};
use super::*;

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

    let canonical = note.canonicalize().unwrap();
    let stored = index_path_string(&root_str, &canonical).expect("should be indexable");

    let expected = crate::fs::path_to_string(note.clone());
    assert_eq!(stored, expected);
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

    let outside = std::env::temp_dir().join("elsewhere.md");
    assert_eq!(index_path_string(&root_str, &outside), None);

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
    assert!(!format!("{}notes_older/x.md", &prefix[..prefix.len() - 10]).starts_with(&prefix));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_populates_tags_and_links() {
    let root = temp_vault();
    let root_str = crate::fs::path_to_string(root.clone());

    std::fs::write(
        root.join("alpha.md"),
        "#work #work again\nsee [[Beta]] and ![[not-a-link.png]]\n",
    )
    .unwrap();
    std::fs::write(root.join("beta.md"), "#work\nback to [[alpha]]\n").unwrap();

    rebuild(&root_str).unwrap();

    let tags = index_tags(&root_str).unwrap();
    let work = tags.iter().find(|t| t.tag == "work").expect("tag missing");
    assert_eq!(work.count, 2, "one row per note, not per occurrence");

    let beta = crate::fs::path_to_string(root.join("beta.md"));
    let back = index_backlinks(&root_str, &beta).unwrap();
    assert_eq!(back.len(), 1, "expected alpha.md, got {back:?}");
    assert_eq!(back[0].name, "alpha.md");

    let stem_match = crate::fs::path_to_string(root.join("not-a-link.png.md"));
    assert!(
        index_backlinks(&root_str, &stem_match).unwrap().is_empty(),
        "image embeds must not become links"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_older_index_schema_is_replaced_on_open() {
    let root = temp_vault();
    let root_str = crate::fs::path_to_string(root.clone());
    std::fs::write(root.join("alpha.md"), "see [[Beta]]\n").unwrap();
    std::fs::write(root.join("beta.md"), "#work\n").unwrap();

    let conn = open_db(&root_str).unwrap();
    conn.execute_batch(
        "DROP TABLE links;
         CREATE TABLE links (
             src       TEXT NOT NULL,
             target    TEXT NOT NULL,
             target_lc TEXT NOT NULL,
             PRIMARY KEY (src, target)
         );
         INSERT INTO links (src, target, target_lc) VALUES ('stale', 'legacy', 'legacy');",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    drop(conn);

    rebuild(&root_str).unwrap();

    let beta = crate::fs::path_to_string(root.join("beta.md"));
    let back = index_backlinks(&root_str, &beta).unwrap();
    assert_eq!(back.len(), 1, "expected alpha.md, got {back:?}");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn deleting_a_note_clears_its_tags_and_links() {
    let root = temp_vault();
    let root_str = crate::fs::path_to_string(root.clone());
    let note = root.join("gamma.md");
    std::fs::write(&note, "#orphan\n[[alpha]]\n").unwrap();
    rebuild(&root_str).unwrap();
    assert!(
        index_tags(&root_str)
            .unwrap()
            .iter()
            .any(|t| t.tag == "orphan")
    );

    std::fs::remove_file(&note).unwrap();
    rebuild(&root_str).unwrap();

    assert!(
        !index_tags(&root_str)
            .unwrap()
            .iter()
            .any(|t| t.tag == "orphan")
    );
    let alpha = crate::fs::path_to_string(root.join("alpha.md"));
    assert!(index_backlinks(&root_str, &alpha).unwrap().is_empty());

    std::fs::remove_dir_all(&root).ok();
}
