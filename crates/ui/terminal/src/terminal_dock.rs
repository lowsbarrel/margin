#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use std::fmt;
use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Global,
    InteractiveElement, IntoElement, KeyDownEvent, NoAction, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, transparent_black,
};
use margin_i18n::{t, t_with};
use margin_terminals::{SessionId, Terminals};
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::{CurrentVault, ToggleTerminal, WORKSPACE_CONTEXT, binding};

use crate::session_status::{SessionStatus, StatusChanged};
use crate::terminal_view::{KEY_CONTEXT, TerminalView};

actions!(terminal, [NewTerminal, CloseTerminal]);

const SHELL_CHORDS: [&str; 7] = [
    "ctrl-p", "ctrl-k", "ctrl-n", "ctrl-\\", "ctrl-w", "ctrl-f", "ctrl-h",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalDockEvent {
    Hide,
}

struct TerminalKeysBound;

impl Global for TerminalKeysBound {}

struct TerminalTab {
    number: u32,
    view: Entity<TerminalView>,
    focus: FocusHandle,
    _status: Subscription,
}

pub struct TerminalDock {
    terminals: Terminals,
    cwd: PathBuf,
    tabs: Vec<TerminalTab>,
    active: usize,
    last_number: u32,
    focus_handle: FocusHandle,
}

impl EventEmitter<TerminalDockEvent> for TerminalDock {}

impl fmt::Debug for TerminalDock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalDock")
            .field("cwd", &self.cwd)
            .field("tabs", &self.tabs.len())
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl Focusable for TerminalDock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.tabs.get(self.active).map_or_else(
            || self.focus_handle.clone(),
            |tab| tab.view.focus_handle(cx),
        )
    }
}

impl TerminalDock {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let vault = CurrentVault::expect(cx);
        Self::with_terminals(vault.terminals.clone(), vault.root.clone(), window, cx)
    }

    pub fn with_terminals(
        terminals: Terminals,
        cwd: PathBuf,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        if !cx.has_global::<TerminalKeysBound>() {
            bind_keys(cx);
            cx.set_global(TerminalKeysBound);
        }
        Self {
            terminals,
            cwd,
            tabs: Vec::new(),
            active: 0,
            last_number: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    #[must_use]
    pub fn has_terminals(&self) -> bool {
        !self.tabs.is_empty()
    }

    pub fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.last_number += 1;
        let number = self.last_number;
        let terminals = self.terminals.clone();
        let cwd = self.cwd.clone();
        let view =
            cx.new(|cx| TerminalView::new(SessionId::new(number), terminals, cwd, window, cx));
        let status = cx.subscribe(&view, |_, _, _: &StatusChanged, cx| cx.notify());
        self.tabs.push(TerminalTab {
            number,
            view,
            focus: cx.focus_handle().tab_stop(true),
            _status: status,
        });
        self.activate(self.tabs.len() - 1, window, cx);
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(tab) = self.tabs.get(index) {
            self.active = index;
            window.focus(&tab.view.focus_handle(cx), cx);
            cx.notify();
        }
    }

    fn activate_number(&mut self, number: u32, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.number == number) {
            self.activate(index, window, cx);
        }
    }

    fn close(&mut self, number: u32, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(index) = self.tabs.iter().position(|tab| tab.number == number) else {
            return;
        };
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = 0;
            cx.emit(TerminalDockEvent::Hide);
            cx.notify();
            return;
        }
        let active = match self.active {
            current if current > index => current - 1,
            current if current == index => index.min(self.tabs.len() - 1),
            current => current,
        };
        self.activate(active, window, cx);
    }

    fn close_active(&mut self, _: &CloseTerminal, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(number) = self.tabs.get(self.active).map(|tab| tab.number) {
            self.close(number, window, cx);
        }
    }

    fn open_new(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.new_terminal(window, cx);
    }

    fn tab(&self, index: usize, window: &Window, cx: &Context<'_, Self>) -> impl IntoElement {
        let tab = &self.tabs[index];
        let palette = MarginTheme::palette(cx);
        let number = tab.number;
        let label = number.to_string();
        let exit_code = match tab.view.read(cx).status() {
            SessionStatus::Exited(code) => Some(code.to_string()),
            SessionStatus::Running | SessionStatus::Failed(_) => None,
        };
        let (fill, text) = if index == self.active {
            (palette.surface(3), palette.text_primary)
        } else {
            (transparent_black(), palette.text_tertiary)
        };
        let hover = (index != self.active).then(|| (palette.surface(2), palette.text_secondary));
        let ring = if tab.focus.is_focused(window) {
            palette.bg_brand
        } else {
            transparent_black()
        };
        let close = Button::new(("terminal-close", number))
            .custom(quiet(cx))
            .xsmall()
            .icon(Icon::new(IconName::X))
            .tooltip(t(cx, "terminal_close_tab"))
            .on_click(cx.listener(move |dock, _, window, cx| {
                cx.stop_propagation();
                dock.close(number, window, cx);
            }));
        h_flex()
            .id(("terminal-tab", number))
            .track_focus(&tab.focus)
            .h_7()
            .min_w_0()
            .gap_1p5()
            .px_1p5()
            .rounded(cx.theme().radius)
            .border_2()
            .border_color(ring)
            .bg(fill)
            .text_ui_xs()
            .text_color(text)
            .cursor_pointer()
            .when_some(hover, |this, (fill, text)| {
                this.hover(move |style| style.bg(fill).text_color(text))
            })
            .on_click(cx.listener(move |dock, _, window, cx| {
                dock.activate_number(number, window, cx);
            }))
            .on_key_down(cx.listener(move |dock, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    dock.activate_number(number, window, cx);
                }
            }))
            .child(div().min_w_0().max_w_32().truncate().child(t_with(
                cx,
                "terminal_tab_title",
                &[("n", label.as_str())],
            )))
            .children(exit_code.map(|code| {
                div()
                    .flex_shrink_0()
                    .text_color(palette.text_tertiary)
                    .child(code)
            }))
            .child(close)
    }
}

