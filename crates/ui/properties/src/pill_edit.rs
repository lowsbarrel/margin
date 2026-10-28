use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Styled, Subscription, Window, div,
};
use margin_theme::Palette;

use crate::panel_actions::{Cancel, FIELD_CONTEXT};
use crate::properties_panel::PropertiesPanel;

pub(crate) struct PillEdit {
    index: usize,
    input: Entity<InputState>,
    _subscription: Subscription,
}

impl PillEdit {
    pub(crate) fn new(
        key: &str,
        index: usize,
        text: String,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        let owner = key.to_owned();
        let subscription = cx.subscribe_in(&input, window, move |panel, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                panel.update_pills(&owner, window, cx, |pills, window, cx| {
                    pills.close_edit(window, cx)
                });
            }
        });
        Self {
            index,
            input,
            _subscription: subscription,
        }
    }

    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn text(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    pub(crate) fn render(
        &self,
        palette: &Palette,
        cancel: impl Fn(&Cancel, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .key_context(FIELD_CONTEXT)
            .px_2()
            .rounded_full()
            .border_1()
            .border_color(palette.bg_brand)
            .on_action(cancel)
            .child(Input::new(&self.input).appearance(false).small())
            .into_any_element()
    }
}
