#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use gpui_kit::{App, Menu, MenuItem, SystemMenuType, actions};
use margin_i18n::{Translator, t};
use margin_workbench::{
    CheckForUpdates, CloseTab, FindInNote, LockVault, NewNote, OpenSettings, OpenSpotlight,
    OpenTrash, PasteAsPlainText, Quit, ReopenClosedTab, ReplaceInNote, SearchVault, SyncNow,
    ToggleBacklinks, ToggleHistory, ToggleSidebar, ToggleTerminal, ToggleViewMode,
};

actions!(
    margin_app,
    [
        HideApp,
        HideOtherApps,
        ShowAllApps,
        MinimizeWindow,
        ZoomWindow
    ]
);

pub(crate) fn install(cx: &mut App) {
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    cx.on_action(|_: &MinimizeWindow, cx| {
        if let Some(window) = cx.active_window() {
            drop(window.update(cx, |_, window, _| window.minimize_window()));
        }
    });
    cx.on_action(|_: &ZoomWindow, cx| {
        if let Some(window) = cx.active_window() {
            drop(window.update(cx, |_, window, _| window.zoom_window()));
        }
    });
    set_menus(cx);
    cx.observe_global::<Translator>(set_menus).detach();
}

fn set_menus(cx: &mut App) {
    let menus = vec![
        Menu::new(t(cx, "app_display_name")).items([
            MenuItem::action(t(cx, "menu_settings"), OpenSettings),
            MenuItem::action(t(cx, "menu_check_for_updates"), CheckForUpdates),
            MenuItem::separator(),
            MenuItem::os_submenu(t(cx, "menu_services"), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu_hide"), HideApp),
            MenuItem::action(t(cx, "menu_hide_others"), HideOtherApps),
            MenuItem::action(t(cx, "menu_show_all"), ShowAllApps),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu_quit"), Quit),
        ]),
        Menu::new(t(cx, "menu_file")).items([
            MenuItem::action(t(cx, "menu_new_note"), NewNote),
            MenuItem::action(t(cx, "menu_spotlight"), OpenSpotlight),
            MenuItem::action(t(cx, "menu_search_vault"), SearchVault),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu_close_tab"), CloseTab),
            MenuItem::action(t(cx, "menu_reopen_closed_tab"), ReopenClosedTab),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu_sync_now"), SyncNow),
            MenuItem::action(t(cx, "menu_trash"), OpenTrash),
            MenuItem::action(t(cx, "menu_lock_vault"), LockVault),
        ]),
        Menu::new(t(cx, "menu_edit")).items([
            MenuItem::action(t(cx, "menu_find"), FindInNote),
            MenuItem::action(t(cx, "menu_replace"), ReplaceInNote),
            MenuItem::action(t(cx, "menu_paste_plain"), PasteAsPlainText),
        ]),
        Menu::new(t(cx, "menu_view")).items([
            MenuItem::action(t(cx, "menu_toggle_sidebar"), ToggleSidebar),
            MenuItem::action(t(cx, "menu_toggle_terminal"), ToggleTerminal),
            MenuItem::action(t(cx, "menu_toggle_backlinks"), ToggleBacklinks),
            MenuItem::action(t(cx, "menu_toggle_history"), ToggleHistory),
            MenuItem::action(t(cx, "menu_toggle_view_mode"), ToggleViewMode),
        ]),
        Menu::new(t(cx, "menu_window")).items([
            MenuItem::action(t(cx, "menu_minimize"), MinimizeWindow),
            MenuItem::action(t(cx, "menu_zoom"), ZoomWindow),
        ]),
    ];
    cx.set_menus(menus);
}
