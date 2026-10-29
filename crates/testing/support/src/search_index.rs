use margin_kernel::{RelativePath, UnixMillis};
use margin_search::{IndexedNote, SearchIndex, SearchMode, build_match_for, build_match_query};

fn path(value: &str) -> RelativePath {
    RelativePath::new(value).expect("conformance paths are valid")
}

fn note(value: &str, body: &str, modified: u64) -> IndexedNote {
    IndexedNote {
        path: path(value),
        body: body.to_owned(),
        modified: UnixMillis::from_millis(modified),
        size: u64::try_from(body.len()).expect("conformance bodies are small"),
    }
}

fn hits(index: &dyn SearchIndex, match_query: &str, limit: u32) -> Vec<String> {
    index
        .search(match_query, limit)
        .expect("the index searches")
        .into_iter()
        .map(|hit| hit.path.to_string())
        .collect()
}

pub fn search_index_conformance(index: &dyn SearchIndex) {
    finds_body_and_name_terms(index);
    ignores_unknown_terms(index);
    matches_word_prefixes(index);
    joins_terms_with_and_or_or(index);
    matches_phrases_in_order(index);
    honours_the_limit(index);
    snapshots_the_body(index);
    matches_nothing_without_a_query(index);
    groups_tags_per_note(index);
    lists_backlinks_by_source_name(index);
    removes_a_note_and_its_subtree(index);
    moves_rows_on_rename(index);
    reports_stored_metadata(index);
    replaces_a_note_on_reindex(index);
}

fn finds_body_and_name_terms(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("notes/alpha.md", "the quick brown fox", 1)])
        .expect("upsert");

    assert_eq!(
        hits(index, &build_match_query("quick"), 10),
        ["notes/alpha.md"]
    );
    assert_eq!(
        hits(index, &build_match_query("alpha"), 10),
        ["notes/alpha.md"]
    );
}

fn ignores_unknown_terms(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "quick brown fox", 1)])
        .expect("upsert");

    assert!(hits(index, &build_match_query("zebra"), 10).is_empty());
    assert!(hits(index, &build_match_query("argi"), 10).is_empty());
}

fn matches_word_prefixes(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "a marginal note", 1)])
        .expect("upsert");

    assert_eq!(hits(index, &build_match_query("marg"), 10), ["alpha.md"]);
    assert_eq!(
        hits(index, &build_match_query("marginal"), 10),
        ["alpha.md"]
    );
    assert!(hits(index, &build_match_query("marginals"), 10).is_empty());
}

fn joins_terms_with_and_or_or(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "alpha beta gamma", 1)])
        .expect("upsert");

    assert_eq!(
        hits(index, &build_match_for("alpha gamma", SearchMode::All), 10),
        ["alpha.md"]
    );
    assert!(hits(index, &build_match_for("alpha delta", SearchMode::All), 10).is_empty());
    assert_eq!(
        hits(index, &build_match_for("delta gamma", SearchMode::Any), 10),
        ["alpha.md"]
    );
    assert!(
        hits(
            index,
            &build_match_for("delta epsilon", SearchMode::Any),
            10
        )
        .is_empty()
    );
}

fn matches_phrases_in_order(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "alpha beta gamma", 1)])
        .expect("upsert");

    assert_eq!(
        hits(
            index,
            &build_match_for("alpha beta", SearchMode::Phrase),
            10
        ),
        ["alpha.md"]
    );
    assert!(
        hits(
            index,
            &build_match_for("beta alpha", SearchMode::Phrase),
            10
        )
        .is_empty()
    );
}

fn honours_the_limit(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    for name in ["a.md", "b.md", "c.md"] {
        index
            .upsert(&[note(name, "shared body", 1)])
            .expect("upsert");
    }

    assert_eq!(hits(index, &build_match_query("shared"), 10).len(), 3);
    assert_eq!(hits(index, &build_match_query("shared"), 2).len(), 2);
    assert!(hits(index, &build_match_query("shared"), 0).is_empty());
}

fn snapshots_the_body(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "some words before margin and after", 1)])
        .expect("upsert");

    let search = index
        .search(&build_match_query("margin"), 10)
        .expect("search");
    let hit = search.first().expect("a hit");
    assert!(
        hit.snippet.to_lowercase().contains("margin"),
        "got {}",
        hit.snippet
    );
}

