#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

use gpui_kit::component::input::InputState;
use gpui_kit::{Entity, Pixels, Point, Subscription};

use crate::model::WorldPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarPopover {
    Shape,
    Style,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasMenu {
    pub(crate) position: Point<Pixels>,
    pub(crate) confirming: bool,
}

pub(crate) struct TextEdit {
    pub(crate) at: WorldPoint,
    pub(crate) input: Entity<InputState>,
    pub(crate) _subscription: Subscription,
}
