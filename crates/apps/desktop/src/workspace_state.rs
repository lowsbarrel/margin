use gpui_kit::{App, Context, Entity, Window};
use margin_files::SortBy;
use margin_kernel::RelativePath;
use margin_panes::{PaneItem, PaneLayout, Panes, Tab};
use margin_settings::{WorkspacePane, WorkspaceState, WorkspaceTab};
use margin_workbench::VaultServices;

use crate::tab_view::{TabView, view_mode_from_name, view_mode_name};
use crate::workspace::Workspace;

pub(crate) const DEFAULT_SIDEBAR_WIDTH: f32 = 260.0;
pub(crate) const DEFAULT_TERMINAL_HEIGHT: f32 = 280.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShellLayout {
    pub sidebar_open: bool,
    pub sidebar_width: f32,
    pub terminal_open: bool,
    pub terminal_height: f32,
}

impl Default for ShellLayout {
    fn default() -> Self {
        Self {
            sidebar_open: true,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            terminal_open: false,
            terminal_height: DEFAULT_TERMINAL_HEIGHT,
        }
    }
}

impl ShellLayout {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "panel sizes are a few thousand pixels at most"
    )]
    fn from_state(state: &WorkspaceState) -> Self {
        let defaults = Self::default();
        let positive = |value: f64, fallback: f32| {
            if value > 0.0 { value as f32 } else { fallback }
        };
        Self {
            sidebar_open: state.sidebar_open,
            sidebar_width: positive(state.sidebar_width, defaults.sidebar_width),
            terminal_open: state.terminal_open,
            terminal_height: positive(state.terminal_height, defaults.terminal_height),
        }
    }
}

pub(crate) struct Snapshot {
    pub layout: ShellLayout,
    pub expanded_folders: Vec<RelativePath>,
    pub sort_by: SortBy,
}

pub(crate) fn capture(
    layout: &PaneLayout<TabView>,
    snapshot: &Snapshot,
    cx: &App,
) -> WorkspaceState {
    let panes = layout
        .panes()
        .iter()
        .filter(|pane| !pane.tabs.is_empty())
        .map(|pane| WorkspacePane {
            tabs: pane.tabs.iter().map(|tab| workspace_tab(tab, cx)).collect(),
            active_tab_index: pane
                .active
                .and_then(|active| i32::try_from(active).ok())
                .unwrap_or_default(),
        })
        .collect();
    let kept = |index: usize| !layout.panes()[index].tabs.is_empty();
    WorkspaceState {
        panes,
        pane_flexes: layout
            .flexes()
            .iter()
            .enumerate()
            .filter(|(index, _)| kept(*index))
            .map(|(_, flex)| f64::from(*flex))
            .collect(),
        active_pane_index: (0..layout.active_pane())
            .filter(|index| kept(*index))
            .count(),
        expanded_folders: snapshot
            .expanded_folders
            .iter()
            .map(|folder| folder.as_str().to_owned())
            .collect(),
        sidebar_open: snapshot.layout.sidebar_open,
        sidebar_width: f64::from(snapshot.layout.sidebar_width),
        sort_order: sort_name(snapshot.sort_by).to_owned(),
        terminal_open: snapshot.layout.terminal_open,
        terminal_height: f64::from(snapshot.layout.terminal_height),
    }
}

pub(crate) fn load_existing(services: &VaultServices) -> Option<WorkspaceState> {
    let mut state = services.settings.load_workspace(&services.key).ok()??;
    for pane in &mut state.panes {
        let before = pane.tabs.len();
        pane.tabs.retain(|tab| {
            RelativePath::new(&tab.path).is_ok_and(|path| services.files.exists(&path))
        });
        if pane.tabs.len() != before {
            pane.active_tab_index = 0;
        }
    }
    drop_empty_panes(&mut state);
    Some(state)
}

