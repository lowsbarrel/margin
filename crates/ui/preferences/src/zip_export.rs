use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Icon, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window};
use margin_i18n::t;
use margin_workbench::{CurrentVault, notify_error, notify_info, run_blocking};

use crate::field::{CONTROL_HEIGHT, Tone, note};

pub(crate) struct ZipExport {
    exporting: bool,
}

impl ZipExport {
    pub(crate) const fn new() -> Self {
        Self { exporting: false }
    }

    fn export(&self, window: &Window, cx: &Context<'_, Self>) {
        if self.exporting {
            return;
        }
        let services = CurrentVault::expect(cx);
        let directory = services
            .root
            .parent()
            .map_or_else(|| services.root.clone(), PathBuf::from);
        let suggested = format!("{}.zip", services.name);
        let destination = cx.prompt_for_new_path(&directory, Some(&suggested));
        let files = Arc::clone(&services.files);
        cx.spawn_in(window, async move |this, cx| {
            let path = match destination.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    drop(cx.update(|window, cx| {
                        notify_error(window, cx, "toast_zip_export_failed", &error);
                    }));
                    return;
                }
            };
            let Ok(export) = this.update(cx, |this, cx| {
                this.exporting = true;
                cx.notify();
                run_blocking(cx, move || files.export_vault_zip(&path))
            }) else {
                return;
            };
            let result = export.await;
            drop(this.update_in(cx, |this, window, cx| {
                this.exporting = false;
                cx.notify();
                match result {
                    Ok(()) => notify_info(window, cx, "toast_zip_exported"),
                    Err(error) => notify_error(window, cx, "toast_zip_export_failed", &error),
                }
            }));
        })
        .detach();
    }
}

impl Render for ZipExport {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let label = if self.exporting {
            "settings_export_zip_exporting"
        } else {
            "settings_export_zip_btn"
        };
        v_flex()
            .gap_3()
            .child(note(t(cx, "settings_export_zip_hint"), Tone::Neutral, cx))
            .child(
                Button::new("export-zip")
                    .outline()
                    .h(CONTROL_HEIGHT)
                    .w_full()
                    .icon(Icon::new(IconName::Archive))
                    .label(t(cx, label))
                    .loading(self.exporting)
                    .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
            )
    }
}
