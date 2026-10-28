use gpui_kit::{AppContext, Context, Entity, Window};
use margin_find_bar::{
    FindBar, FindEvent, FindMode, find_matches, nearest_match, next_match, previous_match,
    replace_all, replace_current,
};
use margin_markdown::Span;
use margin_text_buffer::{EditOrigin, Selection};

use crate::note_editor::{Edited, NoteEditor};

#[derive(Default)]
pub(crate) struct FindState {
    bar: Option<Entity<FindBar>>,
    matches: Vec<Span>,
    current: Option<usize>,
    _subscription: Option<gpui_kit::Subscription>,
}

impl FindState {
    pub(crate) fn highlights(&self) -> (Vec<Span>, Option<Span>) {
        if self.bar.is_none() {
            return (Vec::new(), None);
        }
        let current = self
            .current
            .and_then(|index| self.matches.get(index).copied());
        (self.matches.clone(), current)
    }

    pub(crate) const fn bar(&self) -> Option<&Entity<FindBar>> {
        self.bar.as_ref()
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.bar.is_some()
    }

    fn close(&mut self) {
        *self = Self::default();
    }
}

pub(crate) fn open(
    editor: &mut NoteEditor,
    replace: bool,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let mode = if replace {
        FindMode::Replace
    } else {
        FindMode::Find
    };
    let bar = if let Some(bar) = editor.surfaces().find().bar() {
        bar.clone()
    } else {
        let bar = cx.new(|cx| FindBar::new(window, cx));
        let subscription =
            cx.subscribe_in(&bar, window, |editor, _, event: &FindEvent, window, cx| {
                on_event(editor, event, window, cx);
            });
        *editor.surfaces_mut().find_mut() = FindState {
            bar: Some(bar.clone()),
            _subscription: Some(subscription),
            ..FindState::default()
        };
        bar
    };
    bar.update(cx, |bar, cx| {
        bar.set_mode(mode, window, cx);
        bar.focus(window, cx);
    });
    refresh(editor, cx);
}

pub(crate) fn refresh(editor: &mut NoteEditor, cx: &mut Context<'_, NoteEditor>) {
    let Some(bar) = editor.surfaces().find().bar().cloned() else {
        return;
    };
    let query = bar.read(cx).query(cx);
    let matches = find_matches(editor.session().text(), &query);
    let current = nearest_match(&matches, editor.session().primary());
    let total = matches.len();
    let find = editor.surfaces_mut().find_mut();
    find.matches = matches;
    find.current = current;
    bar.update(cx, |bar, cx| bar.set_status(current, total, cx));
}

fn select_match(
    editor: &mut NoteEditor,
    index: Option<usize>,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let Some(index) = index else {
        return;
    };
    let Some(found) = editor.surfaces().find().matches.get(index).copied() else {
        return;
    };
    editor
        .session_mut()
        .select(Selection::new(found.start, found.end));
    editor.edited(Edited::Selection, window, cx);
}

pub(crate) fn close(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    editor.surfaces_mut().find_mut().close();
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
    cx.notify();
}

pub(crate) fn step(
    editor: &mut NoteEditor,
    forward: bool,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if editor.surfaces().find().is_open() {
        let event = if forward {
            FindEvent::Next
        } else {
            FindEvent::Previous
        };
        on_event(editor, &event, window, cx);
    }
}

pub(crate) fn select_matches(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let session = editor.session();
    let primary = session.primary();
    let text = session.text();
    let needle = primary.span().slice(text).to_owned();
    if needle.is_empty() {
        return;
    }
    let mut selections = Vec::new();
    let mut main = 0;
    for (at, _) in text.match_indices(needle.as_str()) {
        if at == primary.start() {
            main = selections.len();
        }
        selections.push(Selection::new(at, at + needle.len()));
    }
    editor.session_mut().set_selections(selections, main);
    editor.edited(Edited::Selection, window, cx);
}

fn on_event(
    editor: &mut NoteEditor,
    event: &FindEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let Some(bar) = editor.surfaces().find().bar().cloned() else {
        return;
    };
    let query = bar.read(cx).query(cx);
    match event {
        FindEvent::QueryChanged => refresh(editor, cx),
        FindEvent::Next => {
            let index = next_match(
                &editor.surfaces().find().matches,
                editor.session().primary(),
            );
            select_match(editor, index, window, cx);
        }
        FindEvent::Previous => {
            let index = previous_match(
                &editor.surfaces().find().matches,
                editor.session().primary(),
            );
            select_match(editor, index, window, cx);
        }
        FindEvent::ReplaceOne(replacement) => {
            let session = editor.session();
            if let Some(change) =
                replace_current(session.text(), &query, session.primary(), replacement)
            {
                editor.apply(change, EditOrigin::Command, window, cx);
            }
        }
        FindEvent::ReplaceAll(replacement) => {
            if let Some(change) = replace_all(editor.session().text(), &query, replacement) {
                editor.apply(change, EditOrigin::Command, window, cx);
            }
        }
        FindEvent::Close => {
            close(editor, window, cx);
            return;
        }
    }
    refresh(editor, cx);
    cx.notify();
}
