use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::v_flex;
use gpui_kit::{
    App, Context, EventEmitter, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    Render, Role, ScrollStrategy, StatefulInteractiveElement, Styled, Task,
    UniformListScrollHandle, Window, div, uniform_list,
};
use margin_editor_commands::slash_items;
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, css_px};
use margin_workbench::{CurrentVault, run_blocking};

use crate::completion::{Completion, find_note, heading_items, link_items, slash_item, tag_items};
use crate::completion_item::CompletionItem;
use crate::note_headings::note_headings;
use crate::trigger::{Trigger, TriggerKind};

const MENU_WIDTH: f32 = 268.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionEvent {
    Confirm(Completion),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    Files,
    Tags,
    Headings(RelativePath),
}

#[derive(Debug, Default)]
struct Sources {
    root: Option<PathBuf>,
    files: Option<Arc<[RelativePath]>>,
    tags: Option<Arc<[String]>>,
    headings: HashMap<RelativePath, Arc<[String]>>,
}

#[derive(Debug)]
pub struct CompletionMenu {
    trigger: Option<Trigger>,
    items: Vec<CompletionItem>,
    selected: usize,
    pending: bool,
    scroll: UniformListScrollHandle,
    sources: Sources,
    loading: Option<(Source, Task<Option<()>>)>,
    filtering: Task<Option<()>>,
}

impl EventEmitter<CompletionEvent> for CompletionMenu {}

impl CompletionMenu {
    #[must_use]
    pub fn new(_cx: &mut Context<'_, Self>) -> Self {
        Self {
            trigger: None,
            items: Vec::new(),
            selected: 0,
            pending: false,
            scroll: UniformListScrollHandle::new(),
            sources: Sources::default(),
            loading: None,
            filtering: Task::ready(None),
        }
    }

    pub fn set_trigger(&mut self, trigger: Trigger, cx: &mut Context<'_, Self>) {
        if self.trigger.as_ref() == Some(&trigger) {
            return;
        }
        self.trigger = Some(trigger);
        self.refilter(cx);
    }

    #[must_use]
    pub const fn trigger(&self) -> Option<&Trigger> {
        self.trigger.as_ref()
    }

    pub fn dismiss(&mut self, cx: &mut Context<'_, Self>) {
        self.trigger = None;
        self.items.clear();
        self.selected = 0;
        self.pending = false;
        self.filtering = Task::ready(None);
        cx.notify();
    }

