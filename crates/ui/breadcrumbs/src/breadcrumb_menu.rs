use std::ops::Range;

use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, ClickEvent, Context, Div, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement, Render, Role, ScrollStrategy,
    Stateful, StatefulInteractiveElement, Styled, Task, UniformListScrollHandle, Window, div, px,
    uniform_list,
};
use margin_files::{SortBy, TreeRow};
use margin_i18n::t_with;
use margin_kernel::RelativePath;
use margin_theme::css_px;
use margin_workbench::{CurrentVault, OpenRequest, OpenTarget, WorkbenchEvent, run_blocking};

use crate::folder_rows::{matching_rows, parent_index};
use crate::menu_chrome::{menu_empty, menu_footer, menu_header};
use crate::menu_row::{RowState, menu_row};

const MENU_TOP_INSET: f32 = 112.0;

#[derive(Debug)]
pub(crate) struct BreadcrumbMenu {
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    title: String,
    source: RelativePath,
    filter: String,
    focused: usize,
    keyboard: bool,
    expanded: Vec<RelativePath>,
    folder: Option<RelativePath>,
    rows: Option<Vec<TreeRow>>,
    matches: Option<Vec<TreeRow>>,
    rows_task: Task<Option<()>>,
    matches_task: Task<Option<()>>,
}

impl EventEmitter<WorkbenchEvent> for BreadcrumbMenu {}