fn matches_nothing_without_a_query(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("alpha.md", "alpha", 1)])
        .expect("upsert");

    assert!(hits(index, "", 10).is_empty());
}

fn groups_tags_per_note(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("a.md", "# Heading\n#work #work again", 1)])
        .expect("upsert");
    index.upsert(&[note("b.md", "#work", 1)]).expect("upsert");

    let tags = index.tags().expect("tags");
    let work: Vec<_> = tags.iter().filter(|info| info.tag == "work").collect();
    assert_eq!(work.len(), 1, "got {tags:?}");
    assert_eq!(work[0].count, 2);
    assert_eq!(
        work[0]
            .files
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["a.md", "b.md"]
    );
    assert!(
        tags.iter().all(|info| info.tag != "heading"),
        "got {tags:?}"
    );
}

fn lists_backlinks_by_source_name(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("notes/alpha.md", "see [[BETA]]", 1)])
        .expect("upsert");
    index
        .upsert(&[note("notes/beta.md", "[[beta]] links to itself", 1)])
        .expect("upsert");

    let backlinks = index.backlinks(&path("notes/beta.md")).expect("backlinks");
    assert_eq!(backlinks.len(), 1, "got {backlinks:?}");
    assert_eq!(backlinks[0].path.to_string(), "notes/alpha.md");
    assert_eq!(backlinks[0].name, "alpha.md");

    assert!(
        index
            .backlinks(&path("notes/missing.md"))
            .expect("backlinks")
            .is_empty()
    );
}

fn removes_a_note_and_its_subtree(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("notes/x.md", "#orphan\n[[alpha]]", 1)])
        .expect("upsert");
    index
        .upsert(&[note("notes/deep/y.md", "nested body", 1)])
        .expect("upsert");
    index
        .upsert(&[note("alpha.md", "target", 1)])
        .expect("upsert");
    index
        .upsert(&[note("notes_older/z.md", "sibling folder", 1)])
        .expect("upsert");

    index.remove(&path("notes/x.md")).expect("remove");
    assert!(hits(index, &build_match_query("orphan"), 10).is_empty());
    assert!(
        index
            .backlinks(&path("alpha.md"))
            .expect("backlinks")
            .is_empty()
    );

    index.remove(&path("notes")).expect("remove");
    assert!(hits(index, &build_match_query("nested"), 10).is_empty());
    assert_eq!(
        hits(index, &build_match_query("sibling"), 10),
        ["notes_older/z.md"]
    );
}

fn moves_rows_on_rename(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("old.md", "#work\nsee [[Beta]]", 1)])
        .expect("upsert");
    index
        .upsert(&[note("beta.md", "target", 1)])
        .expect("upsert");

    index
        .rename(&path("old.md"), &path("new.md"))
        .expect("rename");

    assert_eq!(hits(index, &build_match_query("work"), 10), ["new.md"]);
    let backlinks = index.backlinks(&path("beta.md")).expect("backlinks");
    assert_eq!(backlinks.len(), 1, "got {backlinks:?}");
    assert_eq!(backlinks[0].path.to_string(), "new.md");
}

fn reports_stored_metadata(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index.upsert(&[note("a.md", "hello", 42)]).expect("upsert");

    let indexed = index.indexed().expect("indexed");
    assert_eq!(indexed.len(), 1);
    assert_eq!(indexed[0].path.to_string(), "a.md");
    assert_eq!(indexed[0].modified, UnixMillis::from_millis(42));
    assert_eq!(indexed[0].size, 5);
}

fn replaces_a_note_on_reindex(index: &dyn SearchIndex) {
    index.clear().expect("the index clears");
    index
        .upsert(&[note("a.md", "first body", 1)])
        .expect("upsert");
    index
        .upsert(&[note("a.md", "second body", 2)])
        .expect("upsert");

    assert_eq!(index.indexed().expect("indexed").len(), 1);
    assert!(hits(index, &build_match_query("first"), 10).is_empty());
    assert_eq!(hits(index, &build_match_query("second"), 10), ["a.md"]);
}
