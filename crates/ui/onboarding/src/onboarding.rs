use std::fmt;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, rems,
};
use margin_i18n::t;
use margin_theme::{Appearance, MarginTheme, UI_FONT_FAMILY};

use crate::actions::{
    Cancel, Confirm, NextStep, ONBOARDING_CONTEXT, PreviousStep, RemoveVault, RenameVault,
    SelectNext, SelectPrevious, bind_keys_once,
};
use crate::brand::{Brand, auto_opening, brand};
use crate::intro_art::IntroStep;
use crate::intro_view::{self, IntroExit};
use crate::onboarding_event::OnboardingEvent;
use crate::steps::Steps;
use crate::vault_picker::{PickerEvent, VaultPicker};
use crate::vault_wizard::{ExitWizard, VaultWizard, WizardExit};

const COLUMN_WIDTH_REMS: f32 = 27.1429;
const COMPACT_HEIGHT_REMS: f32 = 54.2857;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Loading,
    AutoOpening,
    Intro,
    Picker,
    Wizard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutoLogin {
    LastSession,
    Skipped,
}

pub struct Onboarding {
    focus_handle: FocusHandle,
    screen: Screen,
    auto_login: AutoLogin,
    tour: Steps,
    picker: Entity<VaultPicker>,
    wizard: Entity<VaultWizard>,
    _subscriptions: Vec<Subscription>,
}

impl fmt::Debug for Onboarding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Onboarding")
            .field("screen", &self.screen)
            .field("tour", &self.tour)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<OnboardingEvent> for Onboarding {}

impl Focusable for Onboarding {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Onboarding {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::build(AutoLogin::LastSession, window, cx)
    }

    pub fn new_without_auto_login(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::build(AutoLogin::Skipped, window, cx)
    }

