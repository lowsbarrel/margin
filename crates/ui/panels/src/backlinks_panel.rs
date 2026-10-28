use std::fmt;

use gpui_kit::assets::IconName;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, div,
};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::{CurrentVault, OpenRequest, VaultServices, WorkbenchEvent, run_blocking};

use crate::list_key::{self, ListKey};
use crate::panel_event::PanelEvent;
use crate::panel_header::{icon_button, panel_header, panel_placeholder};

#[derive(Clone, Debug, PartialEq, Eq)]
struct BacklinkRow {
    path: RelativePath,
    title: SharedString,
    folder: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Loading,
    Ready,
    Failed,
}

pub struct BacklinksPanel {
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    note: Option<RelativePath>,
    rows: Vec<BacklinkRow>,
    status: Status,
    selected: Option<usize>,
    revision: u64,
    load: Task<()>,
}

impl BacklinksPanel {
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "every panel is built through the Shell's window-and-context constructor"
    )]
    pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            note: None,
            rows: Vec::new(),
            status: Status::Ready,
            selected: None,
            revision: 0,
            load: Task::ready(()),
        }
    }

    pub fn set_note(&mut self, path: Option<RelativePath>, cx: &mut Context<'_, Self>) {
        if self.note == path {
            return;
        }
        self.note = path;
        self.rows.clear();
        self.selected = None;
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<'_, Self>) {
        self.revision += 1;
        let Some(note) = self.note.clone() else {
            self.rows.clear();
            self.status = Status::Ready;
            cx.notify();
            return;
        };
        self.status = Status::Loading;
        let revision = self.revision;
        let services = CurrentVault::expect(cx);
        let work = run_blocking(cx, move || load_rows(&services, &note));
        self.load = cx.spawn(async move |this, cx| {
            let rows = work.await;
            let _released = this.update(cx, |panel, cx| {
                if panel.revision != revision {
                    return;
                }
                match rows {
                    Ok(rows) => {
                        panel.selected = panel.selected.filter(|index| *index < rows.len());
                        panel.rows = rows;
                        panel.status = Status::Ready;
                    }
                    Err(_) => panel.status = Status::Failed,
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    fn open(&self, index: usize, cx: &mut Context<'_, Self>) {
        if let Some(row) = self.rows.get(index) {
            cx.emit(WorkbenchEvent::Open(OpenRequest::new(row.path.clone())));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(key) = ListKey::of(event, cx) else {
            return;
        };
        match key {
            ListKey::Close => cx.emit(PanelEvent::Close),
            ListKey::Activate => {
                if let Some(index) = self.selected {
                    self.open(index, cx);
                }
            }
            ListKey::Previous | ListKey::Next => {
                self.selected = list_key::step(self.selected, self.rows.len(), key);
                if let Some(index) = self.selected {
                    self.scroll.scroll_to_item(index);
                }
                cx.notify();
            }
            ListKey::Delete | ListKey::Commit => return,
        }
        cx.stop_propagation();
    }

    fn render_row(
        &self,
        index: usize,
        row: &BacklinkRow,
        focused: bool,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement + use<> {
        let palette = MarginTheme::palette(cx).clone();
        let selected = self.selected == Some(index);
        let fill = palette.brand(if focused { 32.0 } else { 16.0 });
        let hover = palette.surface(3);
        let hover_text = palette.text_primary;
        v_flex()
            .id(("backlink", index))
            .w_full()
            .gap_0p5()
            .p_2()
            .rounded_sm()
            .text_ui_sm()
            .text_color(palette.text_secondary)
            .map(|row| {
                if selected {
                    row.bg(fill).text_color(palette.text_primary)
                } else {
                    row.hover(move |style| style.bg(hover).text_color(hover_text))
                }
            })
            .child(div().w_full().truncate().child(row.title.clone()))
            .when_some(row.folder.clone(), |element, folder| {
                element.child(
                    div()
                        .w_full()
                        .truncate()
                        .text_ui_xs()
                        .text_color(palette.text_tertiary)
                        .child(folder),
                )
            })
            .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| {
                panel.selected = None;
                panel.open(index, cx);
                cx.notify();
            }))
    }
}

fn load_rows(
    services: &VaultServices,
    note: &RelativePath,
) -> Result<Vec<BacklinkRow>, margin_search::Error> {
    let links = services.search.backlinks(note)?;
    Ok(links
        .into_iter()
        .map(|link| {
            let title = link
                .name
                .strip_suffix(".md")
                .unwrap_or(&link.name)
                .to_owned();
            let folder = link
                .path
                .parent()
                .filter(|parent| !parent.is_root())
                .map(|parent| SharedString::from(parent.as_str().to_owned()));
            BacklinkRow {
                title: title.into(),
                folder,
                path: link.path,
            }
        })
        .collect())
}

impl fmt::Debug for BacklinksPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BacklinksPanel").finish_non_exhaustive()
    }
}

impl EventEmitter<WorkbenchEvent> for BacklinksPanel {}

impl EventEmitter<PanelEvent> for BacklinksPanel {}

impl Focusable for BacklinksPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for BacklinksPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx).clone();
        let close = icon_button("backlinks-close", IconName::X, t(cx, "backlinks_close"))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(PanelEvent::Close)));
        let header = panel_header(
            IconName::Link2,
            &t(cx, "backlinks_title"),
            self.rows.len(),
            [close],
            cx,
        );
        let body: Vec<AnyElement> = match (self.status, self.rows.is_empty()) {
            (Status::Failed, _) => {
                vec![panel_placeholder(t(cx, "backlinks_load_failed"), cx).into_any_element()]
            }
            (Status::Loading, true) => {
                vec![panel_placeholder(t(cx, "backlinks_loading"), cx).into_any_element()]
            }
            (_, true) => vec![panel_placeholder(t(cx, "backlinks_empty"), cx).into_any_element()],
            (_, false) => {
                let focused = self.focus_handle.contains_focused(window, cx);
                self.rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| self.render_row(index, row, focused, cx).into_any_element())
                    .collect()
            }
        };
        v_flex()
            .id("backlinks-panel")
            .key_context("BacklinksPanel")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .bg(palette.surface(1))
            .child(header)
            .child(
                div()
                    .id("backlinks-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p_2()
                    .children(body),
            )
    }
}
