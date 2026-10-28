use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, IntoElement,
    Subscription, Window,
};
use margin_markdown::{Property, PropertyKind};

use crate::effect::{CaretAt, Effect, FocusTarget};
use crate::pills::Pills;
use crate::properties_panel::PropertiesPanel;
use crate::value_editor::ValueEditor;

pub(crate) struct Row {
    property: Property,
    kind: PropertyKind,
    focus: FocusHandle,
    key_input: Entity<InputState>,
    value: ValueEditor,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn kind_icon(kind: PropertyKind) -> IconName {
    match kind {
        PropertyKind::Text => IconName::TextAlignStart,
        PropertyKind::List => IconName::List,
        PropertyKind::Number => IconName::Binary,
        PropertyKind::Checkbox => IconName::SquareCheck,
        PropertyKind::Date => IconName::Calendar,
        PropertyKind::Datetime => IconName::Clock,
        PropertyKind::Tags => IconName::Tags,
        PropertyKind::Aliases => IconName::Forward,
        PropertyKind::Raw => IconName::Info,
    }
}

pub(crate) fn row_listener<E: ?Sized + 'static>(
    key: &str,
    cx: &Context<'_, PropertiesPanel>,
    handler: impl Fn(&mut Row, &E, &mut Window, &mut Context<'_, PropertiesPanel>) -> Effect + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let key = key.to_owned();
    cx.listener(move |panel: &mut PropertiesPanel, event: &E, window, cx| {
        panel.update_row(&key, window, cx, |row, window, cx| {
            handler(row, event, window, cx)
        });
    })
}

pub(crate) fn subscribe<Emitter: EventEmitter<InputEvent>>(
    input: &Entity<Emitter>,
    key: &str,
    window: &Window,
    cx: &mut Context<'_, PropertiesPanel>,
    handler: fn(&mut Row, &InputEvent, &mut Window, &mut Context<'_, PropertiesPanel>) -> Effect,
) -> Subscription {
    let key = key.to_owned();
    cx.subscribe_in(
        input,
        window,
        move |panel, _, event: &InputEvent, window, cx| {
            panel.update_row(&key, window, cx, |row, window, cx| {
                handler(row, event, window, cx)
            });
        },
    )
}

impl Row {
    pub(crate) fn new(
        property: Property,
        kind: PropertyKind,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Self {
        let key = property.key.clone();
        let key_input = cx.new(|cx| InputState::new(window, cx).default_value(key.clone()));
        let mut subscriptions = vec![subscribe(&key_input, &key, window, cx, Self::key_event)];
        let (value, subscription) = ValueEditor::new(&property, kind, window, cx);
        subscriptions.extend(subscription);
        Self {
            property,
            kind,
            focus: cx.focus_handle().tab_stop(true),
            key_input,
            value,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.property.key
    }

    pub(crate) const fn kind(&self) -> PropertyKind {
        self.kind
    }

    pub(crate) const fn property(&self) -> &Property {
        &self.property
    }

    pub(crate) fn is_same(&self, property: &Property, kind: PropertyKind) -> bool {
        self.kind == kind
            && self.property.key == property.key
            && self.property.value == property.value
            && self.property.raw == property.raw
    }

    pub(crate) const fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    pub(crate) fn pills_mut(&mut self) -> Option<&mut Pills> {
        self.value.pills_mut()
    }

    pub(crate) fn focus_key(&self, window: &mut Window, cx: &mut Context<'_, PropertiesPanel>) {
        self.key_input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
    }

    pub(crate) fn focus_value(
        &mut self,
        at: CaretAt,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        self.value.focus(at, &self.focus, window, cx);
    }

    pub(crate) fn reset_key(&self, window: &mut Window, cx: &mut Context<'_, PropertiesPanel>) {
        let key = self.property.key.clone();
        self.key_input
            .update(cx, |input, cx| input.set_value(key, window, cx));
    }

    fn key_event(
        &mut self,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        match event {
            InputEvent::PressEnter { .. } => self.submit_key(cx),
            InputEvent::Blur => {
                let next = self.key_input.read(cx).value().trim().to_owned();
                if next == self.property.key {
                    Effect::Nothing
                } else {
                    Effect::Rename(next, None)
                }
            }
            _ => Effect::Nothing,
        }
    }

    pub(crate) fn submit_key(&self, cx: &Context<'_, PropertiesPanel>) -> Effect {
        let next = self.key_input.read(cx).value().trim().to_owned();
        if next == self.property.key {
            Effect::Focus(FocusTarget::Value(next))
        } else {
            Effect::Rename(next.clone(), Some(FocusTarget::Value(next)))
        }
    }

    pub(crate) fn cancel_key(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        self.reset_key(window, cx);
        Effect::Focus(FocusTarget::Row(self.property.key.clone()))
    }

    pub(crate) fn value_event(
        &mut self,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        self.value
            .on_event(&self.property, self.kind, event, window, cx)
    }

    pub(crate) fn cancel_value(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Effect {
        self.value.cancel(&self.property, self.kind, window, cx);
        Effect::Focus(FocusTarget::Row(self.property.key.clone()))
    }

    pub(crate) fn render_key(&self) -> AnyElement {
        Input::new(&self.key_input)
            .appearance(false)
            .small()
            .into_any_element()
    }

    pub(crate) fn render_value(
        &self,
        window: &Window,
        cx: &Context<'_, PropertiesPanel>,
    ) -> AnyElement {
        self.value.render(&self.property, window, cx)
    }
}
