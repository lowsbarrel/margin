use super::*;

fn temp_vault(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "margin-ai-test-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("journal")).unwrap();
    std::fs::write(
        dir.join("projects").join("roadmap.md"),
        "#roadmap\nShip the **editor** rewrite.\nDeferred: sync conflicts.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("projects").join("beta.md"),
        "Links to [[roadmap]].\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("journal").join("today.md"),
        "#journal\nEditor work again.\n",
    )
    .unwrap();
    crate::index::rebuild(&dir.to_string_lossy()).unwrap();
    dir
}

#[test]
fn search_finds_notes_and_cites_them_by_name() {
    let root = temp_vault("search");
    let root = root.to_string_lossy().to_string();

    let out = run(&root, "search", &json!({"query": "editor rewrite"}));
    assert!(out.contains("[[roadmap]]"), "got {out}");
    assert!(out.contains("projects/roadmap.md"), "got {out}");
    assert!(!out.contains("journal/today.md"), "got {out}");

    let out = run(&root, "search", &json!({"query": "editor", "mode": "any"}));
    assert!(out.contains("journal/today.md"), "got {out}");

    let out = run(
        &root,
        "search",
        &json!({"query": "editor rewrite", "mode": "phrase"}),
    );
    assert!(out.contains("[[roadmap]]"), "got {out}");
    let reversed = run(
        &root,
        "search",
        &json!({"query": "rewrite editor", "mode": "phrase"}),
    );
    assert!(!reversed.contains("[[roadmap]]"), "got {reversed}");
}

#[test]
fn read_note_caps_the_range_and_never_escapes_the_vault() {
    let dir = temp_vault("read");
    let root = dir.to_string_lossy().to_string();

    let out = run(&root, "read_note", &json!({"path": "projects/roadmap.md"}));
    assert!(out.contains("Ship the **editor** rewrite."), "got {out}");
    assert!(out.contains("lines 1-"), "got {out}");

    let ranged = run(
        &root,
        "read_note",
        &json!({"path": "projects/roadmap.md", "from_line": 2, "to_line": 2}),
    );
    assert!(ranged.contains("Ship the"), "got {ranged}");
    assert!(!ranged.contains("Deferred:"), "got {ranged}");

    let absolute = dir.join("projects").join("roadmap.md");
    let out = run(
        &root,
        "read_note",
        &json!({"path": absolute.to_string_lossy()}),
    );
    assert!(out.contains("Ship the"), "got {out}");

    let outside = std::env::temp_dir().join("margin-ai-outside.md");
    std::fs::write(&outside, "secret").unwrap();
    let out = run(
        &root,
        "read_note",
        &json!({"path": outside.to_string_lossy()}),
    );
    assert!(out.contains("escapes the vault"), "got {out}");
    assert!(!out.contains("secret"), "leaked file contents: {out}");

    let out = run(&root, "read_note", &json!({"path": "../escape.md"}));
    assert!(out.contains("escapes the vault"), "got {out}");

    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(&outside).ok();
}

#[test]
fn grep_reports_line_numbers_and_respects_a_folder() {
    let dir = temp_vault("grep");
    let root = dir.to_string_lossy().to_string();

    let out = run(
        &root,
        "grep",
        &json!({"pattern": "Editor", "folder": "journal"}),
    );
    assert!(out.starts_with("journal/today.md:2:"), "got {out}");
    assert!(!out.contains("roadmap.md"), "got {out}");

    let out = run(&root, "grep", &json!({"pattern": "^Deferred"}));
    assert!(out.starts_with("projects/roadmap.md:3:"), "got {out}");

    let out = run(&root, "grep", &json!({"pattern": "("}));
    assert!(out.contains("Invalid regular expression"), "got {out}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn tags_links_and_recents_come_from_the_index() {
    let dir = temp_vault("tags");
    let root = dir.to_string_lossy().to_string();

    let out = run(&root, "list_tags", &json!({}));
    assert!(out.contains("#roadmap (1 notes)"), "got {out}");
    assert!(out.contains("#journal (1 notes)"), "got {out}");

    let out = run(&root, "notes_with_tag", &json!({"tag": "Roadmap"}));
    assert!(out.contains("[[roadmap]]"), "got {out}");
    assert!(!out.contains("today.md"), "got {out}");

    let out = run(&root, "backlinks", &json!({"path": "projects/roadmap.md"}));
    assert!(out.contains("[[beta]]"), "got {out}");

    let out = run(&root, "recent_notes", &json!({"days": 1, "limit": 5}));
    assert!(out.contains("projects/roadmap.md"), "got {out}");
    assert!(out.contains("today"), "got {out}");

    let out = run(&root, "find_notes", &json!({"name": "road"}));
    assert!(out.contains("[[roadmap]]"), "got {out}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_missing_argument_is_reported_to_the_model() {
    let root = temp_vault("missing");
    let root = root.to_string_lossy().to_string();

    let out = run(&root, "read_note", &json!({}));
    assert!(out.contains("Missing required argument: path"), "got {out}");
    let out = run(&root, "search", &json!({"query": "   "}));
    assert!(
        out.contains("Missing required argument: query"),
        "got {out}"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_long_note_is_truncated_rather_than_flooding_the_prompt() {
    let dir = temp_vault("long");
    let long = dir.join("projects").join("long.md");
    std::fs::write(&long, "x".repeat(MAX_OUTPUT_CHARS * 2)).unwrap();
    let root = dir.to_string_lossy().to_string();

    let out = run(&root, "read_note", &json!({"path": "projects/long.md"}));
    assert!(
        out.len() <= MAX_OUTPUT_CHARS + 64,
        "not capped: {}",
        out.len()
    );
    assert!(out.contains("truncated"), "got {}", &out[out.len() - 40..]);

    std::fs::remove_dir_all(&dir).ok();
}
