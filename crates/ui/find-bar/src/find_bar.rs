use std::fmt;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Enter, Escape, Input, InputEvent, InputState};
use gpui_kit::component::{Disableable, Selectable, Sizable, h_flex, v_flex};
use gpui_kit::{
    App, AppContext, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString, Styled,
    Subscription, Window, div, px, rems,
};
use margin_i18n::{t, t_count, t_with};
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::{FindInNote, ReplaceInNote, WORKSPACE_CONTEXT};

use crate::query::{CaseSensitivity, FindQuery};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FindMode {
    #[default]
    Find,
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindEvent {
    QueryChanged,
    Next,
    Previous,
    ReplaceOne(String),
    ReplaceAll(String),
    Close,
}

pub struct FindBar {
    find_input: Entity<InputState>,
    replace_input: Entity<InputState>,
    mode: FindMode,
    case: CaseSensitivity,
    current: Option<usize>,
    total: usize,
    _subscription: Subscription,
}

impl fmt::Debug for FindBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FindBar")
            .field("mode", &self.mode)
            .field("case", &self.case)
            .field("current", &self.current)
            .field("total", &self.total)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<FindEvent> for FindBar {}

impl FindBar {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let find_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "find_placeholder")));
        let replace_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "find_replace_placeholder")));
        let subscription = cx.subscribe_in(&find_input, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.emit(FindEvent::QueryChanged);
            }
        });
        Self {
            find_input,
            replace_input,
            mode: FindMode::Find,
            case: CaseSensitivity::Insensitive,
            current: None,
            total: 0,
            _subscription: subscription,
        }
    }

    pub fn set_mode(&mut self, mode: FindMode, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.find_input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
    }

    pub fn set_status(&mut self, current: Option<usize>, total: usize, cx: &mut Context<'_, Self>) {
        self.current = current;
        self.total = total;
        cx.notify();
    }

    #[must_use]
    pub fn query(&self, cx: &App) -> FindQuery {
        FindQuery {
            text: self.find_input.read(cx).value().to_string(),
            case: self.case,
        }
    }

    fn status(&self, cx: &App) -> Option<SharedString> {
        if self.find_input.read(cx).value().is_empty() {
            return None;
        }
        if self.total == 0 {
            return Some(t(cx, "find_no_results"));
        }
        let total = self.total.to_string();
        Some(match self.current {
            Some(index) => {
                let current = (index + 1).to_string();
                t_with(
                    cx,
                    "find_count",
                    &[("current", &current), ("total", &total)],
                )
            }
            None => t_count(cx, "find_matches", self.total as u64, &[]),
        })
    }

    fn toggle_case(&mut self, cx: &mut Context<'_, Self>) {
        self.case = match self.case {
            CaseSensitivity::Insensitive => CaseSensitivity::Sensitive,
            CaseSensitivity::Sensitive => CaseSensitivity::Insensitive,
        };
        cx.emit(FindEvent::QueryChanged);
        cx.notify();
    }

    fn toggle_replace(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let mode = match self.mode {
            FindMode::Find => FindMode::Replace,
            FindMode::Replace => FindMode::Find,
        };
        self.set_mode(mode, window, cx);
    }

    fn replacement(&self, cx: &App) -> String {
        self.replace_input.read(cx).value().to_string()
    }

    fn open_in(&mut self, mode: FindMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.set_mode(mode, window, cx);
        self.focus(window, cx);
    }

    fn find_row(&self, cx: &Context<'_, Self>) -> Div {
        let palette = MarginTheme::palette(cx);
        let empty = self.total == 0;
        let case_on = self.case == CaseSensitivity::Sensitive;
        let replacing = self.mode == FindMode::Replace;
        let toggle_label = t(cx, "find_toggle_replace");
        let case = icon_button(
            "find-case",
            IconName::CaseSensitive,
            t(cx, "find_case_sensitive"),
            px(24.0),
        )
        .selected(case_on)
        .toggled(case_on)
        .debug_selector(|| "find-case".to_owned())
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_case(cx)));
        h_flex()
            .gap_1p5()
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(field(
                        &self.find_input,
                        "find-query",
                        t(cx, "find_placeholder"),
                    ))
                    .child(div().mr_0p5().child(case)),
            )
            .child(
                div()
                    .min_w(rems(50.0 / 14.0))
                    .text_center()
                    .text_ui_xs()
                    .whitespace_nowrap()
                    .text_color(palette.text_tertiary)
                    .children(self.status(cx)),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .child(
                        icon_button(
                            "find-previous",
                            IconName::ChevronUp,
                            t(cx, "find_previous"),
                            BUTTON,
                        )
                        .disabled(empty)
                        .on_click(
                            cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(FindEvent::Previous)),
                        ),
                    )
                    .child(
                        icon_button(
                            "find-next",
                            IconName::ChevronDown,
                            t(cx, "find_next"),
                            BUTTON,
                        )
                        .disabled(empty)
                        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(FindEvent::Next))),
                    )
                    .child(
                        Button::new("find-toggle-replace")
                            .ghost()
                            .with_size(BUTTON)
                            .icon(IconName::Replace)
                            .selected(replacing)
                            .toggled(replacing)
                            .accessibility_label(toggle_label.clone())
                            .tooltip_with_action(
                                toggle_label,
                                &ReplaceInNote,
                                Some(WORKSPACE_CONTEXT),
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.toggle_replace(window, cx);
                            })),
                    )
                    .child(
                        icon_button("find-close", IconName::X, t(cx, "find_close"), BUTTON)
                            .on_click(
                                cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(FindEvent::Close)),
                            ),
                    ),
            )
    }

    fn replace_row(&self, cx: &Context<'_, Self>) -> Div {
        let empty = self.total == 0;
        h_flex()
            .gap_1p5()
            .child(h_flex().flex_1().overflow_hidden().child(field(
                &self.replace_input,
                "find-replacement",
                t(cx, "find_replace_placeholder"),
            )))
            .child(
                h_flex()
                    .gap_0p5()
                    .child(
                        icon_button(
                            "find-replace",
                            IconName::Replace,
                            t(cx, "find_replace"),
                            BUTTON,
                        )
                        .disabled(empty)
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                cx.emit(FindEvent::ReplaceOne(this.replacement(cx)));
                            },
                        )),
                    )
                    .child(
                        icon_button(
                            "find-replace-all",
                            IconName::ReplaceAll,
                            t(cx, "find_replace_all"),
                            BUTTON,
                        )
                        .disabled(empty)
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                cx.emit(FindEvent::ReplaceAll(this.replacement(cx)));
                            },
                        )),
                    ),
            )
    }
}