impl Render for TerminalDock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let surface = palette.surface(1);
        let border = palette.border_secondary;
        let tabs: Vec<_> = (0..self.tabs.len())
            .map(|index| self.tab(index, window, cx))
            .collect();
        let new_terminal = Button::new("terminal-new")
            .custom(quiet(cx))
            .small()
            .icon(Icon::new(IconName::Plus))
            .tooltip_with_action(t(cx, "terminal_new"), &NewTerminal, Some(KEY_CONTEXT))
            .on_click(cx.listener(|dock, _, window, cx| dock.new_terminal(window, cx)));
        let hide = Button::new("terminal-hide")
            .custom(quiet(cx))
            .small()
            .icon(Icon::new(IconName::ChevronDown))
            .tooltip_with_action(
                t(cx, "terminal_hide"),
                &ToggleTerminal,
                Some(WORKSPACE_CONTEXT),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.emit(TerminalDockEvent::Hide)));
        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(surface)
            .on_action(cx.listener(Self::open_new))
            .on_action(cx.listener(Self::close_active))
            .child(
                h_flex()
                    .h_9()
                    .flex_shrink_0()
                    .gap_1()
                    .px_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        h_flex()
                            .id("terminal-tabs")
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .overflow_x_scroll()
                            .children(tabs),
                    )
                    .child(new_terminal)
                    .child(hide),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .children(self.tabs.get(self.active).map(|tab| tab.view.clone())),
            )
    }
}

fn quiet(cx: &App) -> ButtonCustomVariant {
    let palette = MarginTheme::palette(cx);
    ButtonCustomVariant::new(cx)
        .foreground(palette.text_tertiary)
        .hover(palette.surface(3))
        .active(palette.surface(3))
}

fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    let (new_terminal, close_terminal) = if cx.platform_info().is_mac() {
        ("cmd-t", "cmd-w")
    } else {
        ("ctrl-shift-t", "ctrl-shift-w")
    };
    let mut bindings = vec![
        binding(new_terminal, NewTerminal, context, cx),
        binding(close_terminal, CloseTerminal, context, cx),
    ];
    bindings.extend(
        SHELL_CHORDS
            .iter()
            .map(|chord| binding(chord, NoAction, context, cx)),
    );
    cx.bind_keys(bindings);
}
