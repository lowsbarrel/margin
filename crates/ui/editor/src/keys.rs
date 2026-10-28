use gpui_kit::{Action, App, Context, Div, Global, InteractiveElement, Stateful, Window};
use margin_editor_actions::{
    Backspace, Cancel, Copy, Cut, CycleChecklist, Delete, DeleteLine, DeleteToLineEnd,
    DeleteToLineStart, DeleteWordLeft, DeleteWordRight, ExportHtml, FindNextMatch,
    FindPreviousMatch, Indent, InsertLink, MoveDocEnd, MoveDocStart, MoveDown, MoveLeft,
    MoveLineDown, MoveLineEnd, MoveLineStart, MoveLineUp, MovePageDown, MovePageUp, MoveRight,
    MoveUp, MoveWordLeft, MoveWordRight, Newline, OpenContextMenu, OpenSlashMenu, Outdent, Paste,
    Redo, SelectAll, SelectDocEnd, SelectDocStart, SelectDown, SelectLeft, SelectLineEnd,
    SelectLineStart, SelectMatches, SelectPageDown, SelectPageUp, SelectParagraphDown,
    SelectParagraphUp, SelectRight, SelectUp, SelectWordLeft, SelectWordRight, SoftBreak,
    ToggleBold, ToggleBulletList, ToggleCode, ToggleComment, ToggleHeading1, ToggleHeading2,
    ToggleHeading3, ToggleHeading4, ToggleHeading5, ToggleHeading6, ToggleHighlight, ToggleItalic,
    ToggleOrderedList, ToggleQuote, ToggleStrike, ToggleTaskList, Undo, bind_editor_keys,
};
use margin_editor_commands::{ListKind, MarkKind};
use margin_workbench::{FindInNote, PasteAsPlainText, ReplaceInNote, ToggleViewMode};

use crate::clipboard::{self, PasteMode};
use crate::editing::{self, Deletion};
use crate::movement::{self, Extend, Motion};
use crate::note_editor::NoteEditor;
use crate::{find, formatting, surfaces};

struct KeysBound;

impl Global for KeysBound {}

pub(crate) fn bind_once(cx: &mut App) {
    if cx.has_global::<KeysBound>() {
        return;
    }
    cx.set_global(KeysBound);
    bind_editor_keys(cx);
}

