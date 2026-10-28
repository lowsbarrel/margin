use std::rc::Rc;

use gpui_kit::component::menu::PopupMenu;
use gpui_kit::{
    App, ClipboardItem, Context, DismissEvent, Entity, Focusable, Pixels, Point, SharedString,
    Subscription, Window,
};
use margin_editor_assets::{
    Embed, ImageLocation, VaultIndex, cached_image, image_resource, resolve_embed, resolve_image,
};
use margin_editor_overlays::{
    ContextCommand, ContextImage, ContextLink, ContextTarget, LightboxImage, OnContextCommand,
    editor_context_menu,
};
use margin_live_preview::{AtomKind, LinkKind, render_line};
use margin_table_editing::{apply_table_op, cell_at};
use margin_text_buffer::{EditOrigin, Selection};
use margin_workbench::{OpenRequest, OpenTarget, Platform, WorkbenchEvent, notify_error};

use crate::note_editor::{Edited, NoteEditor};
use crate::pointer::OpenIn;
use crate::{links, surfaces};

pub(crate) struct OpenMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    _subscription: Subscription,
}

impl OpenMenu {
    pub(crate) const fn menu(&self) -> &Entity<PopupMenu> {
        &self.menu
    }

    pub(crate) const fn position(&self) -> Point<Pixels> {
        self.position
    }
}

fn target_at(editor: &NoteEditor, offset: usize, cx: &App) -> ContextTarget {
    let session = editor.session();
    let document = session.document();
    let line = document.line_of(offset);
    let view = render_line(document, line, session.mode(), session.reveal());
    let display = view.map.to_display(offset);
    let hovered_link = view
        .links
        .iter()
        .find(|link| link.display.start <= display && display <= link.display.end)
        .and_then(|link| match &link.kind {
            LinkKind::Wiki(wiki) => Some(ContextLink::Wiki {
                target: wiki.target.clone(),
                note: VaultIndex::get(cx).and_then(|index| index.find_by_name(&wiki.target)),
            }),
            LinkKind::Url(url) => Some(ContextLink::Url(url.clone())),
            LinkKind::Tag(_) | LinkKind::Footnote(_) => None,
        });
    let image = view
        .atoms
        .iter()
        .find(|atom| atom.source.start <= offset && offset <= atom.source.end)
        .and_then(|atom| {
            let index = VaultIndex::get(cx)?;
            let source = atom.source.slice(document.text()).to_owned();
            let resolved = match &atom.kind {
                AtomKind::Image { destination, alt } => {
                    resolve_image(index, editor.note_path(), destination, alt)
                }
                AtomKind::Embed(_) => match resolve_embed(
                    index,
                    editor.note_path(),
                    source.trim_start_matches("![[").trim_end_matches("]]"),
                ) {
                    Some(Embed::Image(image)) => Some(image),
                    _ => None,
                },
                _ => None,
            }?;
            let absolute_path = match &resolved.location {
                ImageLocation::Local(path) => Some(path.clone()),
                ImageLocation::Remote(_) => None,
            };
            Some(ContextImage {
                absolute_path,
                source,
                alt: resolved.alt,
            })
        });
    ContextTarget {
        has_selection: session
            .selections()
            .iter()
            .any(|selection| !selection.is_empty()),
        link: hovered_link,
        image,
        table: cell_at(document, offset),
    }
}

pub(crate) fn open(
    editor: &mut NoteEditor,
    position: Point<Pixels>,
    offset: usize,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let inside = editor
        .session()
        .selections()
        .iter()
        .any(|selection| selection.start() <= offset && offset <= selection.end());
    if !inside {
        editor.session_mut().select(Selection::cursor(offset));
        editor.edited(Edited::Selection, window, cx);
    }
    let target = target_at(editor, offset, cx);
    let focus = editor.focus_handle_ref().clone();
    let entity = cx.entity().downgrade();
    let on_command: OnContextCommand = Rc::new(move |command, window, cx| {
        drop(entity.update(cx, |editor, cx| run(editor, command, offset, window, cx)));
    });
    let document = editor.session().document().clone();
    let menu = PopupMenu::build(window, cx, |menu, _, cx| {
        editor_context_menu(menu, &target, &document, &focus, &on_command, cx)
    });
    let subscription = cx.subscribe_in(&menu, window, |editor, _, _: &DismissEvent, window, cx| {
        editor.surfaces_mut().set_context_menu(None);
        let focus = editor.focus_handle_ref().clone();
        window.focus(&focus, cx);
        cx.notify();
    });
    menu.focus_handle(cx).focus(window, cx);
    editor.surfaces_mut().set_context_menu(Some(OpenMenu {
        menu,
        position,
        _subscription: subscription,
    }));
    cx.notify();
}

fn run(
    editor: &mut NoteEditor,
    command: ContextCommand,
    offset: usize,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match command {
        ContextCommand::OpenLink(ContextLink::Wiki { target, .. }) => {
            let link = margin_markdown::WikiLink {
                target,
                heading: None,
                block: None,
                alias: None,
            };
            links::open_wiki(editor, &link, OpenIn::Current, window, cx);
        }
        ContextCommand::OpenLink(ContextLink::Url(url)) => {
            links::open_destination(editor, &url, OpenIn::Current, window, cx);
        }
        ContextCommand::OpenInNewPane(path) => {
            cx.emit(WorkbenchEvent::Open(
                OpenRequest::new(path).in_target(OpenTarget::SplitRight),
            ));
        }
        ContextCommand::CopyLink(link) => cx.write_to_clipboard(ClipboardItem::new_string(link)),
        ContextCommand::ViewImage(image) => {
            if let Some(path) = image.absolute_path {
                let images = vec![LightboxImage {
                    source: cached_image(
                        image_resource(&ImageLocation::Local(path)),
                        editor.render_state().images(),
                    ),
                    alt: SharedString::from(image.alt),
                }];
                surfaces::open_lightbox(editor, images, 0, window, cx);
            }
        }
        ContextCommand::OpenInDefaultApp(path) => {
            if let Err(error) = Platform::open_external(cx, &path.to_string_lossy()) {
                notify_error(window, cx, "toast_cannot_open_file", &error);
            }
        }
        ContextCommand::CopyPath(path) => {
            cx.write_to_clipboard(ClipboardItem::new_string(
                path.to_string_lossy().into_owned(),
            ));
        }
        ContextCommand::Reveal(path) => {
            if let Err(error) = Platform::reveal(cx, &path) {
                notify_error(window, cx, "toast_cannot_reveal_file", &error);
            }
        }
        ContextCommand::Table(op) => {
            let document = editor.session().document();
            if let Some(location) = cell_at(document, offset)
                && let Some(change) = apply_table_op(document, &location, op)
            {
                editor.apply(change, EditOrigin::Command, window, cx);
            }
        }
    }
}
