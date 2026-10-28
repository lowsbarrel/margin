use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::{
    App, AppContext, ClipboardItem, Context, Entity, EventEmitter, ExternalPaths, FocusHandle,
    Focusable, PathPromptOptions, SharedString, Window,
};
use margin_crypto::generate_mnemonic;
use margin_i18n::t;
use margin_kernel::Secret;
use margin_workbench::notify_error;

use crate::background::in_background;
use crate::onboarding_event::OnboardingEvent;
use crate::passphrase;
use crate::vault_session::{self, NewVault};
use crate::wizard::{Wizard, WizardStep, profile_name};

const COPIED_FEEDBACK: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizardExit {
    ToVaults,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExitWizard;

pub(crate) struct VaultWizard {
    focus_handle: FocusHandle,
    wizard: Wizard,
    exit: WizardExit,
    name: Entity<InputState>,
    passphrase: Entity<TextareaState>,
    generated: Option<Secret<String>>,
    copied: bool,
    busy: bool,
    error: Option<SharedString>,
}

impl fmt::Debug for VaultWizard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultWizard")
            .field("wizard", &self.wizard)
            .field("busy", &self.busy)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<OnboardingEvent> for VaultWizard {}

impl EventEmitter<ExitWizard> for VaultWizard {}

impl VaultWizard {
    pub(crate) fn new(
        focus_handle: FocusHandle,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(cx, "login_vault_name_placeholder"))
        });
        let passphrase = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 5)
                .submit_on_enter(true)
                .placeholder(t(cx, "login_passphrase_placeholder"))
        });
        Self {
            focus_handle,
            wizard: Wizard::default(),
            exit: WizardExit::Unavailable,
            name,
            passphrase,
            generated: None,
            copied: false,
            busy: false,
            error: None,
        }
    }

    pub(crate) const fn wizard(&self) -> &Wizard {
        &self.wizard
    }

    pub(crate) const fn exit(&self) -> WizardExit {
        self.exit
    }

    pub(crate) const fn name_input(&self) -> &Entity<InputState> {
        &self.name
    }

    pub(crate) const fn passphrase_input(&self) -> &Entity<TextareaState> {
        &self.passphrase
    }

    pub(crate) const fn generated(&self) -> Option<&Secret<String>> {
        self.generated.as_ref()
    }

    pub(crate) const fn is_copied(&self) -> bool {
        self.copied
    }

    pub(crate) const fn is_busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn error(&self) -> Option<SharedString> {
        self.error.clone()
    }

    pub(crate) fn restart(
        &mut self,
        exit: WizardExit,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.wizard = Wizard::default();
        self.exit = exit;
        self.generated = None;
        self.copied = false;
        self.busy = false;
        self.error = None;
        self.name
            .update(cx, |name, cx| name.set_value("", window, cx));
        self.passphrase
            .update(cx, |passphrase, cx| passphrase.set_value("", window, cx));
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn go_to(&mut self, step: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.wizard.go_to(step) {
            self.step_changed(window, cx);
        }
    }

    pub(crate) fn next(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.wizard.step() {
            WizardStep::Folder if !self.wizard.can_advance() => Self::pick_folder(window, cx),
            WizardStep::Passphrase => self.submit(window, cx),
            WizardStep::Folder | WizardStep::Name => {
                if self.wizard.next() {
                    self.step_changed(window, cx);
                }
            }
        }
    }

    pub(crate) fn previous(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.wizard.previous() {
            self.step_changed(window, cx);
        }
    }

    pub(crate) fn can_exit(&self, window: &Window, cx: &App) -> bool {
        let editing = self.name.focus_handle(cx).is_focused(window)
            || self.passphrase.focus_handle(cx).is_focused(window);
        self.exit == WizardExit::ToVaults && !editing
    }

    pub(crate) fn folder_dropped(paths: &ExternalPaths, window: &Window, cx: &Context<'_, Self>) {
        let Some(path) = paths.paths().first().cloned() else {
            return;
        };
        in_background(
            window,
            cx,
            move |_| path.is_dir().then_some(path),
            |this, folder, window, cx| {
                if let Some(folder) = folder {
                    this.folder_chosen(&folder, window, cx);
                }
            },
        );
    }

    fn step_changed(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.error = None;
        match self.wizard.step() {
            WizardStep::Folder => self.focus_handle.focus(window, cx),
            WizardStep::Name => self.name.update(cx, |name, cx| name.focus(window, cx)),
            WizardStep::Passphrase => self
                .passphrase
                .update(cx, |passphrase, cx| passphrase.focus(window, cx)),
        }
        cx.notify();
    }

    pub(crate) fn pick_folder(window: &Window, cx: &Context<'_, Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(chosen) = chosen.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| match chosen {
                Ok(Some(paths)) => {
                    if let Some(path) = paths.first() {
                        this.folder_chosen(path, window, cx);
                    }
                }
                Ok(None) => {}
                Err(error) => notify_error(window, cx, "login_error_pick_folder_failed", &error),
            })
            .unwrap_or_default();
        })
        .detach();
    }

    fn folder_chosen(&mut self, path: &Path, window: &mut Window, cx: &mut Context<'_, Self>) {
        let folder = self.wizard.choose_folder(path).to_owned();
        if self.name.read(cx).value().trim().is_empty() {
            let suggested = profile_name("", &folder);
            self.name
                .update(cx, |name, cx| name.set_value(suggested, window, cx));
        }
        self.error = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn generate(&mut self, cx: &mut Context<'_, Self>) {
        self.generated = Some(generate_mnemonic());
        self.copied = false;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn use_generated(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(mnemonic) = self.generated.take() {
            self.passphrase.update(cx, |passphrase, cx| {
                passphrase.set_value(mnemonic.into_exposed(), window, cx);
            });
            cx.notify();
        }
    }

    pub(crate) fn copy_generated(&mut self, cx: &mut Context<'_, Self>) {
        let Some(mnemonic) = &self.generated else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(mnemonic.expose().clone()));
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FEEDBACK).await;
            this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            })
            .unwrap_or_default();
        })
        .detach();
    }

    pub(crate) fn submit(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if self.busy {
            return;
        }
        let Some(folder) = self.wizard.folder().map(str::to_owned) else {
            self.error = Some(t(cx, "login_error_no_path"));
            cx.notify();
            return;
        };
        let text = self.passphrase.read(cx).value();
        if let Err(error) = passphrase::check_not_empty(&text) {
            self.error = Some(t(cx, error.message_key()));
            cx.notify();
            return;
        }
        let name = profile_name(&self.name.read(cx).value(), &folder);
        let root = PathBuf::from(&folder);
        let request = NewVault::new(name.clone(), folder, Secret::new(text.to_string()));
        self.busy = true;
        self.error = None;
        cx.notify();
        in_background(
            window,
            cx,
            move |services| {
                vault_session::create(&services.vaults, services.host_files.as_ref(), &request)
            },
            move |this, created, _, cx| {
                this.busy = false;
                match created {
                    Ok(vault) => cx.emit(OnboardingEvent::Unlocked { name, root, vault }),
                    Err(error) => this.error = Some(error.message(cx)),
                }
                cx.notify();
            },
        );
    }
}
