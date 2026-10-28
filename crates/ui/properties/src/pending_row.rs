use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, Subscription, Window};

use crate::effect::FocusAfter;
use crate::properties_panel::PropertiesPanel;

pub(crate) struct PendingRow {
    input: Entity<InputState>,
    _subscription: Subscription,
}

impl PendingRow {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, PropertiesPanel>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let subscription =
            cx.subscribe_in(&input, window, |panel, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    panel.commit_new(FocusAfter::Value, window, cx);
                }
                InputEvent::Blur => panel.leave_new(window, cx),
                _ => {}
            });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self {
            input,
            _subscription: subscription,
        }
    }

    pub(crate) const fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn key(&self, cx: &App) -> String {
        self.input.read(cx).value().trim().to_owned()
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }
}
