use std::fmt;

use gpui_kit::component::StyledExt;
use gpui_kit::component::input::Escape;
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Context, EventEmitter, ExternalPaths, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Role, StatefulInteractiveElement, Styled, Window, div, uniform_list,
};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, css_px};
use margin_workbench::{DraggedFiles, WorkbenchEvent};

use crate::command::{
    CollapseFolder, Command, CopyEntries, CutEntries, DeleteEntry, ExpandFolder, OpenEntry,
    PasteEntries, RenameEntry, SelectAllEntries, SelectNext, SelectPrevious, TREE_CONTEXT,
};
use crate::file_tree::FileTree;
use crate::model::Key;
use crate::operations::ClipboardMode;
use crate::{header, menu, row, row_drag};

impl EventEmitter<WorkbenchEvent> for FileTree {}

impl fmt::Debug for FileTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTree")
            .field("model", self.model())
            .finish_non_exhaustive()
    }
}

impl Render for FileTree {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        render(self, cx)
    }
}

fn key_listener<A: 'static>(
    key: Key,
    cx: &Context<'_, FileTree>,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |tree, _: &A, window, cx| tree.run(Command::Key(key), window, cx))
}

fn render(tree: &FileTree, cx: &Context<'_, FileTree>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let surface = palette.surface(1);
    let text = palette.text_secondary;
    let focus = tree.focus_handle(cx);
    v_flex_root(tree, cx)
        .id("file-tree")
        .size_full()
        .bg(surface)
        .text_color(text)
        .track_focus(&focus)
        .when(tree.edit().is_none(), |root| root.key_context(TREE_CONTEXT))
        .child(header::render(tree, cx))
        .child(body(tree, cx))
}

fn v_flex_root(tree: &FileTree, cx: &Context<'_, FileTree>) -> gpui_kit::Div {
    let has_clipboard = tree.has_clipboard();
    let has_selection = tree.model().selection().primary().is_some();
    div()
        .v_flex()
        .on_action(key_listener::<SelectNext>(Key::Next, cx))
        .on_action(key_listener::<SelectPrevious>(Key::Previous, cx))
        .on_action(key_listener::<ExpandFolder>(Key::Expand, cx))
        .on_action(key_listener::<CollapseFolder>(Key::Collapse, cx))
        .on_action(key_listener::<OpenEntry>(Key::Activate, cx))
        .on_action(key_listener::<RenameEntry>(Key::Rename, cx))
        .on_action(key_listener::<DeleteEntry>(Key::Delete, cx))
        .when(has_selection, |root| {
            root.on_action(cx.listener(|tree, _: &CopyEntries, window, cx| {
                tree.run(Command::Clipboard(ClipboardMode::Copy, None), window, cx);
            }))
            .on_action(cx.listener(|tree, _: &CutEntries, window, cx| {
                tree.run(Command::Clipboard(ClipboardMode::Cut, None), window, cx);
            }))
        })
        .when(has_clipboard, |root| {
            root.on_action(cx.listener(|tree, _: &PasteEntries, window, cx| {
                let folder = tree.model().target_folder();
                tree.run(Command::Paste(folder), window, cx);
            }))
        })
        .on_action(cx.listener(|tree, _: &SelectAllEntries, window, cx| {
            tree.run(Command::SelectAll, window, cx);
        }))
        .on_action(cx.listener(|tree, _: &Escape, window, cx| tree.close_edit(window, cx)))
}

fn body(tree: &FileTree, cx: &Context<'_, FileTree>) -> impl IntoElement {
    let entity = cx.entity();
    let root = RelativePath::root();
    div()
        .id("file-tree-rows")
        .role(Role::Tree)
        .aria_label(t(cx, "sidebar_explorer"))
        .flex_1()
        .min_h_0()
        .px(css_px(8.0))
        .pb(css_px(8.0))
        .on_click(cx.listener(|tree, _, window, cx| {
            tree.run(Command::ClickBackground, window, cx);
        }))
        .can_drop(move |value, _, _| row_drag::accepts(&root, value))
        .on_drop(cx.listener(|tree, dragged: &DraggedFiles, window, cx| {
            let paths = dragged.paths.clone();
            tree.run(Command::Move(paths, RelativePath::root()), window, cx);
        }))
        .on_drop(cx.listener(|tree, paths: &ExternalPaths, window, cx| {
            let sources = paths.paths().to_vec();
            tree.run(Command::Import(sources, RelativePath::root()), window, cx);
        }))
        .context_menu(move |popup, _, cx| menu::root(popup, &entity, cx))
        .child(
            uniform_list(
                "file-tree-list",
                tree.model().item_count(),
                cx.processor(move |tree, range, window, cx| {
                    row::render_items(tree, range, window, cx)
                }),
            )
            .size_full()
            .track_scroll(tree.scroll()),
        )
}