    fn build(auto_login: AutoLogin, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        bind_keys_once(cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let picker = cx.new(|cx| VaultPicker::new(focus_handle.clone(), window, cx));
        let wizard = cx.new(|cx| VaultWizard::new(focus_handle.clone(), window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&picker, window, Self::picker_event),
            cx.subscribe_in(&picker, window, |_, _, event: &OnboardingEvent, _, cx| {
                cx.emit(event.clone());
            }),
            cx.subscribe_in(&wizard, window, |_, _, event: &OnboardingEvent, _, cx| {
                cx.emit(event.clone());
            }),
            cx.subscribe_in(&wizard, window, |this, _, _: &ExitWizard, window, cx| {
                this.back_to_vaults(window, cx);
            }),
        ];
        Self {
            focus_handle,
            screen: Screen::Loading,
            auto_login,
            tour: Steps::new(IntroStep::ALL.len()),
            picker,
            wizard,
            _subscriptions: subscriptions,
        }
    }

    fn picker_event(
        &mut self,
        picker: &Entity<VaultPicker>,
        event: &PickerEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match *event {
            PickerEvent::Loaded {
                last_session: Some(index),
            } if self.auto_login == AutoLogin::LastSession => {
                self.screen = Screen::AutoOpening;
                picker.update(cx, |picker, cx| picker.open(index, window, cx));
                cx.notify();
            }
            PickerEvent::Loaded { .. } => self.show_home(window, cx),
            PickerEvent::OpenFailed => {
                if self.screen == Screen::AutoOpening {
                    self.show(Screen::Picker, window, cx);
                }
            }
            PickerEvent::Emptied | PickerEvent::ReplayIntro => self.replay_intro(window, cx),
            PickerEvent::AddVault => self.start_new_vault(window, cx),
        }
    }

    fn has_profiles(&self, cx: &App) -> bool {
        !self.picker.read(cx).profiles().is_empty()
    }

    fn show_home(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.has_profiles(cx) {
            self.back_to_vaults(window, cx);
        } else {
            self.replay_intro(window, cx);
        }
    }

    fn show(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.screen = screen;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn replay_intro(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.tour = Steps::new(IntroStep::ALL.len());
        self.show(Screen::Intro, window, cx);
    }

    fn back_to_vaults(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.show(Screen::Picker, window, cx);
    }

    fn start_new_vault(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let exit = if self.has_profiles(cx) {
            WizardExit::ToVaults
        } else {
            WizardExit::Unavailable
        };
        self.wizard
            .update(cx, |wizard, cx| wizard.restart(exit, window, cx));
        self.show(Screen::Wizard, window, cx);
    }

    pub(crate) fn finish_intro(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.has_profiles(cx) {
            self.back_to_vaults(window, cx);
        } else {
            self.start_new_vault(window, cx);
        }
    }

    pub(crate) fn tour_go_to(&mut self, step: usize, _: &mut Window, cx: &mut Context<'_, Self>) {
        if self.tour.go_to(step) {
            cx.notify();
        }
    }

    pub(crate) fn tour_next(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.tour.next() {
            cx.notify();
        } else {
            self.finish_intro(window, cx);
        }
    }

    pub(crate) fn tour_previous(&mut self, cx: &mut Context<'_, Self>) {
        if self.tour.previous() {
            cx.notify();
        }
    }

    fn toggle_theme(cx: &mut App) {
        let appearance = if MarginTheme::palette(cx).dark {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        MarginTheme::set_appearance(appearance, cx);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.screen {
            Screen::Intro => self.tour_next(window, cx),
            Screen::Picker => self.picker.update(cx, |picker, cx| {
                picker.open_selected(window, cx);
            }),
            Screen::Wizard => self.wizard.update(cx, |wizard, cx| wizard.next(window, cx)),
            Screen::Loading | Screen::AutoOpening => {}
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.screen {
            Screen::Intro => self.finish_intro(window, cx),
            Screen::Wizard if self.wizard.read(cx).can_exit(window, cx) => {
                self.back_to_vaults(window, cx);
            }
            Screen::Loading | Screen::AutoOpening | Screen::Picker | Screen::Wizard => {
                cx.propagate();
            }
        }
    }

    fn previous_step(&mut self, _: &PreviousStep, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.screen {
            Screen::Intro => self.tour_previous(cx),
            Screen::Wizard => self.wizard.update(cx, |wizard, cx| {
                let current = wizard.wizard().steps().current();
                wizard.go_to(current.saturating_sub(1), window, cx);
            }),
            Screen::Loading | Screen::AutoOpening | Screen::Picker => cx.propagate(),
        }
    }

    fn next_step(&mut self, _: &NextStep, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.screen {
            Screen::Intro => self.tour_next(window, cx),
            Screen::Wizard => self.wizard.update(cx, |wizard, cx| {
                let current = wizard.wizard().steps().current();
                wizard.go_to(current + 1, window, cx);
            }),
            Screen::Loading | Screen::AutoOpening | Screen::Picker => cx.propagate(),
        }
    }

    fn picker_action(
        &self,
        cx: &mut Context<'_, Self>,
        action: impl FnOnce(&mut VaultPicker, &mut Context<'_, VaultPicker>),
    ) {
        if self.screen == Screen::Picker {
            self.picker.update(cx, action);
        } else {
            cx.propagate();
        }
    }

    fn render_content(&self, cx: &Context<'_, Self>) -> AnyElement {
        match self.screen {
            Screen::Loading => div().into_any_element(),
            Screen::AutoOpening => auto_opening(cx),
            Screen::Intro => {
                let exit = if self.has_profiles(cx) {
                    IntroExit::BackToVaults
                } else {
                    IntroExit::CreateVault
                };
                intro_view::render(self.tour, exit, cx)
            }
            Screen::Picker => v_flex()
                .child(brand(Brand::WithTagline, cx))
                .child(self.picker.clone())
                .into_any_element(),
            Screen::Wizard => v_flex()
                .child(brand(Brand::WithTagline, cx))
                .child(self.wizard.clone())
                .into_any_element(),
        }
    }
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let compact =
            window.viewport_size().height <= rems(COMPACT_HEIGHT_REMS).to_pixels(window.rem_size());
        let palette = MarginTheme::palette(cx);
        let (page, text) = (palette.surface(0), palette.text_primary);
        let theme_icon = if palette.dark {
            IconName::Sun
        } else {
            IconName::Moon
        };
        let shows_toggle = !matches!(self.screen, Screen::Loading | Screen::AutoOpening);
        let content = self.render_content(cx);
        div()
            .id("onboarding")
            .key_context(ONBOARDING_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::previous_step))
            .on_action(cx.listener(Self::next_step))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| {
                this.picker_action(cx, VaultPicker::select_previous);
            }))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.picker_action(cx, VaultPicker::select_next);
            }))
            .on_action(cx.listener(|this, _: &RenameVault, window, cx| {
                this.picker_action(cx, |picker, cx| {
                    if let Some(index) = picker.selected() {
                        picker.rename(index, window, cx);
                    }
                });
            }))
            .on_action(cx.listener(|this, _: &RemoveVault, window, cx| {
                this.picker_action(cx, |picker, cx| {
                    if let Some(index) = picker.selected() {
                        picker.remove(index, window, cx);
                    }
                });
            }))
            .relative()
            .size_full()
            .overflow_y_scroll()
            .bg(page)
            .text_color(text)
            .font_family(UI_FONT_FAMILY)
            .child(
                v_flex()
                    .min_h_full()
                    .w_full()
                    .items_center()
                    .p_8()
                    .map(|column| {
                        if compact {
                            column.justify_start()
                        } else {
                            column.justify_center()
                        }
                    })
                    .child(
                        v_flex()
                            .w(rems(COLUMN_WIDTH_REMS))
                            .max_w_full()
                            .when(compact, |column| column.pt_10().pb_4())
                            .child(content),
                    ),
            )
            .when(shows_toggle, |this| {
                this.child(
                    div().absolute().top_4().right_4().child(
                        Button::new("onboarding-theme")
                            .ghost()
                            .icon(Icon::new(theme_icon))
                            .tooltip(t(cx, "statusbar_toggle_theme"))
                            .on_click(|_, _, cx| Self::toggle_theme(cx)),
                    ),
                )
            })
    }
}
