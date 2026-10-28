use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Window};
use margin_files::FilesService;
use margin_i18n::{t_count, t_with};
use margin_kernel::{EntryKind, HostFiles, RelativePath};
use margin_workbench::WorkbenchEvent;

use crate::model::TreeModel;
use crate::names::{CollisionSuffix, unique_name};
use crate::paths;
use crate::selection::SelectedEntry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Debug)]
pub(crate) struct Failure {
    key: &'static str,
    name: Option<String>,
    error: String,
}

impl Failure {
    fn new(key: &'static str, error: &impl ToString) -> Self {
        Self {
            key,
            name: None,
            error: error.to_string(),
        }
    }

    fn named(key: &'static str, name: &str, error: &impl ToString) -> Self {
        Self {
            key,
            name: Some(name.to_owned()),
            error: error.to_string(),
        }
    }

    fn arguments(&self) -> Vec<(&'static str, &str)> {
        let mut arguments = vec![("error", self.error.as_str())];
        if let Some(name) = &self.name {
            arguments.push(("name", name.as_str()));
        }
        arguments
    }
}

#[derive(Debug, Default)]
pub(crate) struct Report {
    events: Vec<WorkbenchEvent>,
    failures: Vec<Failure>,
    expand: Vec<RelativePath>,
    select: Option<SelectedEntry>,
    imported: u64,
    changed: bool,
}

impl Report {
    pub(crate) fn events(&self) -> &[WorkbenchEvent] {
        &self.events
    }

    pub(crate) fn notify(&self, window: &mut Window, cx: &mut App) {
        for failure in &self.failures {
            let message = t_with(cx, failure.key, &failure.arguments());
            window.push_notification(Notification::error(message), cx);
        }
        if self.imported > 0 {
            let message = t_count(cx, "toast_imported_items", self.imported, &[]);
            window.push_notification(Notification::success(message), cx);
        }
    }

    pub(crate) fn apply_to(&self, model: &mut TreeModel) {
        for event in &self.events {
            match event {
                WorkbenchEvent::Renamed { from, to } => model.remap(from, to),
                WorkbenchEvent::Deleted(path) => model.forget(path),
                _ => {}
            }
        }
        for folder in &self.expand {
            model.expand(folder);
        }
        if let Some(entry) = &self.select {
            model.select(entry.path(), entry.is_dir());
            model.request_reveal(entry.path());
        }
    }

    pub(crate) const fn select(&self) -> Option<&SelectedEntry> {
        self.select.as_ref()
    }

    pub(crate) const fn changed(&self) -> bool {
        self.changed
    }

    fn fail(&mut self, failure: Failure) {
        self.failures.push(failure);
    }

    fn renamed(&mut self, from: RelativePath, to: RelativePath) {
        self.changed = true;
        self.events.push(WorkbenchEvent::Renamed { from, to });
    }

    fn created(&mut self, entry: SelectedEntry) {
        self.changed = true;
        self.select = Some(entry);
    }
}

#[derive(Debug)]
pub(crate) enum Job {
    CreateNote(RelativePath, &'static str),
    CreateFolder(RelativePath, String),
    Rename(RelativePath, String),
    Duplicate(SelectedEntry),
    Paste(Vec<SelectedEntry>, ClipboardMode, RelativePath),
    Delete(Vec<SelectedEntry>),
    Move(Vec<SelectedEntry>, RelativePath),
    Import(HostHandle, Vec<PathBuf>, RelativePath),
}

pub(crate) struct HostHandle(Arc<dyn HostFiles>);

impl HostHandle {
    pub(crate) const fn new(host: Arc<dyn HostFiles>) -> Self {
        Self(host)
    }
}

impl fmt::Debug for HostHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HostHandle")
    }
}

impl Job {
    pub(crate) fn run(self, files: &FilesService) -> Report {
        match self {
            Self::CreateNote(folder, name) => create_note(files, &folder, name),
            Self::CreateFolder(parent, name) => create_folder(files, &parent, &name),
            Self::Rename(path, name) => rename(files, &path, &name),
            Self::Duplicate(entry) => duplicate(files, &entry),
            Self::Paste(entries, mode, folder) => paste(files, &entries, mode, &folder),
            Self::Delete(entries) => delete(files, &entries),
            Self::Move(entries, folder) => move_into(files, &entries, &folder),
            Self::Import(host, sources, folder) => import(files, &*host.0, &sources, &folder),
        }
    }
}

fn free_path(
    files: &FilesService,
    folder: &RelativePath,
    name: &str,
    suffix: CollisionSuffix,
) -> RelativePath {
    let listing = files.list_directory(folder);
    let unique = unique_name(
        name,
        listing.iter().map(|entry| entry.name.as_str()),
        suffix,
    );
    paths::child(folder, &unique)
}

fn create_note(files: &FilesService, folder: &RelativePath, name: &str) -> Report {
    let mut report = Report::default();
    let path = free_path(files, folder, name, CollisionSuffix::Numeric);
    match files.write_file(&path, b"") {
        Ok(()) => {
            report.expand.push(folder.clone());
            report.created(SelectedEntry::new(path, false));
        }
        Err(error) => report.fail(Failure::new("file_tree_create_note_failed", &error)),
    }
    report
}

