use gpui_kit::px;
use margin_kernel::RelativePath;

use crate::closed_tabs::ClosedTabs;
use crate::drag::{Dragged, ZoneMode, strip_accepts, strip_drop_gap, strip_gap_at, zone_mode};
use crate::layout::{PaneLayout, Side, Tab};

fn tabs(layout: &PaneLayout<&'static str>, pane: usize) -> Vec<&'static str> {
    layout.panes()[pane]
        .tabs
        .iter()
        .map(|tab| tab.item)
        .collect()
}

fn active(layout: &PaneLayout<&'static str>, pane: usize) -> Option<&'static str> {
    layout.panes()[pane].active_tab().map(|tab| tab.item)
}

fn one_pane(items: &[&'static str]) -> PaneLayout<&'static str> {
    let mut layout = PaneLayout::default();
    for item in items {
        layout.open(0, None, *item);
    }
    layout
}

fn two_panes() -> PaneLayout<&'static str> {
    let mut layout = one_pane(&["a", "b"]);
    layout.open_split(0, Side::Right, "c");
    layout.open(1, None, "d");
    layout
}

#[test]
fn opening_appends_activates_and_never_lands_before_pinned_tabs() {
    let mut layout = one_pane(&["a", "b"]);
    assert_eq!(active(&layout, 0), Some("b"));
    layout.toggle_pin(0, 1);
    assert_eq!(tabs(&layout, 0), ["b", "a"]);
    let at = layout.open(0, Some(0), "c");
    assert_eq!(at, 1);
    assert_eq!(tabs(&layout, 0), ["b", "c", "a"]);
    assert_eq!(active(&layout, 0), Some("c"));
}

#[test]
fn closing_the_active_tab_activates_the_tab_that_took_its_place() {
    let mut layout = one_pane(&["a", "b", "c"]);
    layout.switch(0, 1);
    assert_eq!(layout.close(0, 1), ["b"]);
    assert_eq!(active(&layout, 0), Some("c"));
    layout.close(0, 1);
    assert_eq!(active(&layout, 0), Some("a"));
    layout.open(0, None, "d");
    layout.close(0, 0);
    assert_eq!(active(&layout, 0), Some("d"));
}

#[test]
fn closing_the_last_tab_closes_the_pane_unless_it_is_the_only_one() {
    let mut layout = two_panes();
    layout.close(1, 0);
    layout.close(1, 0);
    assert_eq!(layout.panes().len(), 1);
    assert_eq!(layout.flexes(), [1.0]);
    assert_eq!(layout.active_pane(), 0);
    layout.close(0, 0);
    layout.close(0, 0);
    assert_eq!(layout.panes().len(), 1);
    assert!(layout.panes()[0].tabs.is_empty());
    assert_eq!(layout.active_tab(), None);
}

#[test]
fn close_others_and_close_all_keep_pinned_tabs() {
    let mut layout = one_pane(&["a", "b", "c", "d"]);
    layout.toggle_pin(0, 3);
    assert_eq!(layout.close_others(0, 3), ["a", "b"]);
    assert_eq!(tabs(&layout, 0), ["d", "c"]);
    assert_eq!(active(&layout, 0), Some("c"));
    assert_eq!(layout.close_all(0), ["c"]);
    assert_eq!(tabs(&layout, 0), ["d"]);
    assert_eq!(active(&layout, 0), Some("d"));
    let mut split = two_panes();
    assert_eq!(split.close_all(1), ["c", "d"]);
    assert_eq!(split.panes().len(), 1);
}

#[test]
fn pinning_moves_tabs_first_and_the_active_tab_follows() {
    let mut layout = one_pane(&["a", "b", "c"]);
    layout.switch(0, 1);
    layout.toggle_pin(0, 2);
    assert_eq!(tabs(&layout, 0), ["c", "a", "b"]);
    assert_eq!(active(&layout, 0), Some("b"));
    layout.toggle_pin(0, 0);
    assert_eq!(tabs(&layout, 0), ["c", "a", "b"]);
    assert!(!layout.panes()[0].tabs[0].pinned);
}

#[test]
fn reordering_within_a_strip_uses_gaps_and_respects_the_pinned_block() {
    let mut layout = one_pane(&["a", "b", "c", "d"]);
    layout.switch(0, 0);
    layout.move_within(0, 0, 3);
    assert_eq!(tabs(&layout, 0), ["b", "c", "a", "d"]);
    assert_eq!(active(&layout, 0), Some("a"));
    layout.move_within(0, 3, 0);
    assert_eq!(tabs(&layout, 0), ["d", "b", "c", "a"]);
    layout.move_within(0, 1, 2);
    assert_eq!(tabs(&layout, 0), ["d", "b", "c", "a"]);
    layout.toggle_pin(0, 0);
    layout.move_within(0, 3, 0);
    assert_eq!(tabs(&layout, 0), ["d", "a", "b", "c"]);
}

#[test]
fn moving_a_tab_into_another_pane_removes_the_emptied_source() {
    let mut layout = two_panes();
    layout.move_to_pane(1, 0, 0, Some(1));
    assert_eq!(tabs(&layout, 0), ["a", "c", "b"]);
    assert_eq!(active(&layout, 0), Some("c"));
    assert_eq!(layout.active_pane(), 0);
    layout.move_to_pane(1, 0, 0, None);
    assert_eq!(layout.panes().len(), 1);
    assert_eq!(tabs(&layout, 0), ["a", "c", "b", "d"]);
    assert_eq!(layout.flexes(), [1.0]);
}

#[test]
fn moving_from_an_emptied_left_pane_targets_the_shifted_destination() {
    let mut layout = PaneLayout::default();
    layout.open(0, None, "a");
    layout.open_split(0, Side::Right, "b");
    layout.move_to_pane(0, 0, 1, None);
    assert_eq!(layout.panes().len(), 1);
    assert_eq!(tabs(&layout, 0), ["b", "a"]);
    assert_eq!(layout.active_pane(), 0);
}

#[test]
fn splitting_halves_the_reference_flex_and_focuses_the_new_pane() {
    let mut layout = one_pane(&["a", "b"]);
    layout.split_tab(0, 1, 0, Side::Left);
    assert_eq!(tabs(&layout, 0), ["b"]);
    assert_eq!(tabs(&layout, 1), ["a"]);
    assert_eq!(layout.flexes(), [0.5, 0.5]);
    assert_eq!(layout.active_pane(), 0);
    layout.split_tab(0, 0, 1, Side::Right);
    assert_eq!(layout.panes().len(), 2);
    assert_eq!(tabs(&layout, 0), ["a"]);
    assert_eq!(tabs(&layout, 1), ["b"]);
    assert_eq!(layout.flexes(), [0.5, 0.5]);
    assert_eq!(layout.active_pane(), 1);
}

#[test]
fn removing_deleted_files_keeps_the_active_tab_when_it_survives() {
    let mut layout = two_panes();
    layout.switch(0, 0);
    let removed = layout.remove_where(|item| *item == "b" || *item == "c" || *item == "d");
    assert_eq!(removed, ["b", "c", "d"]);
    assert_eq!(active(&layout, 0), Some("a"));
    assert_eq!(layout.panes().len(), 2);
    assert_eq!(layout.panes()[1].active, None);
}

#[test]
fn restoring_drops_empty_panes_and_clamps_indices_and_flexes() {
    let mut layout = PaneLayout::default();
    let tab = |item| Tab {
        item,
        pinned: false,
    };
    layout.restore(
        vec![
            (vec![tab("a"), tab("b")], 7),
            (Vec::new(), 0),
            (vec![tab("c")], 0),
        ],
        &[2.0, 1.0, 1.0],
        9,
    );
    assert_eq!(layout.panes().len(), 2);
    assert_eq!(active(&layout, 0), Some("b"));
    assert_eq!(layout.flexes(), [1.0, 1.0]);
    assert_eq!(layout.active_pane(), 1);
    layout.restore(
        vec![(vec![tab("x")], 0), (vec![tab("y")], 0)],
        &[3.0, 1.0],
        0,
    );
    assert_eq!(layout.flexes(), [3.0, 1.0]);
}

#[test]
fn dividers_resize_neighbours_within_the_minimum_width() {
    let mut layout = two_panes();
    layout.resize(0, 300.0, 1000.0, 120.0);
    assert!((layout.flexes()[0] - 0.3).abs() < 1e-6);
    assert!((layout.flexes()[1] - 0.7).abs() < 1e-6);
    layout.resize(0, 10.0, 1000.0, 120.0);
    assert!((layout.flexes()[0] - 0.12).abs() < 1e-6);
}

#[test]
fn drop_zones_and_strips_follow_the_original_rules() {
    let tab = Dragged::Tab {
        pane: 0,
        tab: 0,
        pinned: false,
    };
    assert_eq!(zone_mode(tab, 1, 1), ZoneMode::All);
    assert_eq!(zone_mode(tab, 0, 2), ZoneMode::Split);
    assert_eq!(zone_mode(tab, 0, 1), ZoneMode::None);
    let file = Dragged::Files { has_folder: false };
    let folder = Dragged::Files { has_folder: true };
    assert_eq!(zone_mode(file, 0, 0), ZoneMode::Split);
    assert_eq!(zone_mode(folder, 0, 3), ZoneMode::None);
    assert!(!strip_accepts(tab, 0, 1));
    assert!(strip_accepts(tab, 1, 1));
    assert!(!strip_accepts(folder, 0, 1));
    let layout = one_pane(&["a", "b", "c"]);
    let strip = &layout.panes()[0].tabs;
    assert_eq!(strip_drop_gap(tab, 0, strip, 1), 0);
    assert_eq!(strip_drop_gap(tab, 0, strip, 3), 3);
    assert_eq!(strip_drop_gap(file, 0, strip, 9), 3);
    let edges = [(px(0.0), px(100.0)), (px(100.0), px(100.0))];
    assert_eq!(strip_gap_at(&edges, px(49.0)), 0);
    assert_eq!(strip_gap_at(&edges, px(51.0)), 1);
    assert_eq!(strip_gap_at(&edges, px(180.0)), 2);
}

#[test]
fn closed_tabs_remember_ten_unique_paths_most_recent_first() {
    let path = |name: &str| RelativePath::new(name).expect("valid path");
    let mut closed = ClosedTabs::default();
    for index in 0..12 {
        closed.push(path(&format!("n{index}.md")));
    }
    closed.push(path("n5.md"));
    assert_eq!(closed.take(), Some(path("n5.md")));
    assert_eq!(closed.take(), Some(path("n11.md")));
    closed.forget(|gone| gone.as_str().starts_with("n1"));
    let mut rest = Vec::new();
    while let Some(next) = closed.take() {
        rest.push(next.as_str().to_owned());
    }
    assert_eq!(
        rest,
        [
            "n9.md", "n8.md", "n7.md", "n6.md", "n4.md", "n3.md", "n2.md"
        ]
    );
}
