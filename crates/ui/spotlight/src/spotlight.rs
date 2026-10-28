use std::fmt;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Task,
    Window,
};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_workbench::{CurrentVault, OpenRequest, OpenTarget, WorkbenchEvent};

use crate::ask_session::{self, AskSession};
use crate::items::Item;
use crate::query::Query;
use crate::replace::{self, ReplaceOutcome};
use crate::results::Results;
use crate::{citations, fetch, result_rows};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpotlightMode {
    Files,
    FullText,
    Tag(String),
    Ask,
    Replace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotlightEvent {
    Dismissed,
}

pub struct Spotlight {
    open: bool,
    query_input: Entity<InputState>,
    replace_input: Entity<InputState>,
    results: Results,
    ask: AskSession,
    show_replace: bool,
    replacing: bool,
    debounce_task: Task<()>,
    search_task: Task<()>,
    tags_task: Task<()>,
    ask_task: Task<()>,
    replace_task: Task<()>,
    citation_task: Task<()>,
    _subscription: gpui_kit::Subscription,
}

impl fmt::Debug for Spotlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spotlight")
            .field("open", &self.open)
            .field("query", self.results.query())
            .finish_non_exhaustive()
    }
}

impl EventEmitter<WorkbenchEvent> for Spotlight {}

impl EventEmitter<SpotlightEvent> for Spotlight {}

impl Focusable for Spotlight {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query_input.focus_handle(cx)
    }
}

