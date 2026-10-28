use std::fmt;

use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render, Styled,
    Window, div, relative,
};
use margin_markdown::Frontmatter;
use margin_theme::MarginTheme;

use crate::effect::{Effect, FocusAfter, FocusTarget};
use crate::panel_actions::bind_keys_once;
use crate::panel_view::{add_button, em, heading, pending_view, row_view};
use crate::pending_row::PendingRow;
use crate::pills::Pills;
use crate::properties_event::PropertiesEvent;
use crate::property_edits::{self, SortOrder};
use crate::property_link::LinkTarget;
use crate::property_row::Row;
use crate::property_rows::Rows;

struct FieldError {
    key: Option<String>,
    message: &'static str,
}

pub struct PropertiesPanel {
    heading: FocusHandle,
    add: FocusHandle,
    source: String,
    frontmatter: Option<Frontmatter>,
    rows: Rows,
    rows_stale: bool,
    collapsed: bool,
    pending: Option<PendingRow>,
    error: Option<FieldError>,
}

impl fmt::Debug for PropertiesPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertiesPanel")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<PropertiesEvent> for PropertiesPanel {}

impl Focusable for PropertiesPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.heading.clone()
    }
}

impl PropertiesPanel {
    pub fn new(_: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        bind_keys_once(cx);
        Self {
            heading: cx.focus_handle().tab_stop(true),
            add: cx.focus_handle().tab_stop(true),
            source: String::new(),
            frontmatter: None,
            rows: Rows::default(),
            rows_stale: false,
            collapsed: false,
            pending: None,
            error: None,
        }
    }

    pub fn set_source(&mut self, note_source: &str, cx: &mut Context<'_, Self>) {
        if note_source == self.source {
            return;
        }
        note_source.clone_into(&mut self.source);
        self.frontmatter = property_edits::frontmatter_of(note_source);
        self.rows_stale = true;
        cx.notify();
    }

    #[must_use]
    pub fn has_properties(&self) -> bool {
        self.pending.is_some()
            || self.frontmatter.as_ref().is_some_and(|frontmatter| {
                !frontmatter.unsupported && !frontmatter.properties.is_empty()
            })
    }

    pub fn start_adding(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let unsupported = self
            .frontmatter
            .as_ref()
            .is_some_and(|front| front.unsupported);
        if unsupported {
            return;
        }
        self.collapsed = false;
        match &self.pending {
            Some(pending) => pending.focus(window, cx),
            None => self.pending = Some(PendingRow::new(window, cx)),
        }
        cx.notify();
    }

    pub fn focus_add(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus(FocusTarget::Add, window, cx);
    }

    pub(crate) fn commit_new(
        &mut self,
        after: FocusAfter,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let key = self
            .pending
            .as_ref()
            .map(|pending| pending.key(cx))
            .unwrap_or_default();
        if let Some(message) = property_edits::key_problem(&self.source, &key, None) {
            self.error = Some(FieldError { key: None, message });
            cx.notify();
            return false;
        }
        self.pending = None;
        let then = (after == FocusAfter::Value).then(|| FocusTarget::Value(key.clone()));
        self.edit(property_edits::add(&self.source, &key), then, window, cx);
        true
    }

