use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Context, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div};
use margin_files::SortBy;
use margin_i18n::t;
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::{NewNote, WORKSPACE_CONTEXT};

use crate::command::{Command, NewEntry};
use crate::file_tree::FileTree;

fn action_button(
    id: &'static str,
    icon: IconName,
    label: SharedString,
    command: Command,
    cx: &Context<'_, FileTree>,
    tertiary: Hsla,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .size(css_px(26.0))
        .text_color(tertiary)
        .icon(Icon::new(icon).size(css_px(14.0)))
        .accessibility_label(label.clone())
        .map(|button| match command {
            Command::Create(NewEntry::Note, _) => {
                button.tooltip_with_action(label, &NewNote, Some(WORKSPACE_CONTEXT))
            }
            _ => button.tooltip(label),
        })
        .on_click(cx.listener(move |tree, _, window: &mut Window, cx| {
            tree.run(command.clone(), window, cx);
        }))
}

pub(crate) fn render(tree: &FileTree, cx: &Context<'_, FileTree>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let folder = tree.model().target_folder();
    let (sort_icon, sort_key, next_sort) = match tree.sort_by() {
        SortBy::Name => (IconName::ArrowDownAZ, "sidebar_sort_by_date", SortBy::Date),
        SortBy::Date => (
            IconName::ArrowDownWideNarrow,
            "sidebar_sort_by_name",
            SortBy::Name,
        ),
    };
    let buttons = [
        (
            "file-tree-sort",
            sort_icon,
            sort_key,
            Command::Sort(next_sort),
        ),
        (
            "file-tree-collapse-all",
            IconName::ChevronsDownUp,
            "sidebar_collapse_all",
            Command::CollapseAll,
        ),
        (
            "file-tree-new-note",
            IconName::FilePlus,
            "sidebar_new_file",
            Command::Create(NewEntry::Note, folder.clone()),
        ),
        (
            "file-tree-new-canvas",
            IconName::PenLine,
            "sidebar_new_canvas",
            Command::Create(NewEntry::Canvas, folder.clone()),
        ),
        (
            "file-tree-new-folder",
            IconName::FolderPlus,
            "sidebar_new_folder",
            Command::Create(NewEntry::Folder, folder),
        ),
    ]
    .into_iter()
    .map(|(id, icon, key, command)| {
        action_button(id, icon, t(cx, key), command, cx, palette.text_tertiary)
    });
    div()
        .h_flex()
        .min_h(css_px(44.0))
        .flex_none()
        .pt(css_px(12.0))
        .pr(css_px(12.0))
        .pb(css_px(8.0))
        .pl(css_px(16.0))
        .gap(css_px(8.0))
        .justify_between()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_ui_xs()
                .font_semibold()
                .text_color(palette.text_tertiary)
                .child(t(cx, "sidebar_explorer").to_uppercase()),
        )
        .child(div().h_flex().gap(css_px(2.0)).children(buttons))
}
