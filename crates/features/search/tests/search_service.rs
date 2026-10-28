use std::sync::Arc;

use margin_kernel::{FileSystem, RelativePath, UnixMillis};
use margin_search::{SearchIndex, SearchMode, SearchService};
use margin_test_support::{MemoryFileSystem, MemorySearchIndex};

fn path(value: &str) -> RelativePath {
    RelativePath::new(value).expect("test paths are valid")
}

fn service(files: &Arc<MemoryFileSystem>, index: &Arc<MemorySearchIndex>) -> SearchService {
    let files: Arc<MemoryFileSystem> = Arc::clone(files);
    let index: Arc<MemorySearchIndex> = Arc::clone(index);
    SearchService::new(index, files)
}

#[test]
fn rebuild_indexes_visible_markdown_only() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("alpha.md", b"#work the alpha note");
    files.insert("notes/beta.md", b"the beta note");
    files.insert("notes/.hidden.md", b"#secret");
    files.insert(".margin/index.sqlite", b"binary");
    files.insert("notes/picture.png", b"binary");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);

    assert_eq!(service.rebuild().expect("rebuild"), 2);
    assert_eq!(indexed_paths(&index), ["alpha.md", "notes/beta.md"]);
    assert_eq!(
        service
            .search("beta", SearchMode::All, 10)
            .expect("search")
            .len(),
        1
    );
    assert_eq!(service.tags().expect("tags").len(), 1);
}

#[test]
fn rebuild_skips_notes_whose_size_and_modified_time_are_unchanged() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("alpha.md", b"first");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);
    service.rebuild().expect("rebuild");

    files
        .write(&path("alpha.md"), b"xxxxx")
        .expect("the bytes change without a new size or timestamp");
    service.rebuild().expect("rebuild");
    assert_eq!(
        service
            .search("first", SearchMode::All, 10)
            .expect("search")
            .len(),
        1
    );
    assert!(
        service
            .search("xxxxx", SearchMode::All, 10)
            .expect("search")
            .is_empty()
    );

    files
        .set_modified(&path("alpha.md"), UnixMillis::from_millis(7))
        .expect("touch");
    service.rebuild().expect("rebuild");
    assert_eq!(
        service
            .search("xxxxx", SearchMode::All, 10)
            .expect("search")
            .len(),
        1
    );
    assert!(
        service
            .search("first", SearchMode::All, 10)
            .expect("search")
            .is_empty()
    );
}

#[test]
fn rebuild_drops_notes_that_are_gone() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("alpha.md", b"#orphan");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);
    service.rebuild().expect("rebuild");
    assert_eq!(service.tags().expect("tags").len(), 1);

    files
        .remove_file(&path("alpha.md"))
        .expect("remove the note");
    assert_eq!(service.rebuild().expect("rebuild"), 0);
    assert!(indexed_paths(&index).is_empty());
    assert!(service.tags().expect("tags").is_empty());
}

#[test]
fn a_change_batch_upserts_added_notes_and_removes_deleted_ones() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("notes/alpha.md", b"[[Beta]] #work");
    files.insert("notes/beta.md", b"target");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);

    service
        .apply_changes(&[path("notes/alpha.md"), path("notes/beta.md")])
        .expect("apply");
    assert_eq!(indexed_paths(&index), ["notes/alpha.md", "notes/beta.md"]);
    assert_eq!(
        service
            .backlinks(&path("notes/beta.md"))
            .expect("backlinks")
            .len(),
        1
    );

    files
        .remove_dir_all(&path("notes"))
        .expect("remove the folder");
    service.apply_changes(&[path("notes")]).expect("apply");
    assert!(indexed_paths(&index).is_empty());
    assert!(service.tags().expect("tags").is_empty());
}

#[test]
fn hidden_paths_never_reach_the_index() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert(".margin/settings.enc", b"#work secret");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);

    service
        .apply_changes(&[path(".margin/settings.enc")])
        .expect("apply");

    assert!(indexed_paths(&index).is_empty());
}

#[test]
fn tags_are_ranked_by_count_then_name() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("a.md", b"#work #alpha");
    files.insert("b.md", b"#work #beta");
    files.insert("c.md", b"#alpha");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);
    service.rebuild().expect("rebuild");

    let tags = service.tags().expect("tags");
    assert_eq!(
        tags.iter()
            .map(|info| (info.tag.as_str(), info.count))
            .collect::<Vec<_>>(),
        [("alpha", 2), ("work", 2), ("beta", 1)]
    );
    let work = tags
        .iter()
        .find(|info| info.tag == "work")
        .expect("the work tag is indexed");
    assert_eq!(
        work.files
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["a.md", "b.md"]
    );
}

#[test]
fn backlinks_are_sorted_by_source_name() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("zeta.md", b"see [[Target]]");
    files.insert("alpha.md", b"see [[Target|alias]]");
    files.insert("target.md", b"[[target]] and itself");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);
    service.rebuild().expect("rebuild");

    let backlinks = service.backlinks(&path("target.md")).expect("backlinks");
    assert_eq!(
        backlinks
            .iter()
            .map(|link| link.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha.md", "zeta.md"]
    );
}

#[test]
fn search_applies_the_requested_mode() {
    let files = Arc::new(MemoryFileSystem::new());
    files.insert("alpha.md", b"alpha beta gamma");
    let index = Arc::new(MemorySearchIndex::new());
    let service = service(&files, &index);
    service.rebuild().expect("rebuild");

    assert_eq!(
        service
            .search("alpha delta", SearchMode::Any, 10)
            .expect("search")
            .len(),
        1
    );
    assert!(
        service
            .search("alpha delta", SearchMode::All, 10)
            .expect("search")
            .is_empty()
    );
    assert!(
        service
            .search("beta alpha", SearchMode::Phrase, 10)
            .expect("search")
            .is_empty()
    );
    assert_eq!(
        service
            .search("alpha beta", SearchMode::Phrase, 10)
            .expect("search")
            .len(),
        1
    );
    assert!(
        service
            .search("  ", SearchMode::All, 10)
            .expect("search")
            .is_empty()
    );
}

fn indexed_paths(index: &MemorySearchIndex) -> Vec<String> {
    index
        .indexed()
        .expect("indexed")
        .into_iter()
        .map(|meta| meta.path.to_string())
        .collect()
}
