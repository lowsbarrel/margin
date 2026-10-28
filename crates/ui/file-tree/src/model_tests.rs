use margin_files::TreeRow;
use margin_kernel::{RelativePath, UnixMillis};

use crate::model::{Effect, Gesture, Item, Key, TreeModel};
use crate::selection::SelectedEntry;

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

fn row(raw: &str, is_dir: bool) -> TreeRow {
    let path = path(raw);
    TreeRow {
        name: path.file_name().to_owned(),
        depth: path.components().count() - 1,
        path,
        is_dir,
        modified: UnixMillis::default(),
    }
}

fn expanded_model() -> TreeModel {
    let mut model = TreeModel::new();
    model.set_expanded(vec![path("a"), path("a/b")]);
    model.set_rows(vec![
        row("a", true),
        row("a/b", true),
        row("a/b/x.md", false),
        row("a/y.md", false),
        row("z.md", false),
    ]);
    model
}

fn visible(model: &TreeModel) -> Vec<&str> {
    model.rows().iter().map(|row| row.path.as_str()).collect()
}

#[test]
fn collapsing_removes_only_the_folder_descendants() {
    let mut model = expanded_model();
    model.collapse(&path("a/b"));
    assert_eq!(visible(&model), ["a", "a/b", "a/y.md", "z.md"]);
    model.collapse(&path("a"));
    assert_eq!(visible(&model), ["a", "z.md"]);
    assert!(model.expanded().is_empty());
}

#[test]
fn arrows_walk_rows_and_left_climbs_to_the_parent() {
    let mut model = expanded_model();
    assert_eq!(model.press(Key::Next), Effect::Reveal(path("a/b")));
    assert_eq!(model.press(Key::Next), Effect::Reveal(path("a/b/x.md")));
    assert_eq!(model.press(Key::Collapse), Effect::Reveal(path("a/b")));
    assert_eq!(model.press(Key::Collapse), Effect::Redraw);
    assert_eq!(visible(&model), ["a", "a/b", "a/y.md", "z.md"]);
    assert_eq!(model.press(Key::Expand), Effect::Rebuild);
    assert!(model.is_expanded(&path("a/b")));
    assert_eq!(model.press(Key::Previous), Effect::Reveal(path("a")));
    assert_eq!(model.press(Key::Previous), Effect::None);
}

#[test]
fn right_on_an_open_folder_enters_its_first_child() {
    let mut model = expanded_model();
    assert_eq!(model.press(Key::Expand), Effect::Reveal(path("a/b")));
}

#[test]
fn enter_opens_files_and_delete_targets_the_focused_row() {
    let mut model = expanded_model();
    model.set_focus(&path("z.md"));
    assert_eq!(model.press(Key::Activate), Effect::Open(path("z.md")));
    assert_eq!(
        model.press(Key::Delete),
        Effect::Delete(SelectedEntry::new(path("z.md"), false))
    );
    assert_eq!(model.press(Key::Rename), Effect::Rename(path("z.md")));
}

#[test]
fn clicks_toggle_folders_once_and_triple_click_renames() {
    let mut model = expanded_model();
    let click = |count| Gesture::Plain { click_count: count };
    assert_eq!(model.click(&path("a/b"), click(1)), Effect::Redraw);
    assert!(!model.is_expanded(&path("a/b")));
    assert_eq!(model.click(&path("a/b"), click(2)), Effect::Redraw);
    assert_eq!(
        model.click(&path("a/b"), click(3)),
        Effect::Rename(path("a/b"))
    );
    assert_eq!(model.target_folder(), path("a/b"));
    assert_eq!(
        model.click(&path("a/y.md"), click(1)),
        Effect::Open(path("a/y.md"))
    );
    assert_eq!(model.target_folder(), path("a"));
}

#[test]
fn selection_survives_collapse_but_not_deletion() {
    let mut model = expanded_model();
    model.click(&path("a/b/x.md"), Gesture::Plain { click_count: 1 });
    model.click(&path("z.md"), Gesture::Toggle);
    model.collapse(&path("a"));
    let rows = model.rows().to_vec();
    model.set_rows(rows);
    assert_eq!(model.selection().entries().len(), 2);
    model.set_rows(vec![row("a", true)]);
    assert_eq!(model.selection().entries().len(), 1);
}

#[test]
fn renaming_a_folder_keeps_its_expanded_descendants() {
    let mut model = expanded_model();
    model.remap(&path("a"), &path("c"));
    let expanded: Vec<&str> = model.expanded().iter().map(RelativePath::as_str).collect();
    assert_eq!(expanded, ["c", "c/b"]);
}

#[test]
fn the_new_folder_row_sits_after_the_parent_subtree() {
    let mut model = expanded_model();
    model.set_pending_new_folder(Some(path("a/b")));
    assert_eq!(model.item_count(), 6);
    assert!(matches!(model.item(3), Some(Item::NewFolder { depth: 2 })));
    assert!(matches!(model.item(4), Some(Item::Row(row)) if row.path == path("a/y.md")));
    assert_eq!(model.item_index(&path("z.md")), Some(5));
    model.set_pending_new_folder(Some(RelativePath::root()));
    assert!(matches!(model.item(0), Some(Item::NewFolder { depth: 0 })));
}

#[test]
fn reveal_expands_every_ancestor() {
    let mut model = TreeModel::new();
    assert!(model.expand_ancestors(&path("a/b/c/note.md")));
    let expanded: Vec<&str> = model.expanded().iter().map(RelativePath::as_str).collect();
    assert_eq!(expanded, ["a", "a/b", "a/b/c"]);
    assert!(!model.expand_ancestors(&path("a/b/other.md")));
}

#[test]
fn the_active_file_follows_into_the_selection_and_anchors_new_entries() {
    let mut model = expanded_model();
    assert!(model.set_active(Some(path("a/y.md"))));
    assert!(!model.set_active(Some(path("a/y.md"))));
    assert!(model.selection().contains(&path("a/y.md")));
    assert_eq!(model.target_folder(), path("a"));
    model.clear_selection();
    assert_eq!(model.target_folder(), RelativePath::root());
}

#[test]
fn dragging_a_selected_row_carries_the_whole_selection_grabbed_first() {
    let mut model = expanded_model();
    model.click(&path("a/y.md"), Gesture::Plain { click_count: 1 });
    model.click(&path("z.md"), Gesture::Toggle);
    model.click(&path("a/b/x.md"), Gesture::Toggle);
    let grabbed = SelectedEntry::new(path("z.md"), false);
    let dragged: Vec<_> = model
        .selection()
        .drag_targets(&grabbed)
        .into_iter()
        .map(SelectedEntry::into_path)
        .collect();
    assert_eq!(dragged, [path("z.md"), path("a/y.md"), path("a/b/x.md")]);
    let outsider = SelectedEntry::new(path("a"), true);
    assert_eq!(model.selection().drag_targets(&outsider), [outsider]);
}
