use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, AppContext, ClickEvent, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Window, div, px,
};
use margin_i18n::t;
use margin_markdown::{PropertyKind, PropertyValue, as_list};
use margin_theme::{MarginTheme, Palette};

use crate::effect::{Effect, FocusTarget};
use crate::panel_actions::{
    Activate, Cancel, FIELD_CONTEXT, MoveDown, MoveLeft, MoveRight, MoveUp, PILL_CONTEXT,
    PILL_INPUT_CONTEXT, Remove,
};
use crate::pill_edit::PillEdit;
use crate::properties_panel::PropertiesPanel;
use crate::property_link::{is_tag_name, link_of};

pub(crate) struct Pills {
    key: String,
    kind: PropertyKind,
    items: Vec<String>,
    handles: Vec<FocusHandle>,
    input: Entity<InputState>,
    editing: Option<PillEdit>,
    _subscription: Subscription,
}

impl Pills {
    pub(crate) fn new(
        key: &str,
        kind: PropertyKind,
        value: &PropertyValue,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Self {
        let items = as_list(value);
        let placeholder = if items.is_empty() {
            t(cx, "editor_property_empty")
        } else {
            SharedString::default()
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let owner = key.to_owned();
        let subscription = cx.subscribe_in(&input, window, move |panel, _, event, window, cx| {
            panel.update_pills(&owner, window, cx, |pills, _, cx| {
                pills.on_input_event(event, cx)
            });
        });
        Self {
            key: key.to_owned(),
            kind,
            handles: items
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            items,
            input,
            editing: None,
            _subscription: subscription,
        }
    }

    pub(crate) fn focus_input(&self, window: &mut Window, cx: &mut Context<'_, PropertiesPanel>) {
        self.input.update(cx, |input, cx| {
            input.focus(window, cx);
            let end = input.value().len();
            input.set_selected_range(end..end, cx);
        });
    }

    pub(crate) fn focus_pill(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        match self.handles.get(index) {
            Some(handle) => handle.focus(window, cx),
            None => self.focus_input(window, cx),
        }
    }

    fn pill(&self, index: usize) -> FocusTarget {
        FocusTarget::Pill(self.key.clone(), index)
    }

    fn value_target(&self) -> FocusTarget {
        FocusTarget::Value(self.key.clone())
    }

    fn listener<E: ?Sized + 'static>(
        &self,
        cx: &Context<'_, PropertiesPanel>,
        handler: impl Fn(&mut Self, &E, &mut Window, &mut Context<'_, PropertiesPanel>) -> Effect
        + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let key = self.key.clone();
        cx.listener(move |panel: &mut PropertiesPanel, event: &E, window, cx| {
            panel.update_pills(&key, window, cx, |pills, window, cx| {
                handler(pills, event, window, cx)
            });
        })
    }

    fn clean(&self, text: &str) -> String {
        if self.kind == PropertyKind::List {
            text.to_owned()
        } else {
            let trimmed = text.trim();
            trimmed.strip_prefix('#').unwrap_or(trimmed).to_owned()
        }
    }

    fn accept(&self, text: &str, except: Option<usize>) -> Result<String, Effect> {
        let value = self.clean(text);
        if value.trim().is_empty() {
            return Err(Effect::Nothing);
        }
        if self.kind == PropertyKind::Tags && !is_tag_name(&value) {
            return Err(Effect::Invalid("editor_property_tag_invalid"));
        }
        if self.kind == PropertyKind::List {
            return Ok(value);
        }
        let duplicate = self
            .items
            .iter()
            .enumerate()
            .position(|(index, item)| Some(index) != except && self.clean(item) == value);
        match duplicate {
            Some(index) => Err(Effect::Focus(self.pill(index))),
            None => Ok(value),
        }
    }

    fn add(&self, text: &str, then: Option<FocusTarget>) -> Effect {
        match self.accept(text, None) {
            Ok(value) => {
                let mut items = self.items.clone();
                items.push(value);
                Effect::Commit(PropertyValue::List(items), then)
            }
            Err(Effect::Focus(_)) if then.is_none() => Effect::Nothing,
            Err(effect) => effect,
        }
    }

    fn without(&self, index: usize) -> PropertyValue {
        let mut items = self.items.clone();
        if index < items.len() {
            items.remove(index);
        }
        PropertyValue::List(items)
    }

    fn input_text(&self, cx: &Context<'_, PropertiesPanel>) -> String {
        self.input.read(cx).value().to_string()
    }

    pub(crate) fn on_input_event(
        &self,
        event: &InputEvent,
        cx: &Context<'_, PropertiesPanel>,
    ) -> Effect {
        let text = self.input_text(cx);
        match event {
            InputEvent::PressEnter { .. } if text.is_empty() => {
                Effect::Focus(FocusTarget::Row(self.key.clone()))
            }
            InputEvent::PressEnter { .. } => self.add(&text, Some(self.value_target())),
            InputEvent::Blur if !text.is_empty() => self.add(&text, None),
            _ => Effect::Nothing,
        }
    }

    pub(crate) fn backspace_at_start(&self, cx: &Context<'_, PropertiesPanel>) -> Effect {
        match self.items.len().checked_sub(1) {
            Some(last) if self.input_text(cx).is_empty() => Effect::Focus(self.pill(last)),
            _ => Effect::Nothing,
        }
    }

    pub(crate) fn start_edit(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        if let Some(text) = self.items.get(index).cloned() {
            self.editing = Some(PillEdit::new(&self.key, index, text, window, cx));
        }
        Effect::Nothing
    }

    pub(crate) fn close_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        let Some(edit) = self.editing.take() else {
            return Effect::Nothing;
        };
        let text = edit.text(cx);
        let index = edit.index();
        let outcome = if text.is_empty() {
            Err(Effect::Nothing)
        } else {
            self.accept(&text, Some(index))
        };
        match outcome {
            Ok(value) => {
                let mut items = self.items.clone();
                if let Some(item) = items.get_mut(index) {
                    *item = value;
                }
                Effect::Commit(PropertyValue::List(items), Some(self.pill(index)))
            }
            Err(effect) => {
                self.focus_pill(index, window, cx);
                match effect {
                    Effect::Invalid(message) => Effect::Invalid(message),
                    _ => Effect::Nothing,
                }
            }
        }
    }

    pub(crate) fn cancel_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        if let Some(edit) = self.editing.take() {
            self.focus_pill(edit.index(), window, cx);
        }
        Effect::Nothing
    }