impl Focusable for BreadcrumbMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BreadcrumbMenu {
    pub(crate) fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            title: String::new(),
            source: RelativePath::root(),
            filter: String::new(),
            focused: 0,
            keyboard: false,
            expanded: Vec::new(),
            folder: None,
            rows: None,
            matches: None,
            rows_task: Task::ready(None),
            matches_task: Task::ready(None),
        }
    }

    pub(crate) fn show(
        &mut self,
        folder: RelativePath,
        title: String,
        source: RelativePath,
        cx: &mut Context<'_, Self>,
    ) {
        self.close();
        self.folder = Some(folder);
        self.title = title;
        self.source = source;
        self.load_rows(self.source.clone(), cx);
        cx.notify();
    }

    pub(crate) fn close(&mut self) {
        self.folder = None;
        self.rows = None;
        self.matches = None;
        self.filter.clear();
        self.expanded.clear();
        self.focused = 0;
        self.rows_task = Task::ready(None);
        self.matches_task = Task::ready(None);
    }

    pub(crate) fn is_filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    pub(crate) fn visible(&self) -> Option<&[TreeRow]> {
        if self.is_filtering() {
            self.matches.as_deref()
        } else {
            self.rows.as_deref()
        }
    }

    fn focused_row(&self) -> Option<TreeRow> {
        self.visible()?.get(self.focused).cloned()
    }

    fn hover(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        if self.focused != index || self.keyboard {
            self.focused = index;
            self.keyboard = false;
            cx.notify();
        }
    }

    fn click(&mut self, index: usize, target: OpenTarget, cx: &mut Context<'_, Self>) {
        self.keyboard = false;
        self.focused = index;
        self.activate(target, cx);
    }

    fn render_rows(&self, range: Range<usize>, cx: &Context<'_, Self>) -> Vec<Stateful<Div>> {
        let Some(rows) = self.visible() else {
            return Vec::new();
        };
        let filtering = self.is_filtering();
        let shown = rows.get(range.clone()).unwrap_or_default();
        let mut elements = Vec::with_capacity(shown.len());
        for (row, index) in shown.iter().zip(range) {
            let active = !row.is_dir && row.path == self.source;
            let state = match (active, index == self.focused, self.keyboard) {
                (true, true, true) => RowState::ActiveKeyboardFocused,
                (true, _, _) => RowState::Active,
                (false, true, true) => RowState::KeyboardFocused,
                (false, true, false) => RowState::Focused,
                (false, false, _) => RowState::Rest,
            };
            let hint = filtering
                .then(|| row.path.parent())
                .flatten()
                .filter(|folder| !folder.is_root());
            let expanded = self.expanded.contains(&row.path);
            let element = menu_row(
                row,
                state,
                expanded,
                hint.as_ref().map(RelativePath::as_str),
                cx,
            )
            .on_hover(cx.listener(move |menu, hovered: &bool, _, cx| {
                if *hovered {
                    menu.hover(index, cx);
                }
            }))
            .on_click(cx.listener(move |menu, event: &ClickEvent, _, cx| {
                let target = if event.modifiers().secondary(cx) {
                    OpenTarget::NewTab
                } else {
                    OpenTarget::ActivePane
                };
                menu.click(index, target, cx);
            }));
            elements.push(element);
        }
        elements
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<'_, Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        match keystroke.key.as_str() {
            "down" => self.move_focus(1),
            "up" => self.move_focus(-1),
            "right" => self.right(cx),
            "left" => self.left(cx),
            "enter" if modifiers.secondary(cx) => self.activate(OpenTarget::NewTab, cx),
            "enter" => self.activate(OpenTarget::ActivePane, cx),
            "backspace" => self.backspace(cx),
            _ => match keystroke.key_char.as_deref() {
                Some(typed)
                    if !modifiers.platform
                        && !modifiers.control
                        && !modifiers.alt
                        && typed.chars().count() == 1 =>
                {
                    self.type_text(typed, cx);
                }
                _ => return,
            },
        }
        self.keyboard = true;
        cx.stop_propagation();
        cx.notify();
    }

    fn move_focus(&mut self, delta: isize) {
        let total = self.visible().map_or(0, <[TreeRow]>::len);
        if total == 0 {
            return;
        }
        self.focused = self.focused.saturating_add_signed(delta).min(total - 1);
        self.scroll
            .scroll_to_item(self.focused, ScrollStrategy::Nearest);
    }

    fn right(&mut self, cx: &mut Context<'_, Self>) {
        let Some(row) = self.focused_row() else {
            return;
        };
        if !row.is_dir {
            return;
        }
        if self.is_filtering() {
            self.drill(row.path, cx);
        } else if self.expanded.contains(&row.path) {
            self.move_focus(1);
        } else {
            self.set_expanded(row.path, true, cx);
        }
    }

    fn left(&mut self, cx: &Context<'_, Self>) {
        if self.is_filtering() {
            return;
        }
        let Some(row) = self.focused_row() else {
            return;
        };
        if row.is_dir && self.expanded.contains(&row.path) {
            self.set_expanded(row.path, false, cx);
        } else if let Some(parent) = self
            .rows
            .as_deref()
            .and_then(|rows| parent_index(rows, self.focused))
        {
            self.focused = parent;
            self.scroll.scroll_to_item(parent, ScrollStrategy::Nearest);
        }
    }

    fn activate(&mut self, target: OpenTarget, cx: &mut Context<'_, Self>) {
        let Some(row) = self.focused_row() else {
            return;
        };
        if !row.is_dir {
            cx.emit(WorkbenchEvent::Open(
                OpenRequest::new(row.path).in_target(target),
            ));
        } else if self.is_filtering() {
            self.drill(row.path, cx);
        } else {
            let open = !self.expanded.contains(&row.path);
            self.set_expanded(row.path, open, cx);
        }
    }

    fn drill(&mut self, folder: RelativePath, cx: &mut Context<'_, Self>) {
        let title = folder.file_name().to_owned();
        let source = self.source.clone();
        self.show(folder, title, source, cx);
    }

    fn type_text(&mut self, typed: &str, cx: &Context<'_, Self>) {
        self.filter.push_str(typed);
        self.focused = 0;
        self.load_matches(cx);
    }

    fn backspace(&mut self, cx: &Context<'_, Self>) {
        if self.filter.pop().is_none() {
            return;
        }
        self.focused = 0;
        if self.is_filtering() {
            self.load_matches(cx);
        } else {
            self.matches = None;
            self.matches_task = Task::ready(None);
        }
    }

    fn set_expanded(&mut self, folder: RelativePath, open: bool, cx: &Context<'_, Self>) {
        if open {
            self.expanded.push(folder.clone());
        } else {
            self.expanded.retain(|expanded| expanded != &folder);
        }
        self.load_rows(folder, cx);
    }

    fn load_rows(&mut self, focus_on: RelativePath, cx: &Context<'_, Self>) {
        let Some(folder) = self.folder.clone() else {
            return;
        };
        let services = CurrentVault::expect(cx);
        let expanded = self.expanded.clone();
        let hidden = services.app_settings.read().attachment_folder();
        let listing = run_blocking(cx, move || {
            services
                .files
                .subtree(&folder, 0, &expanded, SortBy::Name, &[hidden])
        });
        self.rows_task = cx.spawn(async move |menu, cx| {
            let rows = listing.await;
            menu.update(cx, |menu, cx| {
                if let Some(index) = rows.iter().position(|row| row.path == focus_on) {
                    menu.focused = index;
                    menu.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                }
                menu.rows = Some(rows);
                cx.notify();
            })
            .ok()
        });
    }

    fn load_matches(&mut self, cx: &Context<'_, Self>) {
        let Some(folder) = self.folder.clone() else {
            return;
        };
        let services = CurrentVault::expect(cx);
        let filter = self.filter.clone();
        let hidden = services.app_settings.read().attachment_folder();
        let search = run_blocking(cx, move || {
            matching_rows(
                services.files.walk_directory(&folder, false),
                &filter,
                &hidden,
            )
        });
        self.matches_task = cx.spawn(async move |menu, cx| {
            let matches = search.await;
            menu.update(cx, |menu, cx| {
                menu.matches = Some(matches);
                cx.notify();
            })
            .ok()
        });
    }
}

impl Render for BreadcrumbMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let filtering = self.is_filtering();
        let max_height = (window.viewport_size().height - px(MENU_TOP_INSET)).max(px(140.0));
        let count = self.visible().map_or(0, <[TreeRow]>::len);
        let body = if count > 0 {
            uniform_list(
                "breadcrumb-rows",
                count,
                cx.processor(|menu, range, _window, cx| menu.render_rows(range, cx)),
            )
            .track_scroll(&self.scroll)
            .with_sizing_behavior(ListSizingBehavior::Infer)
            .into_any_element()
        } else if self.visible().is_some() {
            menu_empty(filtering, cx).into_any_element()
        } else {
            div().into_any_element()
        };
        v_flex()
            .id("breadcrumb-menu")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|menu, event: &KeyDownEvent, _, cx| {
                menu.on_key_down(event, cx);
            }))
            .role(Role::Tree)
            .aria_label(t_with(
                cx,
                "breadcrumb_folder_contents",
                &[("name", self.title.as_str())],
            ))
            .w(css_px(288.0))
            .max_h(max_height)
            .child(menu_header(&self.title, &self.filter, cx))
            .child(v_flex().flex_1().min_h_0().py_1().child(body))
            .when(!filtering, |menu| menu.child(menu_footer(cx)))
    }
}