    pub(crate) fn leave_new(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let empty = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.key(cx).is_empty());
        if empty || !self.commit_new(FocusAfter::Nothing, window, cx) {
            self.dismiss_new(FocusAfter::Nothing, window, cx);
        }
    }

    pub(crate) fn dismiss_new(
        &mut self,
        after: FocusAfter,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.pending.take().is_none() {
            return;
        }
        self.error = None;
        if after != FocusAfter::Nothing {
            self.add_up(window, cx);
        }
        cx.notify();
    }

    fn edit(
        &mut self,
        next: Option<String>,
        then: Option<FocusTarget>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.error = None;
        if let Some(next) = next.filter(|next| *next != self.source) {
            self.frontmatter = property_edits::frontmatter_of(&next);
            self.source.clone_from(&next);
            cx.emit(PropertiesEvent::Changed(next));
        }
        self.rows.sync(self.frontmatter.as_ref(), window, cx);
        self.rows_stale = false;
        if let Some(target) = then {
            self.focus(target, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn update_row(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        update: impl FnOnce(&mut Row, &mut Window, &mut Context<'_, Self>) -> Effect,
    ) {
        let Some(row) = self.rows.find_mut(key) else {
            return;
        };
        let effect = update(row, window, cx);
        self.apply_effect(Some(key), effect, window, cx);
        cx.notify();
    }

    pub(crate) fn update_pills(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        update: impl FnOnce(&mut Pills, &mut Window, &mut Context<'_, Self>) -> Effect,
    ) {
        self.update_row(key, window, cx, |row, window, cx| {
            row.pills_mut()
                .map_or(Effect::Nothing, |pills| update(pills, window, cx))
        });
    }

    pub(crate) fn apply_effect(
        &mut self,
        key: Option<&str>,
        effect: Effect,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let key = key.unwrap_or_default().to_owned();
        let source = &self.source;
        match effect {
            Effect::Nothing => {}
            Effect::Commit(value, then) => {
                let next = property_edits::set_value(source, &key, &value);
                self.edit(next, then, window, cx);
            }
            Effect::Rename(next, then) => {
                if let Some(message) = property_edits::key_problem(source, &next, Some(&key)) {
                    if let Some(row) = self.rows.find(&key) {
                        row.reset_key(window, cx);
                    }
                    self.error = Some(FieldError {
                        key: Some(key),
                        message,
                    });
                    return;
                }
                let renamed = property_edits::rename(source, &key, &next);
                self.rows.carry_kind(&key, &next);
                self.edit(renamed, then, window, cx);
            }
            Effect::ChangeKind(kind) => {
                if let Some(next) = property_edits::change_kind(source, &key, kind) {
                    self.rows.choose_kind(&key, kind);
                    self.edit(Some(next), Some(FocusTarget::Row(key)), window, cx);
                }
            }
            Effect::Remove => self.remove(&[key.as_str()], FocusAfter::Row, window, cx),
            Effect::Focus(target) => self.focus(target, window, cx),
            Effect::OpenLink(LinkTarget::WikiLink(target)) => {
                cx.emit(PropertiesEvent::OpenWikiLink(target));
            }
            Effect::OpenLink(LinkTarget::Url(url)) => cx.emit(PropertiesEvent::OpenUrl(url)),
            Effect::OpenTag(tag) => cx.emit(PropertiesEvent::TagSelected(tag)),
            Effect::Invalid(message) => {
                self.error = Some(FieldError {
                    key: Some(key),
                    message,
                });
            }
        }
    }

    fn remove(
        &mut self,
        keys: &[&str],
        after: FocusAfter,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let then = (after != FocusAfter::Nothing).then(|| self.rows.neighbour_after_removing(keys));
        if let Some(next) = property_edits::remove(&self.source, keys) {
            self.edit(Some(next), then, window, cx);
        }
    }

    pub(crate) fn sort(
        &mut self,
        order: SortOrder,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let next = property_edits::sort(&self.source, order);
        self.edit(next, None, window, cx);
    }

    pub(crate) fn clear(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let keys = self.rows.keys();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.remove(&keys, FocusAfter::Nothing, window, cx);
    }

    pub(crate) fn fold(
        &mut self,
        collapsed: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.collapsed = collapsed;
        self.heading.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn focus(
        &mut self,
        target: FocusTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match target {
            FocusTarget::Add if self.collapsed => self.heading.focus(window, cx),
            FocusTarget::Add => self.add.focus(window, cx),
            FocusTarget::New => self.start_adding(window, cx),
            FocusTarget::Body => cx.emit(PropertiesEvent::FocusBody),
            target => self.rows.focus(&target, window, cx),
        }
    }

    pub(crate) fn row_at(
        &mut self,
        index: usize,
        offset: isize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let target = index.checked_add_signed(offset);
        if let Some(row) = target.and_then(|target| self.rows.get(target)) {
            row.focus_handle().focus(window, cx);
        } else if target == Some(self.rows.len()) && self.pending.is_some() {
            self.start_adding(window, cx);
        } else if offset < 0 {
            self.heading.focus(window, cx);
        } else {
            self.focus(FocusTarget::Add, window, cx);
        }
    }

    pub(crate) fn heading_down(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.collapsed {
            cx.emit(PropertiesEvent::FocusBody);
        } else {
            self.row_at(0, 0, window, cx);
        }
    }

    pub(crate) fn add_up(&self, window: &mut Window, cx: &mut App) {
        match self.rows.last() {
            Some(row) => row.focus_handle().focus(window, cx),
            None => self.heading.focus(window, cx),
        }
    }

    fn error_for(&self, key: Option<&str>) -> Option<&'static str> {
        self.error
            .as_ref()
            .filter(|error| error.key.as_deref() == key)
            .map(|error| error.message)
    }
}

impl Render for PropertiesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.rows_stale {
            self.rows_stale = false;
            self.rows.sync(self.frontmatter.as_ref(), window, cx);
        }
        if !self.has_properties() {
            return div();
        }
        let palette = MarginTheme::palette(cx).clone();
        let root = div()
            .flex()
            .flex_col()
            .w_full()
            .pt(em(0.25))
            .pb(em(2.3))
            .text_size(em(1.0))
            .line_height(relative(1.5))
            .text_color(palette.text_secondary)
            .child(heading(&self.heading, self.collapsed, &palette, window, cx));
        if self.collapsed {
            return root;
        }
        let mut list = div().flex().flex_col().gap(em(0.2));
        for (index, row) in self.rows.iter().enumerate() {
            let error = self.error_for(Some(row.key()));
            list = list.child(row_view(row, index, error, &palette, window, cx));
        }
        if let Some(pending) = &self.pending {
            let error = self.error_for(None);
            list = list.child(pending_view(pending.input(), error, &palette, cx));
        }
        let add = add_button(&self.add, &palette, window, cx);
        root.child(list).child(div().flex().child(add))
    }
}
