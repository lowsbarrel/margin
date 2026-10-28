use std::f32::consts::FRAC_PI_2;
use std::ops::Range;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, ElementId, Focusable, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Role, SharedString, StatefulInteractiveElement,
    Styled, Window, div, radians,
};
use margin_files::TreeRow;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::file_title;

use crate::command::Command;
use crate::file_tree::FileTree;
use crate::inline_edit::InlineEdit;
use crate::model::{Gesture, Item};
use crate::selection::SelectedEntry;
use crate::{menu, row_drag};

pub(crate) fn row_id(path: &RelativePath) -> ElementId {
    ElementId::Name(SharedString::from(format!("file-tree-row:{path}")))
}

fn indent(depth: usize, inset: f32) -> gpui_kit::Rems {
    css_px(f32::from(u16::try_from(depth).unwrap_or(u16::MAX)).mul_add(16.0, inset))
}

fn icon(name: IconName) -> Icon {
    Icon::new(name).size(css_px(16.0))
}

fn gesture(event: &ClickEvent, cx: &App) -> Gesture {
    let modifiers = event.modifiers();
    if modifiers.secondary(cx) {
        Gesture::Toggle
    } else if modifiers.shift {
        Gesture::Extend
    } else {
        Gesture::Plain {
            click_count: event.click_count(),
        }
    }
}

pub(crate) fn render_items(
    tree: &FileTree,
    range: Range<usize>,
    window: &Window,
    cx: &Context<'_, FileTree>,
) -> Vec<AnyElement> {
    let focused = tree.focus_handle(cx).contains_focused(window, cx);
    range
        .filter_map(|index| match tree.model().item(index)? {
            Item::NewFolder { depth } => tree
                .edit()
                .map(|edit| editor_row(edit, IconName::Folder, depth, cx)),
            Item::Row(row) => match tree.edit() {
                Some(edit) if edit.renames(&row.path) => {
                    let icon = icon_for(row, tree.model().is_expanded(&row.path));
                    Some(editor_row(edit, icon, row.depth, cx))
                }
                _ => Some(entry_row(tree, row, focused, cx)),
            },
        })
        .collect()
}

fn icon_for(row: &TreeRow, expanded: bool) -> IconName {
    match (row.is_dir, expanded) {
        (true, true) => IconName::FolderOpen,
        (true, false) => IconName::Folder,
        (false, _) => IconName::FileText,
    }
}

fn editor_row(
    edit: &InlineEdit,
    icon: IconName,
    depth: usize,
    cx: &Context<'_, FileTree>,
) -> AnyElement {
    div()
        .h_flex()
        .h_8()
        .w_full()
        .gap_2()
        .pl(indent(depth, 8.0))
        .pr_3()
        .text_ui_sm()
        .text_color(MarginTheme::palette(cx).text_secondary)
        .child(self::icon(icon))
        .child(Input::new(edit.input()).small().flex_1())
        .into_any_element()
}

fn entry_row(
    tree: &FileTree,
    row: &TreeRow,
    tree_focused: bool,
    cx: &Context<'_, FileTree>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let radius = cx.theme().radius;
    let model = tree.model();
    let path = row.path.clone();
    let entry = SelectedEntry::new(path.clone(), row.is_dir);
    let expanded = model.is_expanded(&path);
    let selected = model.selection().contains(&path);
    let active = !row.is_dir && model.active() == Some(&path);
    let ring = tree_focused && tree.keyboard() && model.focused() == Some(&path);
    let label = if row.is_dir {
        row.name.as_str()
    } else {
        file_title(&path)
    };
    let entity = cx.entity();
    let fill = palette.brand(if tree_focused { 32.0 } else { 16.0 });
    let hover_fill = palette.surface(3);
    let ring_color = palette.bg_brand;
    let text = palette.text_primary;
    let icon =
        icon(icon_for(row, expanded)).when(active, |icon| icon.text_color(palette.text_brand));
    div()
        .id(row_id(&path))
        .role(Role::TreeItem)
        .aria_label(SharedString::from(label.to_owned()))
        .aria_level(row.depth + 1)
        .aria_selected(selected)
        .when(row.is_dir, |div| div.aria_expanded(expanded))
        .relative()
        .h_flex()
        .h_8()
        .w_full()
        .gap_2()
        .pl(indent(row.depth, if row.is_dir { 3.0 } else { 7.0 }))
        .pr_3()
        .rounded(radius)
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .overflow_hidden()
        .text_ui_sm()
        .text_color(palette.text_secondary)
        .cursor_pointer()
        .when(selected || active, |div| div.bg(fill).text_color(text))
        .when(active, |div| div.font_weight(FontWeight::MEDIUM))
        .when(!selected && !active, |div| {
            div.hover(move |style| style.bg(hover_fill).text_color(text))
        })
        .when(row.is_dir, |div| {
            div.child(
                Icon::new(IconName::ChevronRight)
                    .size(css_px(14.0))
                    .text_color(palette.text_tertiary)
                    .when(expanded, |chevron| chevron.rotate(radians(FRAC_PI_2))),
            )
        })
        .child(icon)
        .child(div().flex_1().min_w_0().truncate().child(label.to_owned()))
        .when(ring, |div| {
            div.child(
                gpui_kit::div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .border_2()
                    .border_color(ring_color),
            )
        })
        .on_click(cx.listener({
            let path = path.clone();
            move |tree, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                let gesture = gesture(event, cx);
                tree.run(Command::Click(path.clone(), gesture), window, cx);
            }
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener({
                let path = path.clone();
                move |tree, _, window, cx| tree.run(Command::ContextMenu(path.clone()), window, cx)
            }),
        )
        .map(|element| row_drag::with_drag_and_drop(element, tree, row, &entry, cx))
        .context_menu(move |popup, _, cx| menu::entry(popup, &entity, &entry, cx))
        .into_any_element()
}
