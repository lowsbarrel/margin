use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, ThemeStyled, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, ClickEvent, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, TestSupportExt,
    WeakEntity, Window, div,
};
use margin_i18n::t;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};
use margin_vaults::VaultProfileInfo;

use crate::actions::{Confirm, RemoveVault, RenameVault};
use crate::brand::card;
use crate::vault_picker::{PickerEvent, VaultPicker};

fn profile_menu(
    menu: PopupMenu,
    picker: &WeakEntity<VaultPicker>,
    focus_handle: &FocusHandle,
    index: usize,
    cx: &mut Context<'_, PopupMenu>,
) -> PopupMenu {
    picker
        .update(cx, |picker, cx| picker.select(index, cx))
        .unwrap_or_default();
    let reveal_target = picker.clone();
    menu.action_context(focus_handle.clone())
        .menu_with_icon(
            t(cx, "login_open"),
            Icon::new(IconName::FolderOpen),
            Box::new(Confirm),
        )
        .menu_with_icon(
            t(cx, "login_rename_vault"),
            Icon::new(IconName::Pencil),
            Box::new(RenameVault),
        )
        .item(
            PopupMenuItem::new(t(cx, "login_reveal_vault"))
                .icon(Icon::new(IconName::FolderSearch))
                .on_click(move |_, window, cx| {
                    reveal_target
                        .update(cx, |picker, cx| picker.reveal(index, window, cx))
                        .unwrap_or_default();
                }),
        )
        .separator()
        .menu_with_icon(
            t(cx, "login_remove_vault"),
            Icon::new(IconName::Trash),
            Box::new(RemoveVault),
        )
}

fn remove_button(index: usize, cx: &Context<'_, VaultPicker>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let (rest, danger) = (palette.text_tertiary, palette.text_negative);
    let label = t(cx, "login_remove_vault");
    div()
        .id(("vault-profile-remove", index))
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .size_7()
        .items_center()
        .justify_center()
        .rounded(cx.theme().radius / 2.0)
        .text_color(rest)
        .hover(|style| style.text_color(danger))
        .cursor_pointer()
        .child(Icon::new(IconName::Trash))
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            this.remove(index, window, cx);
        }))
        .test_support()
}

fn profile_row(
    profile: &VaultProfileInfo,
    index: usize,
    selected: bool,
    focus_handle: FocusHandle,
    window: &Window,
    cx: &Context<'_, VaultPicker>,
) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let picker = cx.entity().downgrade();
    let (border, hover_border) = (palette.border_secondary, palette.text_tertiary);
    let (fill, hover_fill) = (palette.surface(2), palette.surface(1));
    h_flex()
        .id(("vault-profile", index))
        .role(Role::Button)
        .aria_label(SharedString::from(profile.name.clone()))
        .w_full()
        .justify_between()
        .gap_2()
        .px_3p5()
        .py_2p5()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(border)
        .bg(fill)
        .cursor_pointer()
        .hover(|style| style.border_color(hover_border).bg(hover_fill))
        .when(selected, |row| row.focus_ring_style(window, cx))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.open(index, window, cx);
        }))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_ui_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text_primary)
                        .child(profile.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(MONO_FONT_FAMILY)
                        .text_ui_xs()
                        .text_color(palette.text_tertiary)
                        .child(profile.vault_path.clone()),
                ),
        )
        .child(remove_button(index, cx))
        .test_support()
        .context_menu(move |menu, _, cx| profile_menu(menu, &picker, &focus_handle, index, cx))
}

pub(crate) fn quiet_button(
    id: &'static str,
    icon: IconName,
    label: SharedString,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let palette = MarginTheme::palette(cx);
    let hover = palette.text_primary;
    h_flex()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .justify_center()
        .gap_1p5()
        .p_2()
        .text_ui_sm()
        .text_color(palette.text_secondary)
        .cursor_pointer()
        .hover(|style| style.text_color(hover))
        .child(Icon::new(icon))
        .child(label)
}

impl Render for VaultPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let focused = self.focus_handle().is_focused(window);
        let (tertiary, negative, divider) = (
            palette.text_tertiary,
            palette.text_negative,
            palette.border_secondary,
        );
        let rows: Vec<_> = self
            .profiles()
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                let selected = focused && self.selected() == Some(index);
                profile_row(
                    profile,
                    index,
                    selected,
                    self.focus_handle().clone(),
                    window,
                    cx,
                )
            })
            .collect();
        card(cx)
            .gap_2()
            .child(
                div()
                    .mb_1()
                    .text_ui_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tertiary)
                    .child(t(cx, "login_choose_vault")),
            )
            .children(rows)
            .when(self.is_busy(), |card| {
                card.child(
                    div()
                        .text_ui_sm()
                        .italic()
                        .text_color(tertiary)
                        .child(t(cx, "login_opening")),
                )
            })
            .when_some(self.error(), |card, error| {
                card.child(
                    div()
                        .text_ui_sm()
                        .italic()
                        .text_color(negative)
                        .child(error),
                )
            })
            .child(div().my_1().h_px().bg(divider))
            .child(
                quiet_button(
                    "onboarding-add-vault",
                    IconName::Plus,
                    t(cx, "login_add_vault"),
                    cx,
                )
                .on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::AddVault)))
                .test_support(),
            )
            .child(
                quiet_button(
                    "onboarding-replay-intro",
                    IconName::Play,
                    t(cx, "onboarding_replay"),
                    cx,
                )
                .on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::ReplayIntro)))
                .test_support(),
            )
    }
}
