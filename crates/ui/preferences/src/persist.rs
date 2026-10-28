use gpui_kit::{App, Window};
use margin_settings::AppSettings;
use margin_workbench::{CurrentVault, notify_error, notify_info, run_blocking};
use parking_lot::Mutex;

static SETTINGS_WRITES: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveNotice {
    Silent,
    Saved,
    Imported,
}

pub(crate) fn update_settings(
    edit: impl FnOnce(&mut AppSettings),
    notice: SaveNotice,
    window: &Window,
    cx: &App,
) {
    edit(&mut CurrentVault::expect(cx).app_settings.write());
    save_settings(notice, window, cx);
}

pub(crate) fn activate_services(window: &mut Window, cx: &mut App) {
    let services = CurrentVault::expect(cx);
    services.configure_sync();
    let llm = services.app_settings.read().llm.clone();
    if let Some(llm) = llm
        && let Err(error) = services.ask.configure(llm)
    {
        notify_error(window, cx, "toast_save_failed", &error);
    }
}

pub(crate) fn save_settings(notice: SaveNotice, window: &Window, cx: &App) {
    let services = CurrentVault::expect(cx);
    let write = run_blocking(cx, move || {
        let _in_order = SETTINGS_WRITES.lock();
        let settings = services.app_settings.read().clone();
        services.settings.save(&services.key, &settings)
    });
    window
        .spawn(cx, async move |cx| {
            let result = write.await;
            cx.update(|window, cx| match (result, notice) {
                (Err(error), _) => notify_error(window, cx, "toast_save_failed", &error),
                (Ok(()), SaveNotice::Saved) => notify_info(window, cx, "toast_settings_saved"),
                (Ok(()), SaveNotice::Imported) => notify_info(window, cx, "toast_import_success"),
                (Ok(()), SaveNotice::Silent) => {}
            })
        })
        .detach();
}
