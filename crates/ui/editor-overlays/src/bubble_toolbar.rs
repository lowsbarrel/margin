use std::fmt;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::{
    Action, App, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle,
    InteractiveElement, IntoElement, ParentElement, Render, Role, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, rems,
};
use margin_editor_actions::{
    EDITOR_CONTEXT, InsertLink, ToggleBold, ToggleCode, ToggleComment, ToggleHighlight,
    ToggleItalic, ToggleStrike,
};
use margin_editor_commands::{BUBBLE_BLOCK_TYPES, BlockType, MarkKind};
use margin_i18n::{t, t_with};
use margin_theme::MarginTheme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BubbleEvent {
    SetLink(String),
    CancelLink,
    TurnInto(BlockType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubbleState {
    pub marks: Vec<MarkKind>,
    pub block: BlockType,
    pub in_table: bool,
}

impl Default for BubbleState {
    fn default() -> Self {
        Self {
            marks: Vec::new(),
            block: BlockType::Text,
            in_table: false,
        }
    }
}

const MARKS: [(MarkKind, &str, IconName); 5] = [
    (MarkKind::Bold, "bubble_bold", IconName::Bold),
    (MarkKind::Italic, "bubble_italic", IconName::Italic),
    (MarkKind::Strike, "bubble_strike", IconName::Strikethrough),
    (MarkKind::Code, "bubble_code", IconName::Code),
    (
        MarkKind::Highlight,
        "bubble_highlight",
        IconName::Highlighter,
    ),
];

fn mark_action(kind: MarkKind) -> Box<dyn Action> {
    match kind {
        MarkKind::Bold => Box::new(ToggleBold),
        MarkKind::Italic => Box::new(ToggleItalic),
        MarkKind::Strike => Box::new(ToggleStrike),
        MarkKind::Code => Box::new(ToggleCode),
        MarkKind::Highlight => Box::new(ToggleHighlight),
        MarkKind::Comment => Box::new(ToggleComment),
    }
}

fn block_label(block: BlockType, cx: &App) -> SharedString {
    let heading = |level: &str| t_with(cx, "bubble_heading", &[("level", level)]);
    match block {
        BlockType::Text => t(cx, "bubble_text"),
        BlockType::Heading1 => heading("1"),
        BlockType::Heading2 => heading("2"),
        BlockType::Heading3 => heading("3"),
        BlockType::Heading4 => heading("4"),
        BlockType::Heading5 => heading("5"),
        BlockType::Heading6 => heading("6"),
        BlockType::BulletList => t(cx, "bubble_block_bullet"),
        BlockType::OrderedList => t(cx, "bubble_block_ordered"),
        BlockType::TaskList => t(cx, "bubble_block_task"),
        BlockType::Quote => t(cx, "bubble_block_quote"),
        BlockType::CodeBlock => t(cx, "bubble_block_code"),
    }
}

const fn block_icon(block: BlockType) -> IconName {
    match block {
        BlockType::Text => IconName::Type,
        BlockType::Heading1 => IconName::Heading1,
        BlockType::Heading2 => IconName::Heading2,
        BlockType::Heading3 => IconName::Heading3,
        BlockType::Heading4 | BlockType::Heading5 | BlockType::Heading6 => IconName::Heading,
        BlockType::BulletList => IconName::List,
        BlockType::OrderedList => IconName::ListOrdered,
        BlockType::TaskList => IconName::ListTodo,
        BlockType::Quote => IconName::Quote,
        BlockType::CodeBlock => IconName::SquareCode,
    }
}

pub struct BubbleToolbar {
    editor_focus: FocusHandle,
    state: BubbleState,
    link: Option<(Entity<InputState>, Subscription)>,
    menu_open: bool,
}

impl fmt::Debug for BubbleToolbar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BubbleToolbar")
            .field("state", &self.state)
            .field("editing_link", &self.link.is_some())
            .field("menu_open", &self.menu_open)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<BubbleEvent> for BubbleToolbar {}

impl BubbleToolbar {
    #[must_use]
    pub fn new(editor_focus: FocusHandle, _cx: &mut Context<'_, Self>) -> Self {
        Self {
            editor_focus,
            state: BubbleState::default(),
            link: None,
            menu_open: false,
        }
    }

    pub fn set_state(&mut self, state: BubbleState, cx: &mut Context<'_, Self>) {
        if self.state != state {
            self.state = state;
            cx.notify();
        }
    }

    #[must_use]
    pub const fn is_in_use(&self) -> bool {
        self.link.is_some() || self.menu_open
    }

    pub fn open_link(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "bubble_link_placeholder")));
        let subscription = cx.subscribe_in(&input, window, |toolbar, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                toolbar.submit_link(window, cx);
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        self.link = Some((input, subscription));
        cx.notify();
    }

    fn submit_link(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let url = self
            .link
            .as_ref()
            .map(|(input, _)| input.read(cx).value().trim().to_owned())
            .unwrap_or_default();
        if url.is_empty() {
            self.close_link(window, cx);
            return;
        }
        self.link = None;
        self.editor_focus.focus(window, cx);
        cx.emit(BubbleEvent::SetLink(url));
        cx.notify();
    }

    fn close_link(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.link = None;
        self.editor_focus.focus(window, cx);
        cx.emit(BubbleEvent::CancelLink);
        cx.notify();
    }

    fn render_link(input: &Entity<InputState>, cx: &Context<'_, Self>) -> impl IntoElement {
        let confirm = t(cx, "bubble_confirm_link");
        let cancel = t(cx, "bubble_cancel_link");
        h_flex()
            .gap_1()
            .on_action(
                cx.listener(|toolbar, _: &Escape, window, cx| toolbar.close_link(window, cx)),
            )
            .child(
                Input::new(input)
                    .small()
                    .w(rems(12.5))
                    .aria_label(t(cx, "bubble_link")),
            )
            .child(
                Button::new("bubble-link-confirm")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Check))
                    .accessibility_label(confirm.clone())
                    .tooltip(confirm)
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        toolbar.submit_link(window, cx);
                    })),
            )
            .child(
                Button::new("bubble-link-cancel")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::X))
                    .accessibility_label(cancel.clone())
                    .tooltip(cancel)
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        toolbar.close_link(window, cx);
                    })),
            )
    }

    fn render_block_menu(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let current = self.state.block;
        let toolbar = cx.entity().downgrade();
        let opener = toolbar.clone();
        Button::new("bubble-block-type")
            .ghost()
            .small()
            .icon(Icon::new(block_icon(current)))
            .label(block_label(current, cx))
            .dropdown_caret(true)
            .accessibility_label(t(cx, "bubble_block_type"))
            .dropdown_menu(move |mut menu, _, cx| {
                for block in BUBBLE_BLOCK_TYPES {
                    let toolbar = toolbar.clone();
                    let item = PopupMenuItem::new(block_label(block, cx))
                        .icon(Icon::new(block_icon(block)))
                        .checked(block == current)
                        .on_click(move |_, _, cx| {
                            toolbar
                                .update(cx, |_, cx| cx.emit(BubbleEvent::TurnInto(block)))
                                .unwrap_or_default();
                        });
                    menu = menu.item(item);
                }
                menu
            })
            .on_open_change(move |open, _, cx| {
                opener
                    .update(cx, |toolbar, cx| {
                        toolbar.menu_open = *open;
                        cx.notify();
                    })
                    .unwrap_or_default();
            })
    }

    fn render_marks(&self, cx: &Context<'_, Self>) -> Vec<Button> {
        MARKS
            .into_iter()
            .map(|(kind, key, icon)| {
                let label = t(cx, key);
                let action = mark_action(kind);
                let focus = self.editor_focus.clone();
                Button::new(key)
                    .ghost()
                    .small()
                    .icon(Icon::new(icon))
                    .toggled(self.state.marks.contains(&kind))
                    .accessibility_label(label.clone())
                    .tooltip_with_action(label, action.as_ref(), Some(EDITOR_CONTEXT))
                    .on_click(move |_, window, cx| {
                        focus.focus(window, cx);
                        focus.dispatch_action(action.as_ref(), window, cx);
                    })
            })
            .collect()
    }
}

fn divider(cx: &App) -> impl IntoElement {
    div()
        .mx_0p5()
        .w_px()
        .h_4()
        .bg(MarginTheme::palette(cx).border_divider)
}

impl Render for BubbleToolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let bar = h_flex()
            .id("bubble-toolbar")
            .occlude()
            .role(Role::Toolbar)
            .gap_0p5()
            .px_1()
            .py_1()
            .bg(palette.surface(0))
            .border_1()
            .border_color(palette.border_secondary)
            .rounded(rems(0.75))
            .shadow_md();
        if let Some((input, _)) = &self.link {
            let input = input.clone();
            return bar.child(Self::render_link(&input, cx));
        }
        let bar = if self.state.in_table {
            bar
        } else {
            bar.child(self.render_block_menu(cx)).child(divider(cx))
        };
        let link = t(cx, "bubble_link");
        bar.children(self.render_marks(cx))
            .child(divider(cx))
            .child(
                Button::new("bubble-link")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Link))
                    .accessibility_label(link.clone())
                    .tooltip_with_action(link, &InsertLink, Some(EDITOR_CONTEXT))
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        toolbar.open_link(window, cx);
                    })),
            )
    }
}
