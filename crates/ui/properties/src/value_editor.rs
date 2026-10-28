use std::ops::Range;

use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::{
    AnyElement, AppContext, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div,
};
use margin_i18n::t;
use margin_markdown::{Property, PropertyKind, PropertyValue};
use margin_theme::{MONO_FONT_FAMILY, MarginTheme};

use crate::effect::{CaretAt, Effect, FocusTarget};
use crate::panel_actions::{Activate, CHECKBOX_CONTEXT, Cancel, FIELD_CONTEXT};
use crate::pills::Pills;
use crate::properties_panel::PropertiesPanel;
use crate::property_edits::{date_parts, parse_number};
use crate::property_link::{PropertyLink, link_of};
use crate::property_row::{Row, row_listener, subscribe};

pub(crate) enum ValueEditor {
    Raw(SharedString),
    Text {
        input: Entity<TextareaState>,
        link: Option<PropertyLink>,
        editing: bool,
    },
    Field(Entity<InputState>),
    Checkbox(FocusHandle),
    List(Pills),
}

fn display_text(value: &PropertyValue, kind: PropertyKind) -> String {
    match (kind, value) {
        (PropertyKind::Date, PropertyValue::Text(text)) => text.chars().take(10).collect(),
        (PropertyKind::Datetime, PropertyValue::Text(text)) => text.replace(' ', "T"),
        (PropertyKind::Date | PropertyKind::Datetime, _) => String::new(),
        (_, value) => value.to_plain_string(),
    }
}

fn parse_field(text: &str, kind: PropertyKind) -> Option<PropertyValue> {
    let text = text.trim();
    if text.is_empty() {
        return Some(PropertyValue::Null);
    }
    match kind {
        PropertyKind::Number => parse_number(text).map(PropertyValue::Number),
        PropertyKind::Date => match date_parts(text) {
            Some((date, None)) => Some(PropertyValue::Text(date.to_owned())),
            _ => None,
        },
        _ => {
            let normalized = text.replacen(' ', "T", 1);
            match date_parts(&normalized) {
                Some((_, Some(time))) if time.len() == 5 => {
                    Some(PropertyValue::Text(format!("{normalized}:00")))
                }
                Some((_, Some(_))) => Some(PropertyValue::Text(normalized)),
                _ => None,
            }
        }
    }
}

fn raw_value(raw: &str) -> SharedString {
    let rest = raw.split_once(':').map_or(raw, |(_, rest)| rest);
    SharedString::from(rest.trim().to_owned())
}

fn caret_range(text: &str, at: CaretAt) -> Range<usize> {
    let end = text.len();
    match at {
        CaretAt::Start => 0..0,
        CaretAt::End => end..end,
        CaretAt::All => 0..end,
    }
}

fn original_text(property: &Property, kind: PropertyKind) -> String {
    display_text(&property.value, kind)
}

fn is_checked(property: &Property) -> bool {
    property.value == PropertyValue::Bool(true)
}

fn toggle(property: &Property) -> Effect {
    Effect::Commit(
        PropertyValue::Bool(!is_checked(property)),
        Some(FocusTarget::Value(property.key.clone())),
    )
}

fn field(key: &str, cx: &Context<'_, PropertiesPanel>) -> Div {
    div()
        .key_context(FIELD_CONTEXT)
        .flex_1()
        .min_w_0()
        .on_action(row_listener(key, cx, |row, _: &Cancel, window, cx| {
            row.cancel_value(window, cx)
        }))
}

fn text_field(key: &str, cx: &Context<'_, PropertiesPanel>) -> Div {
    field(key, cx)
        .key_context(format!("{FIELD_CONTEXT} PropertyText").as_str())
        .on_action(row_listener(key, cx, |row, _: &Activate, window, cx| {
            let enter = InputEvent::PressEnter {
                secondary: false,
                shift: false,
            };
            row.value_event(&enter, window, cx)
        }))
}