const BUTTON: Pixels = px(26.0);

fn field(state: &Entity<InputState>, id: &'static str, label: SharedString) -> Input {
    Input::new(state)
        .id(id)
        .appearance(false)
        .flex_1()
        .px_2()
        .py_1()
        .text_ui_sm()
        .aria_label(label)
}

fn icon_button(id: &'static str, icon: IconName, label: SharedString, size: Pixels) -> Button {
    Button::new(id)
        .ghost()
        .with_size(size)
        .icon(icon)
        .accessibility_label(label.clone())
        .tooltip(label)
}

impl Render for FindBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let surface = palette.surface[1];
        let border = palette.border_secondary;
        let replace_row = (self.mode == FindMode::Replace).then(|| self.replace_row(cx));
        v_flex()
            .id("find-bar")
            .occlude()
            .absolute()
            .top_2()
            .right_4()
            .min_w(rems(320.0 / 14.0))
            .gap_1()
            .px_2()
            .py_1p5()
            .rounded(rems(8.0 / 14.0))
            .border_1()
            .border_color(border)
            .bg(surface)
            .shadow_lg()
            .on_action(cx.listener(|_, action: &Enter, _, cx| {
                cx.emit(if action.shift {
                    FindEvent::Previous
                } else {
                    FindEvent::Next
                });
            }))
            .on_action(cx.listener(|_, _: &Escape, _, cx| cx.emit(FindEvent::Close)))
            .on_action(cx.listener(|this, _: &FindInNote, window, cx| {
                this.open_in(FindMode::Find, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReplaceInNote, window, cx| {
                this.open_in(FindMode::Replace, window, cx);
            }))
            .child(self.find_row(cx))
            .children(replace_row)
    }
}

impl Focusable for FindBar {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.find_input.focus_handle(cx)
    }
}
