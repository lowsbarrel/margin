use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{Action, App, FocusHandle, Window};
use margin_editor_actions::{
    Copy, Cut, Paste, SelectAll, ToggleBold, ToggleBulletList, ToggleCode, ToggleHighlight,
    ToggleItalic, ToggleQuote, ToggleStrike, ToggleTaskList,
};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_markdown::Document;
use margin_table_editing::{CellLocation, OnTableOp, TableMenu, TableOp, table_menu};
use margin_workbench::PasteAsPlainText;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextLink {
    Wiki {
        target: String,
        note: Option<RelativePath>,
    },
    Url(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextImage {
    pub absolute_path: Option<PathBuf>,
    pub source: String,
    pub alt: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContextTarget {
    pub has_selection: bool,
    pub link: Option<ContextLink>,
    pub image: Option<ContextImage>,
    pub table: Option<CellLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextCommand {
    OpenLink(ContextLink),
    OpenInNewPane(RelativePath),
    CopyLink(String),
    ViewImage(ContextImage),
    OpenInDefaultApp(PathBuf),
    CopyPath(PathBuf),
    Reveal(PathBuf),
    Table(TableOp),
}

pub type OnContextCommand = Rc<dyn Fn(ContextCommand, &mut Window, &mut App)>;

fn formats(target: &ContextTarget) -> Vec<(&'static str, Box<dyn Action>)> {
    let mut formats: Vec<(&'static str, Box<dyn Action>)> = vec![
        ("bubble_bold", Box::new(ToggleBold)),
        ("bubble_italic", Box::new(ToggleItalic)),
        ("bubble_strike", Box::new(ToggleStrike)),
        ("bubble_highlight", Box::new(ToggleHighlight)),
        ("bubble_code", Box::new(ToggleCode)),
    ];
    if target.table.is_none() {
        formats.push(("bubble_block_quote", Box::new(ToggleQuote)));
        formats.push(("bubble_block_bullet", Box::new(ToggleBulletList)));
        formats.push(("bubble_block_task", Box::new(ToggleTaskList)));
    }
    formats
}

fn command(
    menu: PopupMenu,
    label_key: &str,
    command: ContextCommand,
    on_command: &OnContextCommand,
    cx: &App,
) -> PopupMenu {
    let on_command = Rc::clone(on_command);
    menu.item(
        PopupMenuItem::new(t(cx, label_key))
            .on_click(move |_, window, cx| on_command(command.clone(), window, cx)),
    )
}

fn image_items(
    menu: PopupMenu,
    image: &ContextImage,
    on_command: &OnContextCommand,
    cx: &App,
) -> PopupMenu {
    let menu = command(
        menu,
        "editor_view_image",
        ContextCommand::ViewImage(image.clone()),
        on_command,
        cx,
    );
    let Some(path) = image.absolute_path.clone() else {
        return menu;
    };
    let menu = command(
        menu,
        "editor_open_default_app",
        ContextCommand::OpenInDefaultApp(path.clone()),
        on_command,
        cx,
    );
    let menu = command(
        menu,
        "editor_copy_path",
        ContextCommand::CopyPath(path.clone()),
        on_command,
        cx,
    );
    command(
        menu,
        "editor_reveal_in_finder",
        ContextCommand::Reveal(path),
        on_command,
        cx,
    )
}

fn link_items(
    menu: PopupMenu,
    link: &ContextLink,
    on_command: &OnContextCommand,
    cx: &App,
) -> PopupMenu {
    let menu = command(
        menu,
        "editor_open_link",
        ContextCommand::OpenLink(link.clone()),
        on_command,
        cx,
    );
    match link {
        ContextLink::Url(url) => command(
            menu,
            "editor_copy_link",
            ContextCommand::CopyLink(url.clone()),
            on_command,
            cx,
        ),
        ContextLink::Wiki {
            note: Some(note), ..
        } => command(
            menu,
            "editor_open_new_pane",
            ContextCommand::OpenInNewPane(note.clone()),
            on_command,
            cx,
        ),
        ContextLink::Wiki { note: None, .. } => menu,
    }
}

#[must_use]
pub fn editor_context_menu(
    menu: PopupMenu,
    target: &ContextTarget,
    document: &Document,
    editor_focus: &FocusHandle,
    on_command: &OnContextCommand,
    cx: &App,
) -> PopupMenu {
    let mut menu = menu.action_context(editor_focus.clone());
    if let Some(image) = &target.image {
        menu = image_items(menu, image, on_command, cx);
    } else {
        if let Some(cell) = &target.table {
            let forward = Rc::clone(on_command);
            let on_op: OnTableOp =
                Rc::new(move |op, window, cx| forward(ContextCommand::Table(op), window, cx));
            menu = table_menu(menu, document, cell, TableMenu::Cell, &on_op, cx);
        }
        if let Some(link) = &target.link {
            menu = link_items(menu, link, on_command, cx);
        }
        if target.has_selection {
            for (key, action) in formats(target) {
                menu = menu.menu(t(cx, key), action);
            }
        }
    }
    let empty = !target.has_selection;
    menu.menu_with_disabled(t(cx, "editor_cut"), Box::new(Cut), empty)
        .menu_with_disabled(t(cx, "editor_copy"), Box::new(Copy), empty)
        .menu(t(cx, "editor_paste"), Box::new(Paste))
        .menu(t(cx, "editor_paste_plain"), Box::new(PasteAsPlainText))
        .menu(t(cx, "editor_select_all"), Box::new(SelectAll))
}
