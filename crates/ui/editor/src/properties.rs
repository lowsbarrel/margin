use gpui_kit::{Context, Window};
use margin_markdown::{Span, TextEdit};
use margin_properties::PropertiesEvent;
use margin_text_buffer::{Change, EditOrigin, Selection};
use margin_workbench::WorkbenchEvent;

use crate::links;
use crate::note_editor::{Edited, NoteEditor};
use crate::pointer::OpenIn;

pub(crate) fn minimal_edit(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let prefix = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, a), b)| a != b)
        .map_or_else(|| old.len().min(new.len()), |((index, _), _)| index);
    let old_rest = &old[prefix..];
    let new_rest = &new[prefix..];
    let suffix = old_rest
        .chars()
        .rev()
        .zip(new_rest.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum::<usize>();
    let suffix = suffix.min(old_rest.len()).min(new_rest.len());
    Some(TextEdit::new(
        Span::new(prefix, old.len() - suffix),
        &new[prefix..new.len() - suffix],
    ))
}

pub(crate) fn on_event(
    editor: &mut NoteEditor,
    event: &PropertiesEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match event {
        PropertiesEvent::Changed(source) => {
            if let Some(edit) = minimal_edit(editor.session().text(), source) {
                editor.apply(Change::new(vec![edit]), EditOrigin::Command, window, cx);
            }
        }
        PropertiesEvent::OpenWikiLink(target) => {
            let link = margin_markdown::WikiLink {
                target: target.clone(),
                heading: None,
                block: None,
                alias: None,
            };
            links::open_wiki(editor, &link, OpenIn::Current, window, cx);
        }
        PropertiesEvent::OpenUrl(url) => {
            links::open_destination(editor, url, OpenIn::Current, window, cx);
        }
        PropertiesEvent::TagSelected(tag) => cx.emit(WorkbenchEvent::TagSelected(tag.clone())),
        PropertiesEvent::FocusBody => {
            let document = editor.session().document();
            let start = document.frontmatter().map_or(0, |frontmatter| {
                let line = document.line_of(frontmatter.span.end.saturating_sub(1)) + 1;
                document.line_span(line).start
            });
            editor.session_mut().select(Selection::cursor(start));
            let focus = editor.focus_handle_ref().clone();
            window.focus(&focus, cx);
            editor.edited(Edited::Selection, window, cx);
        }
        PropertiesEvent::FocusTitle => editor.title_row().focus(window, cx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_edit_keeps_common_prefix_and_suffix() {
        let edit = minimal_edit("---\na: 1\n---\nbody", "---\na: 2\n---\nbody").expect("differs");
        assert_eq!(edit.span, Span::new(7, 8));
        assert_eq!(edit.insert, "2");
        assert_eq!(minimal_edit("same", "same"), None);
        let grow = minimal_edit("aa", "aaa").expect("differs");
        assert_eq!(grow.applied("aa"), "aaa");
    }
}
