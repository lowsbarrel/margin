#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use gpui_kit::{App, Global, actions};
use margin_workbench::binding;

pub(crate) const ONBOARDING_CONTEXT: &str = "Onboarding";

actions!(
    onboarding,
    [
        Confirm,
        Cancel,
        PreviousStep,
        NextStep,
        SelectPrevious,
        SelectNext,
        RenameVault,
        RemoveVault,
    ]
);

struct KeysBound;

impl Global for KeysBound {}

pub(crate) fn bind_keys_once(cx: &mut App) {
    if cx.has_global::<KeysBound>() {
        return;
    }
    cx.set_global(KeysBound);
    let context = Some(ONBOARDING_CONTEXT);
    let bindings = [
        binding("enter", Confirm, context, cx),
        binding("escape", Cancel, context, cx),
        binding("left", PreviousStep, context, cx),
        binding("right", NextStep, context, cx),
        binding("up", SelectPrevious, context, cx),
        binding("down", SelectNext, context, cx),
        binding("f2", RenameVault, context, cx),
        binding("delete", RemoveVault, context, cx),
        binding("secondary-backspace", RemoveVault, context, cx),
    ];
    cx.bind_keys(bindings);
}