    fn open(&self, index: usize) -> Effect {
        let Some(item) = self.items.get(index) else {
            return Effect::Nothing;
        };
        if self.kind == PropertyKind::Tags {
            return Effect::OpenTag(item.trim_start_matches('#').to_owned());
        }
        link_of(item).map_or(Effect::Nothing, |link| {
            Effect::OpenLink(link.target().clone())
        })
    }

    fn label(&self, item: &str) -> String {
        if self.kind == PropertyKind::Tags {
            return item.strip_prefix('#').unwrap_or(item).to_owned();
        }
        link_of(item).map_or_else(|| item.to_owned(), |link| link.label().to_owned())
    }

    fn is_link(&self, item: &str) -> bool {
        self.kind == PropertyKind::Tags || link_of(item).is_some()
    }

    pub(crate) fn render(&self, window: &Window, cx: &Context<'_, PropertiesPanel>) -> AnyElement {
        let palette = MarginTheme::palette(cx).clone();
        let mut pills = Vec::with_capacity(self.items.len());
        for (index, (item, handle)) in self.items.iter().zip(&self.handles).enumerate() {
            pills.push(
                match self.editing.as_ref().filter(|edit| edit.index() == index) {
                    Some(edit) => edit.render(
                        &palette,
                        self.listener(cx, |pills, _: &Cancel, window, cx| {
                            pills.cancel_edit(window, cx)
                        }),
                    ),
                    None => self.render_pill(index, item, handle, &palette, window, cx),
                },
            );
        }
        div()
            .flex()
            .flex_1()
            .flex_wrap()
            .items_center()
            .gap_x_2()
            .gap_y_1()
            .min_w_0()
            .px_2()
            .py_1()
            .children(pills)
            .child(
                div()
                    .key_context(format!("{FIELD_CONTEXT} {PILL_INPUT_CONTEXT}").as_str())
                    .flex_1()
                    .min_w(px(48.0))
                    .on_action(
                        self.listener(cx, |pills, _: &Remove, _, cx| pills.backspace_at_start(cx)),
                    )
                    .on_action(self.listener(cx, |pills, _: &Cancel, _, _| {
                        Effect::Focus(FocusTarget::Row(pills.key.clone()))
                    }))
                    .child(Input::new(&self.input).appearance(false).small()),
            )
            .into_any_element()
    }

