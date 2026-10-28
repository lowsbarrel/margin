use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, Entity};
use margin_i18n::t;
use margin_markdown::{PropertyKind, PropertyValue};

use crate::effect::{Effect, FocusTarget};
use crate::properties_panel::PropertiesPanel;
use crate::property_edits::{SortOrder, convert_value};
use crate::property_row::kind_icon;

const TYPES: [(PropertyKind, &str); 6] = [
    (PropertyKind::Text, "editor_property_type_text"),
    (PropertyKind::List, "editor_property_type_list"),
    (PropertyKind::Number, "editor_property_type_number"),
    (PropertyKind::Checkbox, "editor_property_type_checkbox"),
    (PropertyKind::Date, "editor_property_type_date"),
    (PropertyKind::Datetime, "editor_property_type_datetime"),
];

pub(crate) fn type_label(kind: PropertyKind) -> &'static str {
    TYPES
        .iter()
        .find(|(entry, _)| *entry == kind)
        .map_or("editor_property_type_text", |(_, label)| label)
}

pub(crate) fn type_menu(
    mut menu: PopupMenu,
    panel: &Entity<PropertiesPanel>,
    key: &str,
    current: PropertyKind,
    value: &PropertyValue,
    cx: &App,
) -> PopupMenu {
    let fixed = matches!(
        current,
        PropertyKind::Raw | PropertyKind::Tags | PropertyKind::Aliases
    );
    for (kind, label) in TYPES {
        let panel = panel.clone();
        let owner = key.to_owned();
        menu = menu.item(
            PopupMenuItem::new(t(cx, label))
                .icon(Icon::new(kind_icon(kind)))
                .checked(kind == current)
                .disabled(fixed || convert_value(value, kind).is_none())
                .on_click(move |_, window, cx| {
                    panel.update(cx, |panel, cx| {
                        panel.apply_effect(Some(&owner), Effect::ChangeKind(kind), window, cx);
                    });
                }),
        );
    }
    let panel = panel.clone();
    let owner = key.to_owned();
    menu.separator().item(
        PopupMenuItem::new(t(cx, "editor_property_remove"))
            .icon(Icon::new(IconName::Trash))
            .on_click(move |_, window, cx| {
                panel.update(cx, |panel, cx| {
                    panel.apply_effect(Some(&owner), Effect::Remove, window, cx);
                });
            }),
    )
}

pub(crate) fn heading_menu(
    menu: PopupMenu,
    panel: &Entity<PropertiesPanel>,
    cx: &App,
) -> PopupMenu {
    let adding = panel.clone();
    let ascending = panel.clone();
    let descending = panel.clone();
    let clearing = panel.clone();
    menu.item(
        PopupMenuItem::new(t(cx, "editor_property_add"))
            .icon(Icon::new(IconName::Plus))
            .on_click(move |_, window, cx| {
                adding.update(cx, |panel, cx| {
                    panel.apply_effect(None, Effect::Focus(FocusTarget::New), window, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(t(cx, "editor_properties_sort_ascending"))
            .icon(Icon::new(IconName::ArrowDownAZ))
            .on_click(move |_, window, cx| {
                ascending.update(cx, |panel, cx| panel.sort(SortOrder::Ascending, window, cx));
            }),
    )
    .item(
        PopupMenuItem::new(t(cx, "editor_properties_sort_descending"))
            .icon(Icon::new(IconName::ArrowUpZA))
            .on_click(move |_, window, cx| {
                descending.update(cx, |panel, cx| {
                    panel.sort(SortOrder::Descending, window, cx);
                });
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new(t(cx, "editor_properties_clear"))
            .icon(Icon::new(IconName::Trash))
            .on_click(move |_, window, cx| {
                clearing.update(cx, |panel, cx| panel.clear(window, cx));
            }),
    )
}
