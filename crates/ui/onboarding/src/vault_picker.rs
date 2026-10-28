use std::fmt;
use std::path::{Path, PathBuf};

use gpui_kit::{Context, EventEmitter, FocusHandle, SharedString, Window};
use margin_vaults::{VaultProfileInfo, VaultProfilesInfo};
use margin_workbench::{Platform, notify_error};

use crate::background::in_background;
use crate::onboarding_event::OnboardingEvent;
use crate::{profile_dialogs, vault_session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickerEvent {
    Loaded { last_session: Option<usize> },
    OpenFailed,
    Emptied,
    AddVault,
    ReplayIntro,
}

pub(crate) struct VaultPicker {
    focus_handle: FocusHandle,
    profiles: Vec<VaultProfileInfo>,
    selected: Option<usize>,
    error: Option<SharedString>,
    busy: bool,
}

impl fmt::Debug for VaultPicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultPicker")
            .field("profiles", &self.profiles)
            .field("selected", &self.selected)
            .field("busy", &self.busy)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<OnboardingEvent> for VaultPicker {}

impl EventEmitter<PickerEvent> for VaultPicker {}

impl VaultPicker {
    pub(crate) fn new(focus_handle: FocusHandle, window: &Window, cx: &Context<'_, Self>) -> Self {
        in_background(
            window,
            cx,
            |services| services.vaults.load_profiles(),
            Self::loaded,
        );
        Self {
            focus_handle,
            profiles: Vec::new(),
            selected: None,
            error: None,
            busy: false,
        }
    }

    fn loaded(
        &mut self,
        loaded: Result<VaultProfilesInfo, margin_vaults::Error>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let info = match loaded {
            Ok(info) => info,
            Err(error) => {
                notify_error(window, cx, "login_error_load_failed", &error);
                VaultProfilesInfo {
                    profiles: Vec::new(),
                    last_used: None,
                }
            }
        };
        let last_session = info.last_used.as_deref().and_then(|last_used| {
            info.profiles
                .iter()
                .position(|profile| profile.vault_path == last_used)
        });
        self.profiles = info.profiles;
        self.selected = None;
        cx.emit(PickerEvent::Loaded { last_session });
        cx.notify();
    }

    pub(crate) const fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(crate) fn profiles(&self) -> &[VaultProfileInfo] {
        &self.profiles
    }

    pub(crate) const fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub(crate) fn error(&self) -> Option<SharedString> {
        self.error.clone()
    }

    pub(crate) const fn is_busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn select(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        if index < self.profiles.len() {
            self.selected = Some(index);
            cx.notify();
        }
    }

    pub(crate) fn select_next(&mut self, cx: &mut Context<'_, Self>) {
        let next = self.selected.map_or(0, |index| {
            (index + 1).min(self.profiles.len().saturating_sub(1))
        });
        self.select(next, cx);
    }

    pub(crate) fn select_previous(&mut self, cx: &mut Context<'_, Self>) {
        let previous = self.selected.map_or_else(
            || self.profiles.len().saturating_sub(1),
            |index| index.saturating_sub(1),
        );
        self.select(previous, cx);
    }

    pub(crate) fn open_selected(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if let Some(index) = self.selected {
            self.open(index, window, cx);
        }
    }

    pub(crate) fn open(&mut self, index: usize, window: &Window, cx: &mut Context<'_, Self>) {
        let Some(profile) = self.profiles.get(index).cloned() else {
            return;
        };
        if self.busy {
            return;
        }
        self.selected = self.selected.map(|_| index);
        self.busy = true;
        self.error = None;
        cx.notify();
        let folder = profile.vault_path.clone();
        in_background(
            window,
            cx,
            move |services| {
                vault_session::open_saved(&services.vaults, services.host_files.as_ref(), &folder)
            },
            move |this, opened, _, cx| {
                this.busy = false;
                match opened {
                    Ok(vault) => cx.emit(OnboardingEvent::Unlocked {
                        name: profile.name,
                        root: PathBuf::from(profile.vault_path),
                        vault,
                    }),
                    Err(error) => {
                        this.error = Some(error.message(cx));
                        cx.emit(PickerEvent::OpenFailed);
                    }
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn rename(&self, index: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(profile) = self.profiles.get(index) {
            profile_dialogs::rename(profile, cx.entity().downgrade(), window, cx);
        }
    }

    pub(crate) fn remove(&self, index: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(profile) = self.profiles.get(index) {
            profile_dialogs::confirm_removal(profile, cx.entity().downgrade(), window, cx);
        }
    }

    pub(crate) fn reveal(&self, index: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(profile) = self.profiles.get(index)
            && let Err(error) = Platform::reveal(cx, Path::new(&profile.vault_path))
        {
            notify_error(window, cx, "login_error_reveal_failed", &error);
        }
    }

    pub(crate) fn apply_rename(
        folder: String,
        name: String,
        window: &Window,
        cx: &Context<'_, Self>,
    ) {
        let (target, new_name) = (folder.clone(), name.clone());
        in_background(
            window,
            cx,
            move |services| services.vaults.rename_profile(&target, &new_name),
            move |this, renamed, window, cx| match renamed {
                Ok(()) => {
                    for profile in &mut this.profiles {
                        if profile.vault_path == folder {
                            profile.name.clone_from(&name);
                        }
                    }
                    cx.notify();
                }
                Err(error) => notify_error(window, cx, "login_error_rename_failed", &error),
            },
        );
    }

    pub(crate) fn apply_removal(folder: String, window: &Window, cx: &Context<'_, Self>) {
        let target = folder.clone();
        in_background(
            window,
            cx,
            move |services| services.vaults.delete_profile(&target),
            move |this, removed, window, cx| match removed {
                Ok(()) => {
                    this.profiles.retain(|profile| profile.vault_path != folder);
                    let last = this.profiles.len().checked_sub(1);
                    this.selected = this.selected.zip(last).map(|(index, last)| index.min(last));
                    if this.profiles.is_empty() {
                        cx.emit(PickerEvent::Emptied);
                    }
                    cx.notify();
                }
                Err(error) => notify_error(window, cx, "login_error_remove_failed", &error),
            },
        );
    }
}