impl Spotlight {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let query_input = cx.new(|cx| InputState::new(window, cx));
        let replace_input = cx.new(|cx| InputState::new(window, cx));
        let subscription = cx.subscribe_in(&query_input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.query_changed(None, window, cx);
            }
        });
        Self {
            open: false,
            query_input,
            replace_input,
            results: Results::default(),
            ask: AskSession::default(),
            show_replace: false,
            replacing: false,
            debounce_task: Task::ready(()),
            search_task: Task::ready(()),
            tags_task: Task::ready(()),
            ask_task: Task::ready(()),
            replace_task: Task::ready(()),
            citation_task: Task::ready(()),
            _subscription: subscription,
        }
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, mode: SpotlightMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.open = true;
        self.replacing = false;
        self.ask.reset();
        self.show_replace = mode == SpotlightMode::Replace;
        let (text, tag) = match mode {
            SpotlightMode::Files | SpotlightMode::FullText | SpotlightMode::Replace => {
                (String::new(), None)
            }
            SpotlightMode::Tag(tag) => {
                let tag = tag.trim_start_matches('#').to_owned();
                (format!("#{tag}"), Some(tag))
            }
            SpotlightMode::Ask => ("?".to_owned(), None),
        };
        let replace_placeholder = t(cx, "spotlight_replace_placeholder");
        self.replace_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(replace_placeholder, window, cx);
        });
        self.query_input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.results.set_query(Query::Empty, None);
        self.query_changed(tag, window, cx);
        self.query_input.focus_handle(cx).focus(window, cx);
    }

    pub(crate) fn dismiss(&mut self, cx: &mut Context<'_, Self>) {
        if !self.open {
            return;
        }
        self.cancel_ask(cx);
        self.open = false;
        cx.emit(SpotlightEvent::Dismissed);
        cx.notify();
    }

    pub(crate) const fn results(&self) -> &Results {
        &self.results
    }

    pub(crate) const fn ask(&self) -> &AskSession {
        &self.ask
    }

    pub(crate) const fn query_input(&self) -> &Entity<InputState> {
        &self.query_input
    }

    pub(crate) const fn replace_input(&self) -> &Entity<InputState> {
        &self.replace_input
    }

    pub(crate) const fn shows_replace(&self) -> bool {
        self.show_replace
    }

    pub(crate) const fn is_replacing(&self) -> bool {
        self.replacing
    }

    pub(crate) fn render_row(
        &self,
        row: usize,
        _: &mut Window,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        result_rows::render(&self.results, row, self.show_replace, cx)
    }

    fn query_changed(
        &mut self,
        selected_tag: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let query = Query::parse(&self.query_input.read(cx).value());
        let was_tags = self.results.query().is_tags();
        let was_ask = self.results.query().is_ask();
        self.results.set_query(query.clone(), selected_tag);
        let placeholder = if query.is_ask() {
            t(cx, "spotlight_ask_placeholder")
        } else {
            t(cx, "spotlight_placeholder")
        };
        self.query_input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
        if query.is_tags() && !was_tags {
            self.load_tags(cx);
        }
        if query.is_ask() && !was_ask {
            self.configure_ask(cx);
        }
        self.debounce_task = match query.search_text() {
            Some(text) => {
                self.results.set_searching();
                fetch::debounced_search(text.to_owned(), cx)
            }
            None => Task::ready(()),
        };
        cx.notify();
    }

    fn results_mut(&mut self) -> &mut Results {
        &mut self.results
    }

    pub(crate) fn run_search(&mut self, text: String, cx: &Context<'_, Self>) {
        self.results.set_searching();
        self.search_task = fetch::search(text, Self::results_mut, cx);
    }

    fn load_tags(&mut self, cx: &Context<'_, Self>) {
        self.results.set_tags_loading();
        self.tags_task = fetch::tags(Self::results_mut, cx);
    }

    fn configure_ask(&mut self, cx: &App) {
        let configured = CurrentVault::get(cx).is_some_and(|services| {
            let config = services.app_settings.read().llm.clone();
            config.is_some_and(|config| services.ask.configure(config).is_ok())
        });
        self.ask.set_configured(configured);
    }

    fn submit_ask(&mut self, cx: &mut Context<'_, Self>) {
        let Query::Ask(question) = self.results.query() else {
            return;
        };
        let question = question.clone();
        let Some(services) = CurrentVault::get(cx) else {
            return;
        };
        let Some(request) = self.ask.begin(&question) else {
            return;
        };
        self.ask_task = ask_session::stream(
            Arc::clone(&services.ask),
            request,
            question,
            |this: &mut Self| &mut this.ask,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn cancel_ask(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(request) = self.ask.cancel() {
            if let Some(services) = CurrentVault::get(cx) {
                services.ask.cancel(&request);
            }
            cx.notify();
        }
    }

    pub(crate) fn select(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        if self.results.selected() != index {
            self.results.select(index);
            cx.notify();
        }
    }

    pub(crate) fn move_selection(&mut self, step: isize, cx: &mut Context<'_, Self>) {
        self.results.move_selection(step);
        cx.notify();
    }

    pub(crate) fn confirm(
        &mut self,
        target: OpenTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.results.query().is_ask() {
            self.submit_ask(cx);
        } else {
            self.activate(self.results.selected(), target, window, cx);
        }
    }

    pub(crate) fn escape(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.ask.is_running() {
            self.cancel_ask(cx);
        } else if self.results.selected_tag().is_some() {
            self.back_to_tags(window, cx);
        } else {
            self.dismiss(cx);
        }
    }

    pub(crate) fn activate(
        &mut self,
        index: usize,
        target: OpenTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(item) = self.results.items().get(index).cloned() else {
            return;
        };
        let request = match item {
            Item::Tag { tag, .. } => {
                self.set_query_text(format!("#{tag}"), Some(tag), window, cx);
                return;
            }
            Item::Content(hit) => {
                let request = OpenRequest::new(hit.path).in_target(target);
                match self.results.query().search_text() {
                    Some(text) => request.at_anchor(text),
                    None => request,
                }
            }
            Item::Name { path, .. } | Item::TagFile(path) => {
                OpenRequest::new(path).in_target(target)
            }
        };
        cx.emit(WorkbenchEvent::Open(request));
        self.dismiss(cx);
    }

    pub(crate) fn back_to_tags(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.set_query_text("#".to_owned(), None, window, cx);
        self.query_input.focus_handle(cx).focus(window, cx);
    }

    fn set_query_text(
        &mut self,
        text: String,
        selected_tag: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.query_input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.query_changed(selected_tag, window, cx);
    }

    pub(crate) fn toggle_replace(&mut self, cx: &mut Context<'_, Self>) {
        let query = self.results.query();
        if !query.is_tags() && !query.is_ask() {
            self.show_replace = !self.show_replace;
            cx.notify();
        }
    }

    pub(crate) fn replace(
        &mut self,
        paths: Vec<RelativePath>,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.results.can_replace() || self.replacing {
            return;
        }
        let (Some(text), Some(services)) = (
            self.results.query().search_text().map(str::to_owned),
            CurrentVault::get(cx),
        ) else {
            return;
        };
        let replacement = self.replace_input.read(cx).value().to_string();
        self.replacing = true;
        self.replace_task = replace::start(services, paths, text, replacement, window, cx);
        cx.notify();
    }

    pub(crate) fn finish_replace(
        &mut self,
        outcome: &ReplaceOutcome,
        single: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.replacing = false;
        for path in outcome.replaced() {
            cx.emit(WorkbenchEvent::Saved(path.clone()));
        }
        replace::report(outcome, single, window, cx);
        if outcome.total() > 0
            && let Some(text) = self.results.query().search_text()
        {
            self.run_search(text.to_owned(), cx);
        }
        cx.notify();
    }

    pub(crate) fn open_citation(
        &mut self,
        url: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.citation_task = citations::open(url, window, cx);
    }
}
