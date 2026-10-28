use gpui_kit::{App, KeyBinding, actions};

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
    ]
);

pub fn bind_keys(cx: &mut App) {
    let context = Some(WORKSPACE_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-p", OpenSpotlight, context),
        KeyBinding::new("secondary-k", OpenSpotlight, context),
        KeyBinding::new("secondary-shift-f", SearchVault, context),
        KeyBinding::new("secondary-n", NewNote, context),
        KeyBinding::new("secondary-\\", ToggleSidebar, context),
        KeyBinding::new("secondary-`", ToggleTerminal, context),
        KeyBinding::new("secondary-shift-e", ToggleViewMode, context),
        KeyBinding::new("secondary-w", CloseTab, context),
        KeyBinding::new("secondary-shift-t", ReopenClosedTab, context),
        KeyBinding::new("secondary-f", FindInNote, context),
        KeyBinding::new("secondary-h", ReplaceInNote, context),
        KeyBinding::new("secondary-shift-v", PasteAsPlainText, context),
        KeyBinding::new("secondary-,", OpenSettings, context),
    ]);
}
