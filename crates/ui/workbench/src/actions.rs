#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use std::rc::Rc;

use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate, actions};

pub const WORKSPACE_CONTEXT: &str = "Workspace";

actions!(
    margin,
    [
        OpenSpotlight,
        SearchVault,
        NewNote,
        ToggleSidebar,
        ToggleTerminal,
        ToggleViewMode,
        CloseTab,
        ReopenClosedTab,
        FindInNote,
        ReplaceInNote,
        PasteAsPlainText,
        OpenSettings,
        LockVault,
        SyncNow,
        ToggleBacklinks,
        ToggleHistory,
        OpenTrash,
        TogglePinTab,
        CloseOtherTabs,
        CloseAllTabs,
        CheckForUpdates,
        Quit,
    ]
);

pub fn binding(
    keystrokes: &str,
    action: impl Action,
    context: Option<&str>,
    cx: &App,
) -> KeyBinding {
    let predicate = context.map(|context| {
        Rc::new(
            KeyBindingContextPredicate::parse(context)
                .expect("Margin's key contexts are valid predicates"),
        )
    });
    KeyBinding::load(keystrokes, Box::new(action), predicate, false, None, cx)
        .expect("Margin's shortcuts are valid keystrokes on every platform")
}

pub fn bind_keys(cx: &mut App) {
    let context = Some(WORKSPACE_CONTEXT);
    let bindings = [
        binding("secondary-p", OpenSpotlight, context, cx),
        binding("secondary-k", OpenSpotlight, context, cx),
        binding("secondary-shift-f", SearchVault, context, cx),
        binding("secondary-n", NewNote, context, cx),
        binding("secondary-\\", ToggleSidebar, context, cx),
        binding("secondary-`", ToggleTerminal, context, cx),
        binding("secondary-shift-e", ToggleViewMode, context, cx),
        binding("secondary-w", CloseTab, context, cx),
        binding("secondary-shift-t", ReopenClosedTab, context, cx),
        binding("secondary-f", FindInNote, context, cx),
        binding("secondary-h", ReplaceInNote, context, cx),
        binding("secondary-shift-v", PasteAsPlainText, context, cx),
        binding("secondary-,", OpenSettings, context, cx),
        binding("secondary-q", Quit, None, cx),
    ];
    cx.bind_keys(bindings);
}
