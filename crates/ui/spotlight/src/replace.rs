use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Context, Task, Window};
use margin_files::FilesService;
use margin_i18n::{t_count, t_with};
use margin_kernel::RelativePath;
use margin_search::SearchService;
use margin_workbench::{VaultServices, notify_error, run_blocking};

use crate::spotlight::Spotlight;

#[derive(Debug, Default)]
pub(crate) struct ReplaceOutcome {
    replaced: Vec<RelativePath>,
    total: u64,
    failures: Vec<String>,
}

impl ReplaceOutcome {
    pub(crate) fn replaced(&self) -> &[RelativePath] {
        &self.replaced
    }

    pub(crate) const fn total(&self) -> u64 {
        self.total
    }
}

pub(crate) fn start(
    services: Arc<VaultServices>,
    paths: Vec<RelativePath>,
    text: String,
    replacement: String,
    window: &Window,
    cx: &Context<'_, Spotlight>,
) -> Task<()> {
    let single = paths.len() == 1;
    let work = run_blocking(cx, move || {
        in_files(
            &services.files,
            &services.search,
            &paths,
            &text,
            &replacement,
        )
    });
    cx.spawn_in(window, async move |this, cx| {
        let outcome = work.await;
        drop(this.update_in(cx, |this, window, cx| {
            this.finish_replace(&outcome, single, window, cx);
        }));
    })
}

fn in_files(
    files: &FilesService,
    search: &SearchService,
    paths: &[RelativePath],
    text: &str,
    replacement: &str,
) -> ReplaceOutcome {
    let mut outcome = ReplaceOutcome::default();
    for path in paths {
        match files.replace_in_file(path, text, replacement, false) {
            Ok(0) => {}
            Ok(count) => {
                outcome.total += u64::from(count);
                outcome.replaced.push(path.clone());
            }
            Err(error) => outcome.failures.push(error.to_string()),
        }
    }
    drop(search.apply_changes(&outcome.replaced));
    outcome
}

pub(crate) fn report(outcome: &ReplaceOutcome, single: bool, window: &mut Window, cx: &mut App) {
    if outcome.total > 0 {
        let message = if outcome.replaced.len() > 1 {
            let count = outcome.total.to_string();
            let files = outcome.replaced.len().to_string();
            t_with(
                cx,
                "toast_replaced_in_files",
                &[("count", &count), ("files", &files)],
            )
        } else {
            t_count(cx, "toast_replaced", outcome.total, &[])
        };
        window.push_notification(Notification::success(message), cx);
    }
    match outcome.failures.as_slice() {
        [] => {}
        [error] if single => notify_error(window, cx, "toast_replace_failed", error),
        failures => {
            let count = u64::try_from(failures.len()).unwrap_or(u64::MAX);
            let message = t_count(cx, "toast_replace_failed_count", count, &[]);
            window.push_notification(Notification::error(message), cx);
        }
    }
}
