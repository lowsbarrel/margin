use std::any::Any;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Context, Div, Entity, ExternalDragPayload, ExternalPaths, FileDragPaths,
    InteractiveElement, Stateful, StatefulInteractiveElement, Styled,
};
use margin_files::TreeRow;
use margin_kernel::RelativePath;
use margin_theme::MarginTheme;
use margin_workbench::DraggedFiles;

use crate::command::Command;
use crate::file_tree::FileTree;
use crate::paths;
use crate::selection::SelectedEntry;

pub(crate) fn can_move_into(folder: &RelativePath, source: &RelativePath) -> bool {
    !folder.starts_with(source) && &paths::parent(source) != folder
}

pub(crate) fn accepts(folder: &RelativePath, value: &dyn Any) -> bool {
    value.downcast_ref::<DraggedFiles>().is_none_or(|dragged| {
        dragged
            .paths
            .iter()
            .any(|source| can_move_into(folder, source))
    })
}

fn outbound_paths(tree: &Entity<FileTree>, dragged: &DraggedFiles, cx: &App) -> FileDragPaths {
    let tree = tree.read(cx);
    let root = tree.vault_root();
    FileDragPaths::new(dragged.paths.iter().map(|path| {
        let is_dir = tree.model().row(path).is_some_and(|row| row.is_dir);
        (root.join(path.as_str()), is_dir)
    }))
}

pub(crate) fn dragged_files(tree: &FileTree, entry: &SelectedEntry) -> DraggedFiles {
    let targets = tree.model().selection().drag_targets(entry);
    let has_folder = targets.iter().any(SelectedEntry::is_dir);
    DraggedFiles::new(targets.into_iter().map(SelectedEntry::into_path).collect())
        .with_folder(has_folder)
}

pub(crate) fn with_drag_and_drop(
    element: Stateful<Div>,
    tree: &FileTree,
    row: &TreeRow,
    entry: &SelectedEntry,
    cx: &Context<'_, FileTree>,
) -> Stateful<Div> {
    let palette = MarginTheme::palette(cx);
    let drop_fill = palette.brand(8.0);
    let drop_border = palette.border_strong;
    let folder = if row.is_dir {
        row.path.clone()
    } else {
        paths::parent(&row.path)
    };
    let outbound = cx.entity();
    element
        .on_drag(dragged_files(tree, entry), |dragged, offset, _, cx| {
            dragged.preview(offset, cx)
        })
        .external_drag_payload::<DraggedFiles>(move |dragged, _, cx| {
            Some(ExternalDragPayload::Files(outbound_paths(
                &outbound, dragged, cx,
            )))
        })
        .can_drop({
            let folder = folder.clone();
            move |value, _, _| accepts(&folder, value)
        })
        .when(row.is_dir, |element| {
            element
                .drag_over::<DraggedFiles>(move |style, _, _, _| {
                    style.bg(drop_fill).border_color(drop_border)
                })
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.bg(drop_fill).border_color(drop_border)
                })
        })
        .on_drop(cx.listener({
            let folder = folder.clone();
            move |tree, dragged: &DraggedFiles, window, cx| {
                let command = Command::Move(dragged.paths.clone(), folder.clone());
                tree.run(command, window, cx);
            }
        }))
        .on_drop(cx.listener(move |tree, paths: &ExternalPaths, window, cx| {
            let command = Command::Import(paths.paths().to_vec(), folder.clone());
            tree.run(command, window, cx);
        }))
}
