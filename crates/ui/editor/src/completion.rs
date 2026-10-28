use chrono::{Local, Offset, TimeZone};
use gpui_kit::{App, Context, Entity, Window};
use margin_dates::{CivilDateTime, UtcOffset};
use margin_editor_commands::apply_slash;
use margin_editor_overlays::{Completion, CompletionEvent, CompletionMenu, Trigger};
use margin_i18n::{t, t_with};
use margin_markdown::{Span, TextEdit};
use margin_text_buffer::{Change, EditOrigin, Selection};
use margin_workbench::AppServices;

use crate::editing;
use crate::note_editor::{Edited, NoteEditor};
use crate::surfaces::MenuKey;

pub(crate) struct Completing {
    menu: Entity<CompletionMenu>,
    open: bool,
}

impl Completing {
    pub(crate) const fn new(menu: Entity<CompletionMenu>) -> Self {
        Self { menu, open: false }
    }

    pub(crate) fn visible_menu(&self) -> Option<Entity<CompletionMenu>> {
        self.open.then(|| self.menu.clone())
    }

    pub(crate) fn refresh_sources(&self, cx: &mut App) {
        self.menu.update(cx, CompletionMenu::refresh_sources);
    }
}

pub(crate) fn update(
    editor: &mut NoteEditor,
    trigger: Option<Trigger>,
    kind: Edited,
    cx: &mut Context<'_, NoteEditor>,
) {
    let completing = editor.surfaces_mut().completing_mut();
    match trigger {
        Some(trigger) if completing.open || kind == Edited::Text => {
            completing.open = true;
            let menu = completing.menu.clone();
            menu.update(cx, |menu, cx| menu.set_trigger(trigger, cx));
        }
        _ => {
            if completing.open {
                completing.open = false;
                let menu = completing.menu.clone();
                menu.update(cx, CompletionMenu::dismiss);
            }
        }
    }
}

pub(crate) fn close(editor: &mut NoteEditor, cx: &mut Context<'_, NoteEditor>) -> bool {
    let completing = editor.surfaces_mut().completing_mut();
    if !completing.open {
        return false;
    }
    completing.open = false;
    let menu = completing.menu.clone();
    menu.update(cx, CompletionMenu::dismiss);
    cx.notify();
    true
}

pub(crate) fn menu_key(
    editor: &mut NoteEditor,
    key: MenuKey,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    let Some(menu) = editor.surfaces().completing().visible_menu() else {
        return false;
    };
    match key {
        MenuKey::Up => menu.update(cx, CompletionMenu::select_previous),
        MenuKey::Down => menu.update(cx, CompletionMenu::select_next),
        MenuKey::Confirm => {
            let read = menu.read(cx);
            if read.is_pending() {
                return true;
            }
            let Some(completion) = read.confirm() else {
                close(editor, cx);
                return false;
            };
            accept(editor, completion, window, cx);
        }
    }
    cx.notify();
    true
}

pub(crate) fn on_event(
    editor: &mut NoteEditor,
    event: &CompletionEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let CompletionEvent::Confirm(completion) = event;
    accept(editor, completion.clone(), window, cx);
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
}

fn today(cx: &App) -> String {
    let now = cx.global::<AppServices>().clock.now();
    let millis = i64::try_from(now.as_millis()).unwrap_or(i64::MAX);
    let offset = Local
        .timestamp_millis_opt(millis)
        .single()
        .map_or(UtcOffset::UTC, |time| {
            UtcOffset::from_minutes(time.offset().fix().local_minus_utc() / 60)
        });
    let date = CivilDateTime::from_unix_millis(now, offset);
    let month = t(cx, &format!("panels_month_{}", date.month));
    t_with(
        cx,
        "panels_date_day_month_year",
        &[
            ("month", month.as_ref()),
            ("day", &date.day.to_string()),
            ("year", &date.year.to_string()),
        ],
    )
    .to_string()
}

fn accept(
    editor: &mut NoteEditor,
    completion: Completion,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    close(editor, cx);
    match completion {
        Completion::Slash { trigger, command } => {
            let today = today(cx);
            editing::apply_change(
                editor,
                EditOrigin::Command,
                window,
                cx,
                |document, selections, _| {
                    apply_slash(document, selections, trigger, command, &today)
                },
            );
        }
        Completion::Insert {
            replace,
            text,
            cursor_after,
        } => {
            let cursor = replace.start + cursor_after;
            let change = Change::new(vec![TextEdit::new(
                Span::new(replace.start, replace.end),
                text,
            )])
            .with_selections(vec![Selection::cursor(cursor)]);
            editor.apply(change, EditOrigin::Command, window, cx);
        }
    }
    editor.edited(Edited::Selection, window, cx);
}

pub(crate) fn open_slash_menu(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() {
        return;
    }
    let document = editor.session().document();
    let text = document.text();
    let body_line = document.frontmatter().map_or(0, |frontmatter| {
        document.line_of(frontmatter.span.end.saturating_sub(1)) + 1
    });
    let (at, insert) = if body_line >= document.line_count() {
        (text.len(), "\n/")
    } else {
        let span = document.line_span(body_line);
        if span.is_empty() {
            (span.start, "/")
        } else {
            (span.start, "/\n")
        }
    };
    let cursor = at + insert.find('/').map_or(0, |index| index + 1);
    let change = Change::new(vec![TextEdit::new(Span::empty(at), insert)])
        .with_selections(vec![Selection::cursor(cursor)]);
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
    editor.apply(change, EditOrigin::Typing, window, cx);
}
