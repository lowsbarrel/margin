use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div,
};
use margin_crypto::DerivedVault;
use margin_kernel::Subscription as WatchSubscription;
use margin_onboarding::{Onboarding, OnboardingEvent};
use margin_theme::MarginTheme;
use margin_workbench::{AppServices, CurrentVault, notify_error};

use crate::container;
use crate::workspace::Workspace;

enum Screen {
    Onboarding(Entity<Onboarding>),
    Workspace(Entity<Workspace>),
}

pub(crate) struct MarginWindow {
    screen: Screen,
    watch: Option<WatchSubscription>,
    _subscriptions: Vec<Subscription>,
}

impl MarginWindow {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            MarginTheme::follow_system(window, cx);
        });
        let mut this = Self {
            screen: Screen::Onboarding(cx.new(|cx| Onboarding::new(window, cx))),
            watch: None,
            _subscriptions: vec![appearance],
        };
        this.show_onboarding(window, cx);
        this
    }

    fn show_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let onboarding = cx.new(|cx| Onboarding::new(window, cx));
        let subscription =
            cx.subscribe_in(
                &onboarding,
                window,
                |this, _, event, window, cx| match event {
                    OnboardingEvent::Unlocked { name, root, vault } => {
                        this.open_vault(name.clone(), root.clone(), vault.clone(), window, cx);
                    }
                },
            );
        self._subscriptions.push(subscription);
        self.screen = Screen::Onboarding(onboarding);
        cx.notify();
    }

    fn open_vault(
        &mut self,
        name: String,
        root: PathBuf,
        vault: DerivedVault,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = cx.global::<AppServices>().clone();
        match container::open_vault(&app, name, &root, vault) {
            Ok(opened) => {
                cx.set_global(CurrentVault(Some(Arc::clone(&opened.services))));
                let workspace = cx.new(|cx| {
                    Workspace::new(
                        Arc::clone(&opened.services),
                        Arc::clone(&opened.watcher),
                        window,
                        cx,
                    )
                });
                let subscription = cx.subscribe_in(
                    &workspace,
                    window,
                    |this, _, _locked: &crate::workspace::Locked, window, cx| {
                        this.lock(window, cx);
                    },
                );
                self._subscriptions.push(subscription);
                self.screen = Screen::Workspace(workspace);
                cx.notify();
            }
            Err(error) => notify_error(window, cx, "toast_vault_open_failed", &error),
        }
    }

    fn lock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.watch = None;
        cx.set_global(CurrentVault(None));
        self.show_onboarding(window, cx);
    }
}

impl Render for MarginWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.screen {
            Screen::Onboarding(onboarding) => onboarding.clone().into_any_element(),
            Screen::Workspace(workspace) => workspace.clone().into_any_element(),
        };
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(content)
    }
}
