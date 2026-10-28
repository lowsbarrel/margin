use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Icon, WindowExt, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, px,
};
use margin_i18n::{Translator, t};
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::{CurrentVault, SyncNow};

use crate::ai_pane::AiPane;
use crate::card::card;
use crate::field::CONTROL_HEIGHT;
use crate::look::{LocaleSelect, appearance_body, locale_select};
use crate::persist::{SaveNotice, activate_services, save_settings};
use crate::settings_section::SettingsSection;
use crate::sync_pane::{SyncPane, SyncRequested};
use crate::transfer::{ExportPane, GenerateRequested, ImportPane, Imported};
use crate::updates_pane::UpdatesPane;
use crate::vault_pane::VaultPane;
use crate::zip_export::ZipExport;

const WINDOW_CLEARANCE: f32 = 80.;

pub(crate) struct SettingsView {
    open: HashSet<SettingsSection>,
    vault: Entity<VaultPane>,
    sync: Entity<SyncPane>,
    ai: Entity<AiPane>,
    language: Entity<LocaleSelect>,
    zip: Entity<ZipExport>,
    export: Entity<ExportPane>,
    import: Entity<ImportPane>,
    updates: Entity<UpdatesPane>,
    _subscriptions: [Subscription; 5],
}

impl SettingsView {
    pub(crate) fn new(
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let settings = CurrentVault::expect(cx).app_settings.read().clone();
        let sync = cx.new(|cx| SyncPane::new(&settings, window, cx));
        let ai = cx.new(|cx| AiPane::new(&settings, window, cx));
        let export = cx.new(|cx| ExportPane::new(window, cx));
        let import = cx.new(|cx| ImportPane::new(window, cx));
        let (language, language_changes) = locale_select(window, cx);
        let subscriptions = [
            cx.subscribe_in(&sync, window, |this, _, _: &SyncRequested, window, cx| {
                if this.commit(SaveNotice::Silent, window, cx) {
                    window.dispatch_action(Box::new(SyncNow), cx);
                }
            }),
            cx.subscribe_in(
                &export,
                window,
                |this, export, _: &GenerateRequested, window, cx| {
                    if this.commit(SaveNotice::Silent, window, cx) {
                        export.update(cx, |export, cx| export.generate(window, cx));
                    }
                },
            ),
            cx.subscribe_in(&import, window, |this, _, _: &Imported, window, cx| {
                let settings = CurrentVault::expect(cx).app_settings.read().clone();
                this.sync
                    .update(cx, |pane, cx| pane.load(&settings, window, cx));
                this.ai
                    .update(cx, |pane, cx| pane.load(&settings, window, cx));
            }),
            language_changes,
            cx.observe_global_in::<Translator>(window, Self::relabel),
        ];
        Self {
            open: HashSet::from([section]),
            vault: cx.new(|cx| VaultPane::new(window, cx)),
            sync,
            ai,
            language,
            zip: cx.new(|_| ZipExport::new()),
            export,
            import,
            updates: cx.new(|_| UpdatesPane::new()),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn close_requested(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        self.commit(SaveNotice::Saved, window, cx)
    }

    fn commit(
        &mut self,
        notice: SaveNotice,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let services = CurrentVault::expect(cx);
        let current = services.app_settings.read().clone();
        let mut next = current.clone();
        let sync = self.sync.update(cx, |pane, cx| pane.apply(&mut next, cx));
        let ai = self.ai.update(cx, |pane, cx| pane.apply(&mut next, cx));
        let invalid: Vec<_> = [(sync, SettingsSection::Sync), (ai, SettingsSection::Ai)]
            .into_iter()
            .filter_map(|(result, section)| result.is_err().then_some(section))
            .collect();
        if !invalid.is_empty() {
            self.open.extend(invalid);
            cx.notify();
            return false;
        }
        let changed = next.s3 != current.s3
            || next.auto_sync != current.auto_sync
            || next.conflict_strategy != current.conflict_strategy
            || next.llm != current.llm;
        if changed {
            *services.app_settings.write() = next;
            activate_services(window, cx);
            save_settings(notice, window, cx);
        }
        true
    }

    fn relabel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.vault.update(cx, |pane, cx| pane.relabel(window, cx));
        self.sync.update(cx, |pane, cx| pane.relabel(window, cx));
        self.ai.update(cx, |pane, cx| pane.relabel(window, cx));
        self.import.update(cx, |pane, cx| pane.relabel(window, cx));
        cx.notify();
    }

    fn toggle(&mut self, section: SettingsSection, cx: &mut Context<'_, Self>) {
        if !self.open.remove(&section) {
            self.open.insert(section);
        }
        cx.notify();
    }

    fn section(
        &self,
        section: SettingsSection,
        body: AnyElement,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement + use<> {
        card(
            section,
            self.open.contains(&section),
            cx.listener(move |this, _, _, cx| this.toggle(section, cx)),
            |card| card.child(body),
            cx,
        )
    }
}

fn render_header(cx: &Context<'_, SettingsView>) -> impl IntoElement + use<> {
    let palette = MarginTheme::palette(cx);
    h_flex()
        .flex_none()
        .justify_between()
        .px_6()
        .py_4()
        .border_b_1()
        .border_color(palette.border_secondary)
        .child(
            div()
                .text_ui_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .child(t(cx, "settings_title")),
        )
        .child(
            Button::new("close-settings")
                .ghost()
                .size_7()
                .icon(Icon::new(IconName::X))
                .tooltip(t(cx, "dialog_close"))
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.close_requested(window, cx) {
                        window.close_dialog(cx);
                    }
                })),
        )
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let max_height = window.viewport_size().height - px(WINDOW_CLEARANCE);
        let sections = [
            (
                SettingsSection::Vault,
                self.vault.clone().into_any_element(),
            ),
            (SettingsSection::Sync, self.sync.clone().into_any_element()),
            (SettingsSection::Ai, self.ai.clone().into_any_element()),
            (
                SettingsSection::Language,
                Select::new(&self.language)
                    .h(CONTROL_HEIGHT)
                    .into_any_element(),
            ),
            (
                SettingsSection::Appearance,
                appearance_body(cx).into_any_element(),
            ),
            (
                SettingsSection::ExportVault,
                self.zip.clone().into_any_element(),
            ),
            (
                SettingsSection::ExportSettings,
                self.export.clone().into_any_element(),
            ),
            (
                SettingsSection::ImportSettings,
                self.import.clone().into_any_element(),
            ),
            (
                SettingsSection::Updates,
                self.updates.clone().into_any_element(),
            ),
        ]
        .map(|(section, body)| self.section(section, body, cx));
        v_flex()
            .w_full()
            .max_h(max_height)
            .child(render_header(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(v_flex().gap_3().p_6().children(sections)),
            )
    }
}
