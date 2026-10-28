use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{Action, App, Context, Entity, Focusable, ParentElement, Styled, div};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_theme::MarginTheme;

use crate::command::{
    Command, CopyEntries, CutEntries, DeleteEntry, NewEntry, PasteEntries, RenameEntry,
};
use crate::file_tree::FileTree;
use crate::operations::ClipboardMode;
use crate::paths;
use crate::selection::SelectedEntry;

fn on_click(item: PopupMenuItem, tree: &Entity<FileTree>, command: Command) -> PopupMenuItem {
    let tree = tree.clone();
    item.on_click(move |_, window, cx| {
        tree.update(cx, |tree, cx| tree.run(command.clone(), window, cx));
    })
}

fn item(key: &str, tree: &Entity<FileTree>, command: Command, cx: &App) -> PopupMenuItem {
    on_click(PopupMenuItem::new(t(cx, key)), tree, command)
}

fn shortcut_item(
    key: &str,
    tree: &Entity<FileTree>,
    command: Command,
    action: &dyn Action,
    cx: &App,
) -> PopupMenuItem {
    item(key, tree, command, cx).action(action.boxed_clone())
}

fn delete_item(tree: &Entity<FileTree>, entry: &SelectedEntry, cx: &App) -> PopupMenuItem {
    let label = t(cx, "sidebar_delete");
    let item = PopupMenuItem::element(move |_, cx| {
        div()
            .text_color(MarginTheme::palette(cx).text_negative)
            .child(label.clone())
    });
    on_click(item, tree, Command::Delete(entry.clone())).action(DeleteEntry.boxed_clone())
}

fn reveal_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "sidebar_open_in_finder"
    } else {
        "file_tree_reveal_in_file_manager"
    }
}

fn creation_items(
    menu: PopupMenu,
    tree: &Entity<FileTree>,
    folder: &RelativePath,
    cx: &App,
) -> PopupMenu {
    let create = |kind| Command::Create(kind, folder.clone());
    menu.item(item("sidebar_new_file", tree, create(NewEntry::Note), cx))
        .item(item(
            "sidebar_new_canvas",
            tree,
            create(NewEntry::Canvas),
            cx,
        ))
        .item(item(
            "sidebar_new_folder",
            tree,
            create(NewEntry::Folder),
            cx,
        ))
}

fn paste_item(
    menu: PopupMenu,
    tree: &Entity<FileTree>,
    folder: RelativePath,
    cx: &App,
) -> PopupMenu {
    if !tree.read(cx).has_clipboard() {
        return menu;
    }
    menu.item(shortcut_item(
        "sidebar_paste",
        tree,
        Command::Paste(folder),
        &PasteEntries,
        cx,
    ))
}

pub(crate) fn root(
    menu: PopupMenu,
    tree: &Entity<FileTree>,
    cx: &Context<'_, PopupMenu>,
) -> PopupMenu {
    let root = RelativePath::root();
    let menu = creation_items(menu.action_context(tree.focus_handle(cx)), tree, &root, cx);
    paste_item(menu, tree, root.clone(), cx).item(item(
        reveal_key(),
        tree,
        Command::Reveal(root),
        cx,
    ))
}

pub(crate) fn entry(
    menu: PopupMenu,
    tree: &Entity<FileTree>,
    entry: &SelectedEntry,
    cx: &Context<'_, PopupMenu>,
) -> PopupMenu {
    let path = entry.path().clone();
    let mut menu = menu.action_context(tree.focus_handle(cx));
    let paste_folder = if entry.is_dir() {
        menu = creation_items(menu, tree, &path, cx);
        path.clone()
    } else {
        paths::parent(&path)
    };
    let clipboard = |mode| Command::Clipboard(mode, Some(entry.clone()));
    let menu = menu
        .item(shortcut_item(
            "sidebar_copy",
            tree,
            clipboard(ClipboardMode::Copy),
            &CopyEntries,
            cx,
        ))
        .item(shortcut_item(
            "sidebar_cut",
            tree,
            clipboard(ClipboardMode::Cut),
            &CutEntries,
            cx,
        ));
    paste_item(menu, tree, paste_folder, cx)
        .item(shortcut_item(
            "sidebar_rename",
            tree,
            Command::Rename(path.clone()),
            &RenameEntry,
            cx,
        ))
        .item(item(
            "sidebar_duplicate",
            tree,
            Command::Duplicate(entry.clone()),
            cx,
        ))
        .item(item(reveal_key(), tree, Command::Reveal(path), cx))
        .separator()
        .item(delete_item(tree, entry, cx))
}
