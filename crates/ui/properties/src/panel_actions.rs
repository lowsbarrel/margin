#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use gpui_kit::{App, Global, KeyBinding, actions};

pub(crate) const ROW_CONTEXT: &str = "PropertyRow";
pub(crate) const HEADING_CONTEXT: &str = "PropertiesHeading";
pub(crate) const ADD_CONTEXT: &str = "PropertiesAdd";
pub(crate) const PILL_CONTEXT: &str = "PropertyPill";
pub(crate) const PILL_INPUT_CONTEXT: &str = "PropertyPillInput";
pub(crate) const CHECKBOX_CONTEXT: &str = "PropertyCheckbox";
pub(crate) const FIELD_CONTEXT: &str = "PropertyField";
pub(crate) const KEY_CONTEXT: &str = "PropertyKey";

actions!(
    properties,
    [
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        Activate,
        ActivateAtEnd,
        ActivateAtStart,
        AddProperty,
        Remove,
        LeaveToBody,
        Cancel,
        FocusNext,
        FocusPrevious,
    ]
);

struct KeysBound;

impl Global for KeysBound {}

pub(crate) fn bind_keys_once(cx: &mut App) {
    if cx.has_global::<KeysBound>() {
        return;
    }
    cx.set_global(KeysBound);
    let row = Some(ROW_CONTEXT);
    let heading = Some(HEADING_CONTEXT);
    let add = Some(ADD_CONTEXT);
    let pill = Some(PILL_CONTEXT);
    let checkbox = Some(CHECKBOX_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, row),
        KeyBinding::new("k", MoveUp, row),
        KeyBinding::new("down", MoveDown, row),
        KeyBinding::new("j", MoveDown, row),
        KeyBinding::new("left", MoveLeft, row),
        KeyBinding::new("h", MoveLeft, row),
        KeyBinding::new("right", MoveRight, row),
        KeyBinding::new("l", MoveRight, row),
        KeyBinding::new("enter", Activate, row),
        KeyBinding::new("space", Activate, row),
        KeyBinding::new("shift-a", ActivateAtEnd, row),
        KeyBinding::new("i", ActivateAtStart, row),
        KeyBinding::new("o", AddProperty, row),
        KeyBinding::new("delete", Remove, row),
        KeyBinding::new("cmd-backspace", Remove, row),
        KeyBinding::new("ctrl-backspace", Remove, row),
        KeyBinding::new("home", LeaveToBody, row),
        KeyBinding::new("alt-down", LeaveToBody, row),
        KeyBinding::new("tab", FocusNext, row),
        KeyBinding::new("shift-tab", FocusPrevious, row),
        KeyBinding::new("left", MoveLeft, heading),
        KeyBinding::new("right", MoveRight, heading),
        KeyBinding::new("up", MoveUp, heading),
        KeyBinding::new("down", MoveDown, heading),
        KeyBinding::new("j", MoveDown, heading),
        KeyBinding::new("enter", Activate, heading),
        KeyBinding::new("space", Activate, heading),
        KeyBinding::new("enter", Activate, add),
        KeyBinding::new("space", Activate, add),
        KeyBinding::new("up", MoveUp, add),
        KeyBinding::new("k", MoveUp, add),
        KeyBinding::new("shift-tab", MoveUp, add),
        KeyBinding::new("down", MoveDown, add),
        KeyBinding::new("j", MoveDown, add),
        KeyBinding::new("tab", MoveDown, add),
        KeyBinding::new("enter", Activate, pill),
        KeyBinding::new("escape", Cancel, pill),
        KeyBinding::new("backspace", Remove, pill),
        KeyBinding::new("up", MoveUp, pill),
        KeyBinding::new("down", MoveDown, pill),
        KeyBinding::new("left", MoveLeft, pill),
        KeyBinding::new("right", MoveRight, pill),
        KeyBinding::new("space", Activate, checkbox),
        KeyBinding::new("enter", Cancel, checkbox),
        KeyBinding::new("escape", Cancel, checkbox),
        KeyBinding::new("escape", Cancel, Some(FIELD_CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some("PropertyField > Input")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("PropertyField > Input")),
        KeyBinding::new("enter", Activate, Some("PropertyText > Input")),
        KeyBinding::new("escape", Cancel, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some("PropertyKey > Input")),
        KeyBinding::new("backspace", Remove, Some(PILL_INPUT_CONTEXT)),
    ]);
}