type Handler = fn(&mut NoteEditor, &Window, &mut Context<'_, NoteEditor>);
type HandlerMut = fn(&mut NoteEditor, &mut Window, &mut Context<'_, NoteEditor>);

fn on<A: Action>(
    element: Stateful<Div>,
    cx: &Context<'_, NoteEditor>,
    handler: Handler,
) -> Stateful<Div> {
    element.on_action(cx.listener(move |editor, _: &A, window, cx| {
        if !editor.focus_handle_ref().is_focused(window) {
            cx.propagate();
            return;
        }
        handler(editor, window, cx);
    }))
}

fn on_mut<A: Action>(
    element: Stateful<Div>,
    cx: &Context<'_, NoteEditor>,
    handler: HandlerMut,
) -> Stateful<Div> {
    element.on_action(cx.listener(move |editor, _: &A, window, cx| {
        if !editor.focus_handle_ref().is_focused(window) {
            cx.propagate();
            return;
        }
        handler(editor, window, cx);
    }))
}

fn motions(element: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let element = on_mut::<MoveLeft>(element, cx, |e, w, c| {
        movement::run(e, Motion::Left, Extend::Move, w, c);
    });
    let element = on_mut::<MoveRight>(element, cx, |e, w, c| {
        movement::run(e, Motion::Right, Extend::Move, w, c);
    });
    let element = on_mut::<MoveUp>(element, cx, |e, w, c| {
        movement::run(e, Motion::Up, Extend::Move, w, c);
    });
    let element = on_mut::<MoveDown>(element, cx, |e, w, c| {
        movement::run(e, Motion::Down, Extend::Move, w, c);
    });
    let element = on_mut::<MoveWordLeft>(element, cx, |e, w, c| {
        movement::run(e, Motion::WordLeft, Extend::Move, w, c);
    });
    let element = on_mut::<MoveWordRight>(element, cx, |e, w, c| {
        movement::run(e, Motion::WordRight, Extend::Move, w, c);
    });
    let element = on_mut::<MoveLineStart>(element, cx, |e, w, c| {
        movement::run(e, Motion::LineStart, Extend::Move, w, c);
    });
    let element = on_mut::<MoveLineEnd>(element, cx, |e, w, c| {
        movement::run(e, Motion::LineEnd, Extend::Move, w, c);
    });
    let element = on_mut::<MoveDocStart>(element, cx, |e, w, c| {
        movement::run(e, Motion::DocStart, Extend::Move, w, c);
    });
    let element = on_mut::<MoveDocEnd>(element, cx, |e, w, c| {
        movement::run(e, Motion::DocEnd, Extend::Move, w, c);
    });
    let element = on_mut::<MovePageUp>(element, cx, |e, w, c| {
        movement::run(e, Motion::PageUp, Extend::Move, w, c);
    });
    let element = on_mut::<MovePageDown>(element, cx, |e, w, c| {
        movement::run(e, Motion::PageDown, Extend::Move, w, c);
    });
    let element = on_mut::<SelectLeft>(element, cx, |e, w, c| {
        movement::run(e, Motion::Left, Extend::Select, w, c);
    });
    let element = on_mut::<SelectRight>(element, cx, |e, w, c| {
        movement::run(e, Motion::Right, Extend::Select, w, c);
    });
    let element = on_mut::<SelectUp>(element, cx, |e, w, c| {
        movement::run(e, Motion::Up, Extend::Select, w, c);
    });
    let element = on_mut::<SelectDown>(element, cx, |e, w, c| {
        movement::run(e, Motion::Down, Extend::Select, w, c);
    });
    let element = on_mut::<SelectWordLeft>(element, cx, |e, w, c| {
        movement::run(e, Motion::WordLeft, Extend::Select, w, c);
    });
    let element = on_mut::<SelectWordRight>(element, cx, |e, w, c| {
        movement::run(e, Motion::WordRight, Extend::Select, w, c);
    });
    let element = on_mut::<SelectLineStart>(element, cx, |e, w, c| {
        movement::run(e, Motion::LineStart, Extend::Select, w, c);
    });
    let element = on_mut::<SelectLineEnd>(element, cx, |e, w, c| {
        movement::run(e, Motion::LineEnd, Extend::Select, w, c);
    });
    let element = on_mut::<SelectDocStart>(element, cx, |e, w, c| {
        movement::run(e, Motion::DocStart, Extend::Select, w, c);
    });
    let element = on_mut::<SelectDocEnd>(element, cx, |e, w, c| {
        movement::run(e, Motion::DocEnd, Extend::Select, w, c);
    });
    let element = on_mut::<SelectPageUp>(element, cx, |e, w, c| {
        movement::run(e, Motion::PageUp, Extend::Select, w, c);
    });
    let element = on_mut::<SelectPageDown>(element, cx, |e, w, c| {
        movement::run(e, Motion::PageDown, Extend::Select, w, c);
    });
    let element = on_mut::<SelectParagraphUp>(element, cx, |e, w, c| {
        movement::run(e, Motion::ParagraphUp, Extend::Select, w, c);
    });
    on_mut::<SelectParagraphDown>(element, cx, |e, w, c| {
        movement::run(e, Motion::ParagraphDown, Extend::Select, w, c);
    })
}

fn edits(element: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let element = on::<SelectAll>(element, cx, editing::select_all);
    let element = on::<Backspace>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::Backward, w, c);
    });
    let element = on::<Delete>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::Forward, w, c);
    });
    let element = on::<DeleteWordLeft>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::WordBackward, w, c);
    });
    let element = on::<DeleteWordRight>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::WordForward, w, c);
    });
    let element = on::<DeleteToLineStart>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::LineStart, w, c);
    });
    let element = on::<DeleteToLineEnd>(element, cx, |e, w, c| {
        editing::delete(e, Deletion::LineEnd, w, c);
    });
    let element = on::<Newline>(element, cx, editing::newline);
    let element = on::<SoftBreak>(element, cx, editing::soft_break);
    let element = on::<Indent>(element, cx, editing::indent);
    let element = on::<Outdent>(element, cx, editing::outdent);
    let element = on::<Undo>(element, cx, editing::undo);
    let element = on::<Redo>(element, cx, editing::redo);
    let element = on::<Cut>(element, cx, clipboard::cut);
    let element = on::<Copy>(element, cx, |e, _, c| clipboard::copy(e, c));
    let element = on::<Paste>(element, cx, |e, w, c| {
        clipboard::paste(e, PasteMode::Rich, w, c);
    });
    let element = on::<PasteAsPlainText>(element, cx, |e, w, c| {
        clipboard::paste(e, PasteMode::Plain, w, c);
    });
    let element = on::<DeleteLine>(element, cx, editing::delete_line);
    let element = on_mut::<MoveLineUp>(element, cx, |e, w, c| editing::move_line(e, false, w, c));
    on_mut::<MoveLineDown>(element, cx, |e, w, c| editing::move_line(e, true, w, c))
}

