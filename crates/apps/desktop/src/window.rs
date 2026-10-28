use std::fmt;
use std::path::Path;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Subscription, Task, Window, div,
};
use margin_crypto::DerivedVault;
use margin_i18n::{Locale, Translator};
use margin_onboarding::{Onboarding, OnboardingEvent};
use margin_settings::AppSettings;
use margin_theme::{Appearance, MarginTheme, UI_FONT_FAMILY};
use margin_workbench::{AppServices, CheckForUpdates, CurrentVault, Quit, notify_error};

use crate::lifecycle::UPDATE_CHECK_INTERVAL;
use crate::update_notice::RestartAfterUpdate;
use crate::workspace::{Locked, Workspace};
use crate::{container, update_notice, updater};

enum Screen {
    Onboarding(Entity<Onboarding>),
    Workspace(Entity<Workspace>),
}

pub struct MarginWindow {
    screen: Screen,
    screen_subscription: Option<Subscription>,
    closing: bool,
    _updates: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl fmt::Debug for MarginWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MarginWindow")
    }
}

impl MarginWindow {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            MarginTheme::follow_system(window, cx);
        });
        let should_close = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            should_close
                .update(cx, |this, cx| this.request_close(window, cx))
                .unwrap_or(true)
        });
        let updates = cx.spawn_in(window, async move |_, cx| {
            loop {
                if cx.update(update_notice::check).is_err() {
                    break;
                }
                cx.background_executor().timer(UPDATE_CHECK_INTERVAL).await;
            }
        });
        let onboarding = cx.new(|cx| Onboarding::new(window, cx));
        let mut this = Self {
            screen: Screen::Onboarding(onboarding.clone()),
            screen_subscription: None,
            closing: false,
            _updates: updates,
            _subscriptions: vec![appearance],
        };
        this.watch_onboarding(&onboarding, window, cx);
        this
    }

    fn show_onboarding(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let onboarding = cx.new(|cx| Onboarding::new_without_auto_login(window, cx));
        self.watch_onboarding(&onboarding, window, cx);
        self.screen = Screen::Onboarding(onboarding);
        cx.notify();
    }

    fn watch_onboarding(
        &mut self,
        onboarding: &Entity<Onboarding>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.screen_subscription = Some(cx.subscribe_in(
            onboarding,
            window,
            |this, _, event, window, cx| match event {
                OnboardingEvent::Unlocked { name, root, vault } => {
                    this.open_vault(name.clone(), root, vault, window, cx);
                }
            },
        ));
        let focus = onboarding.read(cx).focus_handle(cx);
        focus.focus(window, cx);
    }

    fn open_vault(
        &mut self,
        name: String,
        root: &Path,
        vault: &DerivedVault,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let app = cx.global::<AppServices>().clone();
        match container::open_vault(&app, name, root, vault) {
            Ok(opened) => {
                apply_vault_preferences(&opened.services.app_settings.read(), cx);
                cx.set_global(CurrentVault(Some(Arc::clone(&opened.services))));
                let workspace = cx.new(|cx| {
                    Workspace::new(Arc::clone(&opened.services), &opened.watcher, window, cx)
                });
                self.screen_subscription = Some(cx.subscribe_in(
                    &workspace,
                    window,
                    |this, _, _: &Locked, window, cx| this.lock(window, cx),
                ));
                let focus = workspace.read(cx).focus_handle(cx);
                focus.focus(window, cx);
                self.screen = Screen::Workspace(workspace);
                cx.notify();
            }
            Err(error) => notify_error(window, cx, "toast_vault_open_failed", &error),
        }
    }

    fn lock(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        cx.set_global(CurrentVault(None));
        self.show_onboarding(window, cx);
    }

    fn request_close(&mut self, window: &Window, cx: &mut Context<'_, Self>) -> bool {
        let Screen::Workspace(workspace) = &self.screen else {
            return true;
        };
        if self.closing {
            return true;
        }
        self.closing = true;
        let closing = workspace.update(cx, |workspace, cx| workspace.prepare_to_close(window, cx));
        cx.spawn_in(window, async move |_, cx| {
            closing.await;
            drop(cx.update(|window, _| window.remove_window()));
        })
        .detach();
        false
    }

    fn shut_down(&mut self, finish: fn(&mut App), window: &Window, cx: &mut Context<'_, Self>) {
        let closing = match &self.screen {
            Screen::Workspace(workspace) => {
                Some(workspace.update(cx, |workspace, cx| workspace.prepare_to_close(window, cx)))
            }
            Screen::Onboarding(_) => None,
        };
        self.closing = true;
        cx.spawn(async move |_, cx| {
            if let Some(closing) = closing {
                closing.await;
            }
            cx.update(finish);
        })
        .detach();
    }
}

fn quit_app(cx: &mut App) {
    cx.quit();
}

fn apply_vault_preferences(settings: &AppSettings, cx: &mut App) {
    if let Some(theme) = settings.theme.as_deref() {
        MarginTheme::set_appearance(Appearance::from_setting(Some(theme)), cx);
    }
    if let Some(locale) = settings.locale.as_deref().and_then(Locale::from_tag) {
        Translator::set_locale(locale, cx);
    }
}

impl Render for MarginWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let content = match &self.screen {
            Screen::Onboarding(onboarding) => onboarding.clone().into_any_element(),
            Screen::Workspace(workspace) => workspace.clone().into_any_element(),
        };
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(UI_FONT_FAMILY)
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                this.shut_down(quit_app, window, cx);
            }))
            .on_action(cx.listener(|this, _: &RestartAfterUpdate, window, cx| {
                this.shut_down(updater::relaunch, window, cx);
            }))
            .on_action(cx.listener(|_, _: &CheckForUpdates, window, cx| {
                update_notice::check(window, cx);
            }))
            .child(content)
    }
}