fn drop_empty_panes(state: &mut WorkspaceState) {
    let kept: Vec<bool> = state
        .panes
        .iter()
        .map(|pane| !pane.tabs.is_empty())
        .collect();
    let active = state.active_pane_index.min(kept.len());
    state.active_pane_index -= kept[..active].iter().filter(|kept| !**kept).count();
    if state.pane_flexes.len() == kept.len() {
        let mut flags = kept.iter();
        state
            .pane_flexes
            .retain(|_| flags.next().copied().unwrap_or_default());
    }
    state.panes.retain(|pane| !pane.tabs.is_empty());
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "pane flexes are small ratios"
)]
pub(crate) fn apply(
    state: &WorkspaceState,
    panes: &Entity<Panes<TabView>>,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) -> Snapshot {
    let mut restored = Vec::new();
    for pane in &state.panes {
        let tabs: Vec<Tab<TabView>> = pane
            .tabs
            .iter()
            .filter_map(|tab| {
                let path = RelativePath::new(&tab.path).ok()?;
                let view = TabView::open(path, window, cx);
                restore_tab(&view, tab, window, cx);
                Some(Tab {
                    item: view,
                    pinned: tab.pinned,
                })
            })
            .collect();
        let active = usize::try_from(pane.active_tab_index).unwrap_or_default();
        restored.push((tabs, active));
    }
    let flexes: Vec<f32> = state.pane_flexes.iter().map(|flex| *flex as f32).collect();
    panes.update(cx, |panes, cx| {
        panes.restore(restored, &flexes, state.active_pane_index, window, cx);
    });
    Snapshot {
        layout: ShellLayout::from_state(state),
        expanded_folders: state
            .expanded_folders
            .iter()
            .filter_map(|folder| RelativePath::new(folder).ok())
            .collect(),
        sort_by: if state.sort_order == "date" {
            SortBy::Date
        } else {
            SortBy::Name
        },
    }
}

const fn sort_name(sort_by: SortBy) -> &'static str {
    match sort_by {
        SortBy::Name => "name",
        SortBy::Date => "date",
    }
}

fn workspace_tab(tab: &Tab<TabView>, cx: &App) -> WorkspaceTab {
    let view = &tab.item;
    let note = view.note().map(|editor| editor.read(cx));
    WorkspaceTab {
        path: view.handle().path(cx).as_str().to_owned(),
        tab_type: view.type_name().to_owned(),
        pinned: tab.pinned,
        view_mode: note.map_or_else(
            || view_mode_name(margin_editor::ViewMode::LivePreview).to_owned(),
            |editor| view_mode_name(editor.view_mode()).to_owned(),
        ),
        cursor_pos: note.and_then(|editor| u32::try_from(editor.cursor_offset()).ok()),
    }
}

fn restore_tab(view: &TabView, tab: &WorkspaceTab, window: &mut Window, cx: &mut App) {
    if let Some(editor) = view.note() {
        editor.update(cx, |editor, cx| {
            editor.set_view_mode(view_mode_from_name(&tab.view_mode), cx);
            if let Some(cursor) = tab
                .cursor_pos
                .and_then(|cursor| usize::try_from(cursor).ok())
            {
                editor.set_cursor_offset(cursor, window, cx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(sidebar_width: f64, terminal_height: f64) -> WorkspaceState {
        WorkspaceState {
            panes: Vec::new(),
            pane_flexes: Vec::new(),
            active_pane_index: 0,
            expanded_folders: Vec::new(),
            sidebar_open: false,
            sidebar_width,
            sort_order: "date".to_owned(),
            terminal_open: true,
            terminal_height,
        }
    }

    #[test]
    fn layout_keeps_saved_sizes_and_replaces_missing_ones_with_defaults() {
        let saved = ShellLayout::from_state(&state(320.0, 180.0));
        assert!(!saved.sidebar_open);
        assert!(saved.terminal_open);
        assert!((saved.sidebar_width - 320.0).abs() < f32::EPSILON);
        assert!((saved.terminal_height - 180.0).abs() < f32::EPSILON);
        let missing = ShellLayout::from_state(&state(0.0, -1.0));
        assert!((missing.sidebar_width - DEFAULT_SIDEBAR_WIDTH).abs() < f32::EPSILON);
        assert!((missing.terminal_height - DEFAULT_TERMINAL_HEIGHT).abs() < f32::EPSILON);
    }

    fn pane(paths: &[&str]) -> WorkspacePane {
        WorkspacePane {
            tabs: paths
                .iter()
                .map(|path| WorkspaceTab {
                    path: (*path).to_owned(),
                    tab_type: "markdown".to_owned(),
                    pinned: false,
                    view_mode: "rich".to_owned(),
                    cursor_pos: None,
                })
                .collect(),
            active_tab_index: 0,
        }
    }

    #[test]
    fn dropping_empty_panes_keeps_flexes_and_the_active_pane_aligned() {
        let mut saved = state(260.0, 280.0);
        saved.panes = vec![pane(&["a.md"]), pane(&[]), pane(&["b.md"])];
        saved.pane_flexes = vec![1.0, 2.0, 3.0];
        saved.active_pane_index = 2;
        drop_empty_panes(&mut saved);
        assert_eq!(saved.panes, vec![pane(&["a.md"]), pane(&["b.md"])]);
        assert_eq!(saved.pane_flexes, vec![1.0, 3.0]);
        assert_eq!(saved.active_pane_index, 1);
    }
}
