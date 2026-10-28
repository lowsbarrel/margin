use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{App, AsyncWindowContext, Context, Task, WeakEntity, Window};
use margin_attachments::SWEEP_GRACE;
use margin_kernel::{RelativePath, StoreError, Subscription, Watcher};
use margin_theme::MarginTheme;
use margin_workbench::{VaultServices, notify_error, run_blocking};

use crate::workspace::Workspace;
use crate::workspace_state;

pub(crate) const UPDATE_CHECK_INTERVAL: Duration = Duration::from_mins(5);
pub(crate) const WORKSPACE_SAVE_DELAY: Duration = Duration::from_secs(1);
const CLOSE_FLUSH_TIMEOUT: Duration = Duration::from_secs(3);
const CLOSE_FLUSH_POLL: Duration = Duration::from_millis(20);

pub(crate) fn start(
    workspace: &mut Workspace,
    watcher: &Arc<dyn Watcher>,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    match watch_vault(watcher, cx) {
        Ok((subscription, task)) => {
            workspace.set_watch(subscription);
            workspace.track(task);
        }
        Err(error) => notify_error(window, cx, "toast_vault_open_failed", &error),
    }
    let maintenance = run_maintenance(&workspace.services, cx);
    workspace.track(cx.spawn(async move |this, cx| {
        maintenance.await;
        drop(this.update(cx, |this, cx| this.refresh_vault(cx)));
    }));
    let services = Arc::clone(&workspace.services);
    let loaded = run_blocking(cx, move || workspace_state::load_existing(&services));
    workspace.track(cx.spawn_in(window, async move |this, cx| {
        let state = loaded.await;
        drop(this.update_in(cx, |this, window, cx| this.restore(state, window, cx)));
    }));
}

pub(crate) fn persist_theme(services: &Arc<VaultServices>, cx: &App) {
    let appearance = MarginTheme::global(cx).appearance().as_setting();
    let changed = {
        let mut settings = services.app_settings.write();
        let changed = settings.theme.as_deref() != Some(appearance);
        settings.theme = Some(appearance.to_owned());
        changed
    };
    if changed {
        let services = Arc::clone(services);
        run_blocking(cx, move || {
            drop(
                services
                    .settings
                    .save(&services.key, &services.app_settings.read()),
            );
        })
        .detach();
    }
}

pub(crate) fn watch_vault(
    watcher: &Arc<dyn Watcher>,
    cx: &Context<'_, Workspace>,
) -> Result<(Subscription, Task<()>), StoreError> {
    let (sender, receiver) = async_channel::unbounded::<Vec<RelativePath>>();
    let subscription = watcher.watch(Box::new(move |paths| {
        drop(sender.try_send(paths));
    }))?;
    let task = cx.spawn(async move |this, cx| {
        while let Ok(mut batch) = receiver.recv().await {
            while let Ok(more) = receiver.try_recv() {
                batch.extend(more);
            }
            batch.sort();
            batch.dedup();
            let delivered = this.update(cx, |workspace, cx| workspace.on_vault_changed(batch, cx));
            if delivered.is_err() {
                break;
            }
        }
    });
    Ok((subscription, task))
}

pub(crate) fn run_maintenance(services: &Arc<VaultServices>, cx: &App) -> Task<()> {
    let services = Arc::clone(services);
    run_blocking(cx, move || {
        services.trash.purge_expired();
        drop(services.search.rebuild());
        let folder = services.app_settings.read().attachment_folder();
        drop(services.attachments.sweep_unused(&folder, SWEEP_GRACE));
    })
}

#[expect(
    clippy::future_not_send,
    reason = "GPUI window futures run on the main thread and hold its context"
)]
pub(crate) async fn wait_until_clean(this: WeakEntity<Workspace>, cx: &AsyncWindowContext) {
    let mut waited = Duration::ZERO;
    while waited < CLOSE_FLUSH_TIMEOUT {
        let dirty = this
            .read_with(cx, Workspace::has_unsaved_tabs)
            .unwrap_or(false);
        if !dirty {
            return;
        }
        cx.background_executor().timer(CLOSE_FLUSH_POLL).await;
        waited += CLOSE_FLUSH_POLL;
    }
}
