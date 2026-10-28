use gpui_kit::{App, AppContext, Entity, Pixels, Point};
use margin_kernel::RelativePath;

use crate::drag_preview::DragPreview;
use crate::file_kind::file_title;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraggedFiles {
    pub paths: Vec<RelativePath>,
    pub has_folder: bool,
}

impl DraggedFiles {
    #[must_use]
    pub const fn new(paths: Vec<RelativePath>) -> Self {
        Self {
            paths,
            has_folder: false,
        }
    }

    #[must_use]
    pub const fn with_folder(mut self, has_folder: bool) -> Self {
        self.has_folder = has_folder;
        self
    }

    pub fn preview(&self, cursor_offset: Point<Pixels>, cx: &mut App) -> Entity<DragPreview> {
        let label = self
            .paths
            .first()
            .map(|path| file_title(path).to_owned())
            .unwrap_or_default();
        let extra = self.paths.len().saturating_sub(1);
        cx.new(|_| DragPreview::new(label.into(), extra, cursor_offset))
    }
}