fn create_folder(files: &FilesService, parent: &RelativePath, name: &str) -> Report {
    let mut report = Report::default();
    let path = paths::child(parent, name);
    if files.exists(&path) {
        report.fail(Failure::named("toast_path_exists", name, &""));
        return report;
    }
    match files.create_directory(&path) {
        Ok(()) => {
            report.expand.extend([parent.clone(), path.clone()]);
            report.created(SelectedEntry::new(path, true));
        }
        Err(error) => report.fail(Failure::new("toast_create_folder_failed", &error)),
    }
    report
}

fn rename(files: &FilesService, from: &RelativePath, name: &str) -> Report {
    let mut report = Report::default();
    let to = paths::child(&paths::parent(from), name);
    let differs_beyond_case = !to.as_str().eq_ignore_ascii_case(from.as_str());
    if differs_beyond_case && files.exists(&to) {
        report.fail(Failure::named("toast_path_exists", name, &""));
        return report;
    }
    match files.rename_entry(from, &to) {
        Ok(()) => report.renamed(from.clone(), to),
        Err(error) => report.fail(Failure::new("toast_rename_failed", &error)),
    }
    report
}

fn move_into(files: &FilesService, entries: &[SelectedEntry], folder: &RelativePath) -> Report {
    let mut report = Report::default();
    for entry in entries {
        let source = entry.path();
        if folder.starts_with(source) || &paths::parent(source) == folder {
            continue;
        }
        let name = source.file_name();
        let destination = paths::child(folder, name);
        if files.exists(&destination) {
            report.fail(Failure::named("toast_exists_in_folder", name, &""));
            continue;
        }
        match files.rename_entry(source, &destination) {
            Ok(()) => report.renamed(source.clone(), destination),
            Err(error) => report.fail(Failure::new("toast_move_failed", &error)),
        }
    }
    report
}

fn delete(files: &FilesService, entries: &[SelectedEntry]) -> Report {
    let mut report = Report::default();
    for entry in entries {
        match files.delete_entry(entry.path()) {
            Ok(()) => {
                report.changed = true;
                report
                    .events
                    .push(WorkbenchEvent::Deleted(entry.path().clone()));
            }
            Err(error) => report.fail(Failure::new("toast_delete_failed", &error)),
        }
    }
    report
}

fn duplicate(files: &FilesService, entry: &SelectedEntry) -> Report {
    let mut report = Report::default();
    let parent = paths::parent(entry.path());
    let copy = free_path(
        files,
        &parent,
        entry.path().file_name(),
        CollisionSuffix::Copy,
    );
    let copied = if entry.is_dir() {
        files.copy_directory(entry.path(), &copy)
    } else {
        files.copy_file(entry.path(), &copy)
    };
    match copied {
        Ok(()) => {
            report.expand.push(parent);
            report.created(SelectedEntry::new(copy, entry.is_dir()));
        }
        Err(error) => report.fail(Failure::new("toast_duplicate_failed", &error)),
    }
    report
}

fn paste(
    files: &FilesService,
    entries: &[SelectedEntry],
    mode: ClipboardMode,
    folder: &RelativePath,
) -> Report {
    let mut report = Report::default();
    for entry in entries {
        let source = entry.path();
        let destination = free_path(files, folder, source.file_name(), CollisionSuffix::Numeric);
        let pasted = match (mode, entry.is_dir()) {
            (ClipboardMode::Copy, true) => files.copy_directory(source, &destination),
            (ClipboardMode::Copy, false) => files.copy_file(source, &destination),
            (ClipboardMode::Cut, _) => files.rename_entry(source, &destination),
        };
        match pasted {
            Ok(()) if mode == ClipboardMode::Cut => report.renamed(source.clone(), destination),
            Ok(()) => report.changed = true,
            Err(error) => report.fail(Failure::new("toast_paste_failed", &error)),
        }
    }
    report.expand.push(folder.clone());
    report
}

fn import(
    files: &FilesService,
    host: &dyn HostFiles,
    sources: &[impl AsRef<Path>],
    folder: &RelativePath,
) -> Report {
    let mut report = Report::default();
    for source in sources {
        let source = source.as_ref();
        let Some(name) = source.file_name().map(|name| name.to_string_lossy()) else {
            continue;
        };
        let destination = free_path(files, folder, &name, CollisionSuffix::Numeric);
        let is_dir = host
            .metadata(source)
            .is_ok_and(|metadata| metadata.kind == EntryKind::Directory);
        let imported = if is_dir {
            files.import_directory(source, &destination)
        } else {
            files.import_file(source, &destination)
        };
        match imported {
            Ok(()) => {
                report.changed = true;
                report.imported += 1;
            }
            Err(error) => report.fail(Failure::named("toast_import_file_failed", &name, &error)),
        }
    }
    if report.imported > 0 {
        report.expand.push(folder.clone());
    }
    report
}