impl ValueEditor {
    pub(crate) fn new(
        property: &Property,
        kind: PropertyKind,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> (Self, Option<Subscription>) {
        let key = property.key.as_str();
        let empty = t(cx, "editor_property_empty");
        match kind {
            PropertyKind::Raw => {
                let raw = property.raw.as_deref().unwrap_or_default();
                (Self::Raw(raw_value(raw)), None)
            }
            PropertyKind::List | PropertyKind::Tags | PropertyKind::Aliases => (
                Self::List(Pills::new(key, kind, &property.value, window, cx)),
                None,
            ),
            PropertyKind::Checkbox => (Self::Checkbox(cx.focus_handle().tab_stop(true)), None),
            PropertyKind::Number | PropertyKind::Date | PropertyKind::Datetime => {
                let text = display_text(&property.value, kind);
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(empty)
                        .default_value(text)
                });
                let subscription = subscribe(&input, key, window, cx, Row::value_event);
                (Self::Field(input), Some(subscription))
            }
            PropertyKind::Text => {
                let text = property.value.to_plain_string();
                let link = link_of(&text);
                let input = cx.new(|cx| {
                    let mut state = TextareaState::new(window, cx)
                        .auto_grow(1, 6)
                        .placeholder(empty)
                        .default_value(text);
                    state.set_submit_on_enter(true, cx);
                    state
                });
                let subscription = subscribe(&input, key, window, cx, Row::value_event);
                let editor = Self::Text {
                    input,
                    link,
                    editing: false,
                };
                (editor, Some(subscription))
            }
        }
    }

    pub(crate) fn pills_mut(&mut self) -> Option<&mut Pills> {
        match self {
            Self::List(pills) => Some(pills),
            _ => None,
        }
    }

    pub(crate) fn focus(
        &mut self,
        at: CaretAt,
        row: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        match self {
            Self::Raw(_) => row.focus(window, cx),
            Self::Checkbox(handle) => handle.focus(window, cx),
            Self::List(pills) => pills.focus_input(window, cx),
            Self::Field(input) => input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.set_selected_range(caret_range(&input.value(), at), cx);
            }),
            Self::Text { input, editing, .. } => {
                *editing = true;
                input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.set_selected_range(caret_range(&input.value(), at), cx);
                });
            }
        }
    }

    pub(crate) fn on_event(
        &mut self,
        property: &Property,
        kind: PropertyKind,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        let then = match event {
            InputEvent::PressEnter { .. } => Some(FocusTarget::Row(property.key.clone())),
            InputEvent::Blur => None,
            _ => return Effect::Nothing,
        };
        let original = original_text(property, kind);
        let (text, parsed) = match self {
            Self::Text { input, editing, .. } => {
                let text = input.read(cx).value().to_string();
                let text = if then.is_some() {
                    text
                } else {
                    text.trim_end().to_owned()
                };
                if text == original {
                    *editing = false;
                }
                let value = PropertyValue::Text(text.clone());
                (text, Some(value))
            }
            Self::Field(input) => {
                let text = input.read(cx).value().to_string();
                let parsed = parse_field(&text, kind);
                (text, parsed)
            }
            _ => return Effect::Nothing,
        };
        match parsed {
            Some(value) if text != original => Effect::Commit(value, then),
            _ => {
                if let Self::Field(input) = self {
                    input.update(cx, |input, cx| input.set_value(original, window, cx));
                }
                then.map_or(Effect::Nothing, Effect::Focus)
            }
        }
    }

    pub(crate) fn cancel(
        &mut self,
        property: &Property,
        kind: PropertyKind,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        let original = original_text(property, kind);
        match self {
            Self::Text { input, editing, .. } => {
                *editing = false;
                input.update(cx, |input, cx| input.set_value(original, window, cx));
            }
            Self::Field(input) => {
                input.update(cx, |input, cx| input.set_value(original, window, cx));
            }
            _ => {}
        }
    }

    pub(crate) fn render(
        &self,
        property: &Property,
        window: &Window,
        cx: &Context<'_, PropertiesPanel>,
    ) -> AnyElement {
        let palette = MarginTheme::palette(cx).clone();
        let key = property.key.as_str();
        match self {
            Self::Raw(text) => div()
                .px_2()
                .py_1()
                .text_color(palette.text_tertiary)
                .font_family(MONO_FONT_FAMILY)
                .child(text.clone())
                .into_any_element(),
            Self::Checkbox(handle) => div()
                .id("checkbox")
                .track_focus(handle)
                .key_context(CHECKBOX_CONTEXT)
                .px_2()
                .rounded_md()
                .border_2()
                .border_color(if handle.is_focused(window) {
                    palette.bg_brand
                } else {
                    gpui_kit::transparent_black()
                })
                .on_action(row_listener(key, cx, |row, _: &Activate, _, _| {
                    toggle(row.property())
                }))
                .on_action(row_listener(key, cx, |row, _: &Cancel, _, _| {
                    Effect::Focus(FocusTarget::Row(row.key().to_owned()))
                }))
                .child(
                    Checkbox::new("checked")
                        .checked(is_checked(property))
                        .tab_stop(false)
                        .on_click(row_listener(key, cx, |row, _: &bool, _, _| {
                            toggle(row.property())
                        })),
                )
                .into_any_element(),
            Self::List(pills) => pills.render(window, cx),
            Self::Field(input) => field(key, cx)
                .child(Input::new(input).appearance(false).small())
                .into_any_element(),
            Self::Text {
                link: Some(link),
                editing: false,
                ..
            } => {
                let target = link.target().clone();
                div()
                    .id("link")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .cursor_text()
                    .on_click(row_listener(key, cx, |row, _, window, cx| {
                        row.focus_value(CaretAt::End, window, cx);
                        Effect::Nothing
                    }))
                    .child(
                        div()
                            .id("label")
                            .text_color(palette.text_brand)
                            .cursor_pointer()
                            .on_click(cx.listener(move |panel, _, window, cx| {
                                cx.stop_propagation();
                                let open = Effect::OpenLink(target.clone());
                                panel.apply_effect(None, open, window, cx);
                            }))
                            .child(link.label().to_owned()),
                    )
                    .child(
                        Icon::new(IconName::Pencil)
                            .size_3()
                            .text_color(palette.text_tertiary),
                    )
                    .into_any_element()
            }
            Self::Text { input, .. } => text_field(key, cx)
                .child(Textarea::new(input).appearance(false).small())
                .into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_fields_parse_or_reject() {
        assert_eq!(
            parse_field(" 4.5 ", PropertyKind::Number),
            Some(PropertyValue::Number(4.5))
        );
        assert_eq!(parse_field("soon", PropertyKind::Number), None);
        assert_eq!(
            parse_field("", PropertyKind::Date),
            Some(PropertyValue::Null)
        );
        assert_eq!(
            parse_field("2024-05-01 09:30", PropertyKind::Datetime),
            Some(PropertyValue::Text("2024-05-01T09:30:00".to_owned()))
        );
        assert_eq!(parse_field("2024-13-45x", PropertyKind::Date), None);
    }
}
