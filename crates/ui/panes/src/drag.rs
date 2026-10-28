use gpui_kit::{App, AppContext, Entity, EntityId, Pixels, Point, SharedString};
use margin_workbench::{DragPreview, DraggedFiles};

use crate::layout::{PaneId, Tab};
use crate::tab_order::{clamp_insert_index, reorder_tab_gap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraggedTab {
    pub pane: PaneId,
    pub item: EntityId,
    pub pinned: bool,
    pub label: SharedString,
}

impl DraggedTab {
    pub fn preview(&self, cursor_offset: Point<Pixels>, cx: &mut App) -> Entity<DragPreview> {
        let label = self.label.clone();
        cx.new(|_| DragPreview::new(label, 0, cursor_offset))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DividerDrag(pub usize);

impl gpui_kit::Render for DividerDrag {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<'_, Self>,
    ) -> impl gpui_kit::IntoElement {
        gpui_kit::Empty
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dragged {
    Tab {
        pane: usize,
        tab: usize,
        pinned: bool,
    },
    Files {
        has_folder: bool,
    },
}

impl Dragged {
    #[must_use]
    pub const fn files(files: &DraggedFiles) -> Self {
        Self::Files {
            has_folder: files.has_folder,
        }
    }

    #[must_use]
    pub const fn is_files(self) -> bool {
        matches!(self, Self::Files { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneMode {
    None,
    Split,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZone {
    Left,
    Center,
    Right,
}

#[must_use]
pub const fn zone_mode(dragged: Dragged, pane: usize, tab_count: usize) -> ZoneMode {
    match dragged {
        Dragged::Files { has_folder: true } => ZoneMode::None,
        Dragged::Tab { pane: source, .. } if source != pane => ZoneMode::All,
        Dragged::Tab { .. } if tab_count < 2 => ZoneMode::None,
        Dragged::Files { .. } | Dragged::Tab { .. } => ZoneMode::Split,
    }
}

#[must_use]
pub const fn strip_accepts(dragged: Dragged, pane: usize, tab_count: usize) -> bool {
    match dragged {
        Dragged::Files { has_folder } => !has_folder,
        Dragged::Tab { pane: source, .. } => source != pane || tab_count >= 2,
    }
}

#[must_use]
pub fn strip_drop_gap<T>(dragged: Dragged, pane: usize, tabs: &[Tab<T>], gap: usize) -> usize {
    match dragged {
        Dragged::Tab {
            pane: source, tab, ..
        } if source == pane => reorder_tab_gap(tabs, tab, gap),
        Dragged::Tab { pinned, .. } => clamp_insert_index(tabs, pinned, gap),
        Dragged::Files { .. } => clamp_insert_index(tabs, false, gap),
    }
}

#[must_use]
pub fn strip_gap_at(tab_bounds: &[(Pixels, Pixels)], x: Pixels) -> usize {
    tab_bounds
        .iter()
        .position(|(left, width)| x < *left + *width / 2.0)
        .unwrap_or(tab_bounds.len())
}
