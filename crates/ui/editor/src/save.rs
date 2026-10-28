use std::mem;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, Context, Task, WeakEntity};
use margin_files::Error as FilesError;
use margin_kernel::RelativePath;
use margin_workbench::{VaultServices, run_blocking};

use crate::note_editor::NoteEditor;
use crate::{async_ops, persistence};

const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
const SNAPSHOT_INTERVAL: Duration = Duration::from_mins(5);

#[derive(Default)]
pub(crate) struct SaveController {
    last_saved: Option<Arc<str>>,
    saved_revision: u64,
    timer: Option<Task<()>>,
    writing: Option<u64>,
    queued: bool,
    last_snapshot: Option<Instant>,
    last_snapshot_text: Option<Arc<str>>,
}

pub(crate) struct Outcome {
    revision: u64,
    text: Arc<str>,
    result: Result<(), FilesError>,
}

impl SaveController {
    pub(crate) fn loaded(&mut self, text: Arc<str>, revision: u64) {
        self.last_saved = Some(text);
        self.saved_revision = revision;
        self.timer = None;
        self.queued = false;
    }

    pub(crate) fn is_dirty(&self, revision: u64) -> bool {
        revision != self.saved_revision || self.writing.is_some()
    }

    pub(crate) fn has_unsaved_edits(&self, revision: u64) -> bool {
        revision != self.saved_revision
    }

    pub(crate) fn schedule(&mut self, cx: &Context<'_, NoteEditor>) {
        self.timer = Some(cx.spawn(async move |this: WeakEntity<NoteEditor>, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            drop(this.update(cx, |editor, cx| editor.flush_now(cx)));
        }));
    }

    pub(crate) fn begin(&mut self, revision: u64) -> Option<SaveJob> {
        self.timer = None;
        if revision == self.saved_revision {
            return None;
        }
        if self.writing.is_some() {
            self.queued = true;
            return None;
        }
        self.writing = Some(revision);
        let snapshot = self.departing_snapshot();
        Some(SaveJob { revision, snapshot })
    }

    fn departing_snapshot(&mut self) -> Option<Arc<str>> {
        let departing = self.last_saved.clone()?;
        let due = self
            .last_snapshot
            .is_none_or(|at| at.elapsed() >= SNAPSHOT_INTERVAL);
        if !due || self.last_snapshot_text.as_deref() == Some(&*departing) {
            return None;
        }
        self.last_snapshot = Some(Instant::now());
        self.last_snapshot_text = Some(Arc::clone(&departing));
        Some(departing)
    }

    pub(crate) fn snapshot_before_adopt(&mut self, current: Arc<str>) -> Option<Arc<str>> {
        if self.last_snapshot_text.as_deref() == Some(&*current) {
            return None;
        }
        self.last_snapshot = Some(Instant::now());
        self.last_snapshot_text = Some(Arc::clone(&current));
        Some(current)
    }

    pub(crate) fn finish(&mut self, outcome: Outcome) -> FinishResult {
        self.writing = None;
        let retry = mem::take(&mut self.queued);
        match outcome.result {
            Ok(()) => {
                if outcome.revision >= self.saved_revision {
                    self.saved_revision = outcome.revision;
                }
                self.last_saved = Some(outcome.text);
                FinishResult::Saved { retry }
            }
            Err(error) => FinishResult::Failed { error },
        }
    }
}

pub(crate) struct SaveJob {
    revision: u64,
    snapshot: Option<Arc<str>>,
}

pub(crate) enum FinishResult {
    Saved { retry: bool },
    Failed { error: FilesError },
}

pub(crate) fn write(
    job: SaveJob,
    services: Arc<VaultServices>,
    path: RelativePath,
    text: Arc<str>,
    window: AnyWindowHandle,
    cx: &Context<'_, NoteEditor>,
) {
    let SaveJob { revision, snapshot } = job;
    let work = run_blocking(cx, {
        let path = path.clone();
        let text = Arc::clone(&text);
        move || {
            let snapshot_failed = snapshot.is_some_and(|departing| {
                services.history.save(&path, departing.as_bytes()).is_err()
            });
            let written = services.files.write_file(&path, text.as_bytes());
            (written, snapshot_failed)
        }
    });
    async_ops::after(
        work,
        window,
        cx,
        move |editor, (result, snapshot_failed), window, cx| {
            let outcome = Outcome {
                revision,
                text,
                result,
            };
            persistence::finish_save(editor, outcome, snapshot_failed, &path, window, cx);
        },
    );
}

pub(crate) fn snapshot(
    services: Arc<VaultServices>,
    path: RelativePath,
    text: Arc<str>,
    cx: &Context<'_, NoteEditor>,
) -> Task<bool> {
    run_blocking(cx, move || {
        services.history.save(&path, text.as_bytes()).is_ok()
    })
}
