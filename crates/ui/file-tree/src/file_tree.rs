use std::path::Path;
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{
    App, Context, FocusHandle, Focusable, ScrollStrategy, Task, UniformListScrollHandle, Window,
};
use margin_files::{SortBy, TreeRow};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_workbench::{
    AppServices, CurrentVault, OpenRequest, Platform, VaultServices, WorkbenchEvent, notify_error,
    run_blocking,
};

use crate::clipboard::Clipboard;
use crate::command::{self, Command, NewEntry};
use crate::inline_edit::{Decision, EditTarget, InlineEdit};
use crate::model::{Effect, TreeModel};
use crate::operations::{HostHandle, Job, Report};
use crate::selection::SelectedEntry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Followup {
    Nothing,
    OpenCreated,
    FinishRename,
}

pub struct FileTree {
    services: Arc<VaultServices>,
    model: TreeModel,
    edit: Option<InlineEdit>,
    clipboard: Clipboard,
    keyboard: bool,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    rebuild: Task<()>,
}

impl Focusable for FileTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl FileTree {
    pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        command::bind_keys_once(cx);
        let mut tree = Self {
            services: CurrentVault::expect(cx),
            model: TreeModel::new(),
            edit: None,
            clipboard: Clipboard::default(),
            keyboard: false,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            rebuild: Task::ready(()),
        };
        tree.refresh(cx);
        tree
    }

    pub fn reveal(
        &mut self,
        path: &RelativePath,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.set_active_file(Some(path.clone()), cx);
    }

    pub fn set_active_file(&mut self, path: Option<RelativePath>, cx: &mut Context<'_, Self>) {
        if !self.model.set_active(path.clone()) {
            return;
        }
        let Some(path) = path else {
            cx.notify();
            return;
        };
        self.model.request_reveal(&path);
        if self.model.expand_ancestors(&path) {
            self.refresh(cx);
        } else {
            self.scroll_to_pending();
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &Context<'_, Self>) {
        let files = Arc::clone(&self.services.files);
        let expanded: Vec<RelativePath> = self.model.expanded().iter().cloned().collect();
        let sort_by = self.model.sort_by();
        let hidden = [self.services.app_settings.read().attachment_folder()];
        let rows = run_blocking(cx, move || files.visible_tree(&expanded, sort_by, &hidden));
        self.rebuild = cx.spawn(async move |tree, cx| {
            let rows = rows.await;
            drop(tree.update(cx, |tree, cx| tree.apply_rows(rows, cx)));
        });
    }

    pub fn new_note(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let folder = self.model.target_folder();
        self.run(Command::Create(NewEntry::Note, folder), window, cx);
    }

    pub fn new_folder(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let folder = self.model.target_folder();
        self.run(Command::Create(NewEntry::Folder, folder), window, cx);
    }

    #[must_use]
    pub fn expanded_folders(&self) -> Vec<RelativePath> {
        self.model.expanded().iter().cloned().collect()
    }

    pub fn set_expanded_folders(&mut self, folders: Vec<RelativePath>, cx: &Context<'_, Self>) {
        self.model.set_expanded(folders);
        self.refresh(cx);
    }

    #[must_use]
    pub const fn sort_by(&self) -> SortBy {
        self.model.sort_by()
    }

    pub fn set_sort_by(&mut self, sort_by: SortBy, cx: &Context<'_, Self>) {
        self.model.set_sort_by(sort_by);
        self.refresh(cx);
    }

    #[must_use]
    pub fn selected_paths(&self) -> Vec<RelativePath> {
        let entries = self.model.selection().entries();
        entries.iter().map(|entry| entry.path().clone()).collect()
    }

    pub(crate) const fn model(&self) -> &TreeModel {
        &self.model
    }

    pub(crate) const fn edit(&self) -> Option<&InlineEdit> {
        self.edit.as_ref()
    }

    pub(crate) const fn has_clipboard(&self) -> bool {
        self.clipboard.has_items()
    }

    pub(crate) const fn keyboard(&self) -> bool {
        self.keyboard
    }

    pub(crate) const fn scroll(&self) -> &UniformListScrollHandle {
        &self.scroll
    }

    pub(crate) fn vault_root(&self) -> &Path {
        &self.services.root
    }

    pub(crate) fn run(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.keyboard = matches!(command, Command::Key(_));
        let selection = self.model.selection();
        match command {
            Command::Click(path, gesture) => {
                window.focus(&self.focus_handle, cx);
                let effect = self.model.click(&path, gesture);
                self.apply_effect(effect, window, cx);
            }
            Command::ClickBackground => {
                window.focus(&self.focus_handle, cx);
                self.model.clear_selection();
                cx.notify();
            }
            Command::ContextMenu(path) => {
                if let Some(row) = self.model.row(&path).cloned()
                    && !selection.contains(&path)
                {
                    self.model.select(&row.path, row.is_dir);
                }
                self.model.set_focus(&path);
                cx.notify();
            }
            Command::Key(key) => {
                let effect = self.model.press(key);
                self.apply_effect(effect, window, cx);
            }
            Command::Create(NewEntry::Folder, parent) => self.start_new_folder(parent, window, cx),
            Command::Create(kind, folder) => {
                self.run_job(Job::CreateNote(folder, kind.file_name()), window, cx);
            }
            Command::Rename(path) => self.start_rename(&path, window, cx),
            Command::Duplicate(entry) => self.run_job(Job::Duplicate(entry), window, cx),
            Command::Reveal(path) => {
                let absolute = self.services.root.join(path.as_str());
                if let Err(error) = Platform::reveal(cx, &absolute) {
                    notify_error(window, cx, "toast_open_finder_failed", &error);
                }
            }
            Command::Clipboard(mode, entry) => {
                let entries = selection.clipboard_targets(entry);
                if let Some(key) = self.clipboard.store(mode, entries) {
                    window.push_notification(Notification::success(t(cx, key)), cx);
                }
            }
            Command::Paste(folder) => {
                if let Some((mode, entries)) = self.clipboard.take() {
                    self.run_job(Job::Paste(entries, mode, folder), window, cx);
                }
            }
            Command::Delete(entry) => {
                self.run_job(Job::Delete(selection.targets(entry)), window, cx);
            }
            Command::Move(paths, folder) => {
                let entries = paths
                    .into_iter()
                    .map(|path| {
                        let is_dir = self.model.row(&path).is_some_and(|row| row.is_dir);
                        SelectedEntry::new(path, is_dir)
                    })
                    .collect();
                self.run_job(Job::Move(entries, folder), window, cx);
            }
            Command::Import(sources, folder) => {
                let host = HostHandle::new(Arc::clone(&cx.global::<AppServices>().host_files));
                self.run_job(Job::Import(host, sources, folder), window, cx);
            }
            Command::CollapseAll => {
                self.model.collapse_all();
                self.refresh(cx);
            }
            Command::Sort(sort_by) => self.set_sort_by(sort_by, cx),
            Command::SelectAll => {
                self.model.select_all();
                cx.notify();
            }
        }
    }

    pub(crate) fn commit_edit(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = &mut self.edit else {
            return;
        };
        if edit.is_submitting() {
            return;
        }
        match edit.decide(cx) {
            Decision::Close => self.close_edit(window, cx),
            Decision::Invalid(error) => {
                let value = edit.input().read(cx).value();
                let message = t_with(cx, error.message_key(), &[("name", value.trim())]);
                window.push_notification(Notification::error(message), cx);
                if matches!(edit.target(), EditTarget::NewFolder { .. }) {
                    self.close_edit(window, cx);
                }
            }
            Decision::CreateFolder { parent, name } => {
                self.close_edit(window, cx);
                self.run_job(Job::CreateFolder(parent, name), window, cx);
            }
            Decision::Rename { path, name } => {
                edit.set_submitting(true);
                self.run_job(Job::Rename(path, name), window, cx);
            }
        }
    }

    pub(crate) fn close_edit(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        if matches!(edit.target(), EditTarget::NewFolder { .. }) {
            self.model.set_pending_new_folder(None);
        }
        if edit.input().read(cx).focus_handle(cx).is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
        cx.notify();
    }

    fn apply_effect(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<'_, Self>) {
        match effect {
            Effect::None => {}
            Effect::Redraw => cx.notify(),
            Effect::Rebuild => self.refresh(cx),
            Effect::Open(path) => {
                cx.emit(WorkbenchEvent::Open(OpenRequest::new(path)));
                cx.notify();
            }
            Effect::Reveal(path) => {
                self.model.request_reveal(&path);
                self.scroll_to_pending();
                cx.notify();
            }
            Effect::Rename(path) => self.start_rename(&path, window, cx),
            Effect::Delete(entry) => self.run(Command::Delete(entry), window, cx),
        }
    }

    fn apply_rows(&mut self, rows: Vec<TreeRow>, cx: &mut Context<'_, Self>) {
        self.model.set_rows(rows);
        self.scroll_to_pending();
        cx.notify();
    }

    fn scroll_to_pending(&mut self) {
        if let Some(index) = self.model.take_scroll_target() {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
    }

    fn start_rename(
        &mut self,
        path: &RelativePath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(row) = self.model.row(path).cloned() else {
            return;
        };
        self.close_edit(window, cx);
        self.edit = Some(InlineEdit::rename(&row, window, cx));
        self.model.request_reveal(path);
        self.scroll_to_pending();
        cx.notify();
    }

    fn start_new_folder(
        &mut self,
        parent: RelativePath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.close_edit(window, cx);
        if self.model.expand(&parent) {
            self.refresh(cx);
        }
        self.model.set_pending_new_folder(Some(parent.clone()));
        let target = EditTarget::NewFolder { parent };
        self.edit = Some(InlineEdit::new(target, window, cx));
        self.scroll_to_pending();
        cx.notify();
    }

    fn run_job(&self, job: Job, window: &Window, cx: &Context<'_, Self>) {
        let followup = match &job {
            Job::CreateNote(..) => Followup::OpenCreated,
            Job::Rename(..) => Followup::FinishRename,
            _ => Followup::Nothing,
        };
        let files = Arc::clone(&self.services.files);
        let report = run_blocking(cx, move || job.run(&files));
        cx.spawn_in(window, async move |tree, cx| {
            let report = report.await;
            drop(tree.update_in(cx, |tree, window, cx| {
                tree.apply_report(&report, followup, window, cx);
            }));
        })
        .detach();
    }

    fn apply_report(
        &mut self,
        report: &Report,
        followup: Followup,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        report.notify(window, cx);
        report.apply_to(&mut self.model);
        for event in report.events() {
            cx.emit(event.clone());
        }
        match (followup, &mut self.edit) {
            (Followup::FinishRename, _) if report.changed() => self.close_edit(window, cx),
            (Followup::FinishRename, Some(edit)) => edit.set_submitting(false),
            _ => {}
        }
        if report.changed() {
            cx.emit(WorkbenchEvent::TreeChanged);
            self.refresh(cx);
        }
        if let (Followup::OpenCreated, Some(entry)) = (followup, report.select()) {
            let request = OpenRequest::new(entry.path().clone());
            cx.emit(WorkbenchEvent::Open(request));
        }
        cx.notify();
    }
}
