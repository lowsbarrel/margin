use gpui_kit::{Context, Window};
use margin_editor_assets::{LinkRef, Resolved, VaultIndex, resolve_link, web_href};
use margin_kernel::RelativePath;
use margin_live_preview::LinkKind;
use margin_markdown::WikiLink;
use margin_workbench::{
    OpenRequest, OpenTarget, Platform, WorkbenchEvent, notify_error, notify_info, run_blocking,
};

use crate::note_editor::NoteEditor;
use crate::pointer::OpenIn;
use crate::row_context::RowAction;
use crate::{async_ops, row_actions};

pub(crate) fn open(
    editor: &mut NoteEditor,
    kind: &LinkKind,
    open_in: OpenIn,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match kind {
        LinkKind::Tag(name) => cx.emit(WorkbenchEvent::TagSelected(name.clone())),
        LinkKind::Footnote(id) => {
            row_actions::handle(editor, RowAction::JumpToFootnote(id.clone()), window, cx);
        }
        LinkKind::Wiki(link) => open_wiki(editor, link, open_in, window, cx),
        LinkKind::Url(href) => open_destination(editor, href, open_in, window, cx),
    }
}

pub(crate) fn open_wiki(
    editor: &mut NoteEditor,
    link: &WikiLink,
    open_in: OpenIn,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if link.target.trim().is_empty() {
        if let Some(anchor) = link
            .block
            .as_ref()
            .map(|block| format!("^{block}"))
            .or_else(|| link.heading.clone())
        {
            editor.scroll_to_anchor(&anchor, window, cx);
        }
        return;
    }
    let resolved = VaultIndex::get(cx)
        .map(|index| resolve_link(index, editor.note_path(), LinkRef::Wiki(link)));
    follow(editor, resolved, open_in, window, cx);
}

pub(crate) fn open_destination(
    editor: &mut NoteEditor,
    href: &str,
    open_in: OpenIn,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let resolved = VaultIndex::get(cx)
        .map(|index| resolve_link(index, editor.note_path(), LinkRef::Destination(href)))
        .or_else(|| Some(Resolved::External(web_href(href))));
    follow(editor, resolved, open_in, window, cx);
}

fn target(open_in: OpenIn) -> OpenTarget {
    match open_in {
        OpenIn::Current => OpenTarget::ActivePane,
        OpenIn::NewTab => OpenTarget::NewTab,
    }
}

fn follow(
    editor: &mut NoteEditor,
    resolved: Option<Resolved>,
    open_in: OpenIn,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match resolved {
        Some(Resolved::Note { path, anchor }) => {
            if &path == editor.note_path() {
                if let Some(anchor) = anchor {
                    editor.scroll_to_anchor(&anchor, window, cx);
                }
                return;
            }
            let mut request = OpenRequest::new(path).in_target(target(open_in));
            if let Some(anchor) = anchor {
                request = request.at_anchor(anchor);
            }
            cx.emit(WorkbenchEvent::Open(request));
        }
        Some(Resolved::Attachment(path)) => {
            cx.emit(WorkbenchEvent::Open(
                OpenRequest::new(path).in_target(target(open_in)),
            ));
        }
        Some(Resolved::MissingNote { path_to_create }) => {
            create_and_open(editor, path_to_create, open_in, cx);
        }
        Some(Resolved::External(url)) => {
            if let Err(error) = Platform::open_external(cx, &url) {
                notify_error(window, cx, "toast_cannot_open_url", &error);
            }
        }
        Some(Resolved::OutsideVault) => notify_info(window, cx, "toast_link_outside_vault"),
        Some(Resolved::Unsupported) | None => {}
    }
}

fn create_and_open(
    editor: &NoteEditor,
    path: RelativePath,
    open_in: OpenIn,
    cx: &Context<'_, NoteEditor>,
) {
    let services = editor.services();
    let target_path = path.clone();
    let work = run_blocking(cx, move || {
        if services.files.exists(&target_path) {
            return Ok(());
        }
        services.files.write_file(&target_path, b"")
    });
    async_ops::after(
        work,
        editor.window_handle(),
        cx,
        move |_, result, window, cx| match result {
            Ok(()) => {
                VaultIndex::refresh(cx);
                cx.emit(WorkbenchEvent::TreeChanged);
                cx.emit(WorkbenchEvent::Open(
                    OpenRequest::new(path).in_target(target(open_in)),
                ));
            }
            Err(error) => notify_error(window, cx, "toast_cannot_open_file", &error),
        },
    );
}