fn formats(element: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let element = on::<ToggleBold>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Bold, w, c);
    });
    let element = on::<ToggleItalic>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Italic, w, c);
    });
    let element = on::<ToggleCode>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Code, w, c);
    });
    let element = on::<ToggleStrike>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Strike, w, c);
    });
    let element = on::<ToggleHighlight>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Highlight, w, c);
    });
    let element = on::<ToggleComment>(element, cx, |e, w, c| {
        formatting::toggle_mark(e, MarkKind::Comment, w, c);
    });
    let element = on::<InsertLink>(element, cx, formatting::insert_link);
    let element = on::<ToggleQuote>(element, cx, formatting::toggle_quote);
    let element = on::<ToggleBulletList>(element, cx, |e, w, c| {
        formatting::toggle_list(e, ListKind::Bullet, w, c);
    });
    let element = on::<ToggleOrderedList>(element, cx, |e, w, c| {
        formatting::toggle_list(e, ListKind::Ordered, w, c);
    });
    let element = on::<ToggleTaskList>(element, cx, |e, w, c| {
        formatting::toggle_list(e, ListKind::Task, w, c);
    });
    let element = on::<ToggleHeading1>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 1, w, c);
    });
    let element = on::<ToggleHeading2>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 2, w, c);
    });
    let element = on::<ToggleHeading3>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 3, w, c);
    });
    let element = on::<ToggleHeading4>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 4, w, c);
    });
    let element = on::<ToggleHeading5>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 5, w, c);
    });
    let element = on::<ToggleHeading6>(element, cx, |e, w, c| {
        formatting::toggle_heading(e, 6, w, c);
    });
    on::<CycleChecklist>(element, cx, formatting::cycle_checklist)
}

fn commands(element: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let element = on_mut::<Cancel>(element, cx, surfaces::escape);
    let element = on_mut::<OpenContextMenu>(element, cx, surfaces::open_context_menu_at_caret);
    let element = on_mut::<OpenSlashMenu>(element, cx, surfaces::open_slash_menu);
    let element = on_mut::<ExportHtml>(element, cx, NoteEditor::export_html);
    let element = on_mut::<FindInNote>(element, cx, |e, w, c| surfaces::open_find(e, false, w, c));
    let element =
        on_mut::<ReplaceInNote>(element, cx, |e, w, c| surfaces::open_find(e, true, w, c));
    let element = on_mut::<FindNextMatch>(element, cx, |e, w, c| find::step(e, true, w, c));
    let element = on_mut::<FindPreviousMatch>(element, cx, |e, w, c| find::step(e, false, w, c));
    let element = on::<SelectMatches>(element, cx, find::select_matches);
    on::<ToggleViewMode>(element, cx, |e, _, c| e.toggle_view_mode(c))
}

pub(crate) fn register(element: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let element = motions(element, cx);
    let element = edits(element, cx);
    let element = formats(element, cx);
    commands(element, cx)
}