    fn render_pill(
        &self,
        index: usize,
        item: &str,
        handle: &FocusHandle,
        palette: &Palette,
        window: &Window,
        cx: &Context<'_, PropertiesPanel>,
    ) -> AnyElement {
        let tags = self.kind == PropertyKind::Tags;
        let content = div()
            .id(("pill-content", index))
            .overflow_hidden()
            .child(self.label(item))
            .when(self.is_link(item), |content| {
                content
                    .cursor_pointer()
                    .when(!tags, |content| content.text_color(palette.text_brand))
                    .on_click(self.listener(cx, move |pills, _, _, _| pills.open(index)))
            });
        let remove = div()
            .id(("pill-remove", index))
            .flex()
            .items_center()
            .text_color(palette.text_tertiary)
            .hover(|remove| remove.text_color(palette.text_primary))
            .cursor_pointer()
            .child(Icon::new(IconName::X).size_3())
            .on_click(self.listener(cx, move |pills, _, _, _| {
                Effect::Commit(pills.without(index), None)
            }));
        div()
            .id(("pill", index))
            .track_focus(handle)
            .key_context(PILL_CONTEXT)
            .flex()
            .items_center()
            .gap_1()
            .max_w_full()
            .rounded_full()
            .border_1()
            .border_color(if handle.is_focused(window) {
                palette.bg_brand
            } else {
                gpui_kit::transparent_black()
            })
            .when(tags, |pill| {
                pill.pl_2()
                    .pr_1()
                    .bg(palette.brand(12.0))
                    .text_color(palette.text_brand)
            })
            .on_click(
                self.listener(cx, move |pills, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        pills.start_edit(index, window, cx)
                    } else {
                        Effect::Nothing
                    }
                }),
            )
            .on_action(self.listener(cx, move |pills, _: &Activate, window, cx| {
                pills.start_edit(index, window, cx)
            }))
            .on_action(self.listener(cx, |pills, _: &Cancel, _, _| {
                Effect::Focus(pills.value_target())
            }))
            .on_action(self.listener(cx, move |pills, _: &Remove, _, _| {
                let then = index
                    .checked_sub(1)
                    .map_or_else(|| pills.value_target(), |previous| pills.pill(previous));
                Effect::Commit(pills.without(index), Some(then))
            }))
            .on_action(self.listener(cx, |pills, _: &MoveUp, _, _| Effect::Focus(pills.pill(0))))
            .on_action(self.listener(cx, |pills, _: &MoveDown, _, _| {
                Effect::Focus(pills.value_target())
            }))
            .on_action(self.listener(cx, move |pills, _: &MoveLeft, _, _| {
                Effect::Focus(pills.pill(index.saturating_sub(1)))
            }))
            .on_action(self.listener(cx, move |pills, _: &MoveRight, _, _| {
                Effect::Focus(pills.pill(index + 1))
            }))
            .child(content)
            .child(remove)
            .into_any_element()
    }
}