    pub fn refresh_sources(&mut self, cx: &mut Context<'_, Self>) {
        self.sources = Sources::default();
        self.loading = None;
        self.refilter(cx);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    pub fn select_next(&mut self, cx: &mut Context<'_, Self>) {
        self.step(1, cx);
    }

    pub fn select_previous(&mut self, cx: &mut Context<'_, Self>) {
        self.step(-1, cx);
    }

    #[must_use]
    pub fn confirm(&self) -> Option<Completion> {
        if self.pending {
            return None;
        }
        self.items
            .get(self.selected)
            .map(|item| item.completion().clone())
    }

    pub(crate) fn click(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        self.selected = index;
        if let Some(completion) = self.confirm() {
            cx.emit(CompletionEvent::Confirm(completion));
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<'_, Self>) {
        let total = self.items.len();
        if total == 0 {
            return;
        }
        let total = isize::try_from(total).expect("a completion list fits in isize");
        let current = isize::try_from(self.selected).expect("a completion index fits in isize");
        self.selected = usize::try_from((current + delta).rem_euclid(total))
            .expect("rem_euclid of a positive modulus is never negative");
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn show(&mut self, items: Vec<CompletionItem>, cx: &mut Context<'_, Self>) {
        self.items = items;
        self.selected = 0;
        self.pending = false;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn sync_vault(&mut self, cx: &App) {
        let root = CurrentVault::get(cx).map(|vault| vault.root.clone());
        if self.sources.root != root {
            self.sources = Sources {
                root,
                ..Sources::default()
            };
            self.loading = None;
        }
    }

    fn refilter(&mut self, cx: &mut Context<'_, Self>) {
        let Some(trigger) = self.trigger.clone() else {
            return;
        };
        self.sync_vault(cx);
        self.filtering = Task::ready(None);
        if trigger.kind != TriggerKind::Slash && self.sources.root.is_none() {
            self.show(Vec::new(), cx);
            return;
        }
        match &trigger.kind {
            TriggerKind::Slash => {
                let items = slash_items(&trigger.query, |key| t(cx, key))
                    .into_iter()
                    .map(|item| {
                        let label = t(cx, item.label_key).to_string();
                        let description = t(cx, item.description_key).to_string();
                        slash_item(&trigger, item.command, label, description)
                    })
                    .collect();
                self.show(items, cx);
            }
            TriggerKind::Tag => match self.sources.tags.clone() {
                Some(tags) => self.show(tag_items(&tags, &trigger), cx),
                None => self.load(Source::Tags, cx),
            },
            TriggerKind::WikiLink { .. } => match self.sources.files.clone() {
                Some(files) => self.filter_links(files, trigger.clone(), cx),
                None => self.load(Source::Files, cx),
            },
            TriggerKind::Heading { note, .. } => {
                let Some(files) = self.sources.files.clone() else {
                    self.load(Source::Files, cx);
                    return;
                };
                let Some(path) = find_note(&files, note).cloned() else {
                    self.show(Vec::new(), cx);
                    return;
                };
                match self.sources.headings.get(&path).cloned() {
                    Some(headings) => self.show(heading_items(&headings, &trigger), cx),
                    None => self.load(Source::Headings(path), cx),
                }
            }
        }
    }

    fn filter_links(
        &mut self,
        files: Arc<[RelativePath]>,
        trigger: Trigger,
        cx: &Context<'_, Self>,
    ) {
        self.pending = true;
        let filtered = run_blocking(cx, move || link_items(&files, &trigger));
        self.filtering = cx.spawn(async move |menu, cx| {
            let items = filtered.await;
            menu.update(cx, |menu, cx| menu.show(items, cx)).ok()
        });
    }

    fn load(&mut self, source: Source, cx: &Context<'_, Self>) {
        if self
            .loading
            .as_ref()
            .is_some_and(|(loading, _)| *loading == source)
        {
            return;
        }
        let Some(vault) = CurrentVault::get(cx) else {
            return;
        };
        self.pending = true;
        let root = self.sources.root.clone();
        let wanted = source.clone();
        let task = match source {
            Source::Files => {
                let walk = run_blocking(cx, move || {
                    vault
                        .files
                        .walk_directory(&RelativePath::root(), false)
                        .into_iter()
                        .filter(|entry| !entry.is_dir)
                        .map(|entry| entry.path)
                        .collect::<Arc<[RelativePath]>>()
                });
                cx.spawn(async move |menu, cx| {
                    let files = walk.await;
                    menu.update(cx, |menu, cx| {
                        menu.loaded(root.as_ref(), cx, |sources| {
                            sources.files = Some(files);
                        });
                    })
                    .ok()
                })
            }
            Source::Tags => {
                let query = run_blocking(cx, move || {
                    vault.search.tags().map_or_else(
                        |_| Arc::from([]),
                        |tags| {
                            tags.into_iter()
                                .map(|info| info.tag)
                                .collect::<Arc<[String]>>()
                        },
                    )
                });
                cx.spawn(async move |menu, cx| {
                    let tags = query.await;
                    menu.update(cx, |menu, cx| {
                        menu.loaded(root.as_ref(), cx, |sources| {
                            sources.tags = Some(tags);
                        });
                    })
                    .ok()
                })
            }
            Source::Headings(path) => {
                let note = path.clone();
                let read = run_blocking(cx, move || {
                    vault.files.read_file(&note).map_or_else(
                        |_| Arc::from([]),
                        |bytes| Arc::from(note_headings(&String::from_utf8_lossy(&bytes))),
                    )
                });
                cx.spawn(async move |menu, cx| {
                    let headings = read.await;
                    menu.update(cx, |menu, cx| {
                        menu.loaded(root.as_ref(), cx, |sources| {
                            sources.headings.insert(path, headings);
                        });
                    })
                    .ok()
                })
            }
        };
        self.loading = Some((wanted, task));
    }

    fn loaded(
        &mut self,
        root: Option<&PathBuf>,
        cx: &mut Context<'_, Self>,
        store: impl FnOnce(&mut Sources),
    ) {
        self.loading = None;
        if self.sources.root.as_ref() == root {
            store(&mut self.sources);
        }
        self.refilter(cx);
    }
}

impl Render for CompletionMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let body = if !self.items.is_empty() {
            uniform_list(
                "completion-items",
                self.items.len(),
                cx.processor(|menu, range: Range<usize>, _window, cx| {
                    let mut rows = Vec::with_capacity(range.len());
                    for index in range {
                        if let Some(item) = menu.items.get(index) {
                            rows.push(item.row(index, index == menu.selected, cx));
                        }
                    }
                    rows
                }),
            )
            .track_scroll(&self.scroll)
            .with_sizing_behavior(ListSizingBehavior::Infer)
            .into_any_element()
        } else if self.pending {
            div().into_any_element()
        } else {
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(palette.text_tertiary)
                .child(t(cx, "completion_no_matches"))
                .into_any_element()
        };
        v_flex()
            .id("completion-menu")
            .role(Role::ListBox)
            .w(css_px(MENU_WIDTH))
            .max_h(css_px(320.0))
            .p(css_px(4.0))
            .bg(palette.surface(0))
            .border_1()
            .border_color(palette.border_secondary)
            .rounded(css_px(12.0))
            .shadow_lg()
            .overflow_hidden()
            .child(body)
    }
}
