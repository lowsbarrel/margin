use std::f32::consts::FRAC_PI_2;

use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Div, ElementId, FontWeight, InteractiveElement, ParentElement, Role, Stateful,
    StatefulInteractiveElement, Styled, div, radians,
};
use margin_files::TreeRow;
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::file_title;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowState {
    Rest,
    Focused,
    KeyboardFocused,
    Active,
    ActiveKeyboardFocused,
}

fn indent(depth: usize) -> gpui_kit::Rems {
    css_px(f32::from(u16::try_from(depth).unwrap_or(u16::MAX)).mul_add(14.0, 6.0))
}

pub(crate) fn menu_row(
    row: &TreeRow,
    state: RowState,
    expanded: bool,
    folder_hint: Option<&str>,
    cx: &App,
) -> Stateful<Div> {
    let palette = MarginTheme::palette(cx);
    let radius = cx.theme().radius;
    let active = matches!(state, RowState::Active | RowState::ActiveKeyboardFocused);
    let ring = matches!(
        state,
        RowState::KeyboardFocused | RowState::ActiveKeyboardFocused
    );
    let icon = match (row.is_dir, expanded) {
        (true, true) => IconName::FolderOpen,
        (true, false) => IconName::Folder,
        (false, _) => IconName::FileText,
    };
    let label = file_title(&row.path).to_owned();
    h_flex()
        .id(ElementId::Name(row.path.as_str().to_owned().into()))
        .role(Role::TreeItem)
        .aria_label(label.clone())
        .aria_level(row.depth + 1)
        .aria_selected(active)
        .when(row.is_dir, |element| element.aria_expanded(expanded))
        .relative()
        .w_full()
        .gap_2()
        .py_1p5()
        .pr_2p5()
        .pl(indent(row.depth))
        .rounded(radius)
        .cursor_pointer()
        .text_ui_sm()
        .map(|element| match state {
            RowState::Active | RowState::ActiveKeyboardFocused => element
                .bg(palette.brand(16.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.text_primary),
            RowState::KeyboardFocused | RowState::Focused => element
                .bg(palette.surface[3])
                .text_color(palette.text_primary),
            RowState::Rest => element.text_color(palette.text_secondary),
        })
        .child(
            div()
                .flex()
                .flex_none()
                .size_3p5()
                .items_center()
                .justify_center()
                .when(row.is_dir, |slot| {
                    slot.child(
                        Icon::new(IconName::ChevronRight)
                            .size(css_px(12.0))
                            .when(expanded, |chevron| chevron.rotate(radians(FRAC_PI_2))),
                    )
                }),
        )
        .child(
            Icon::new(icon)
                .size(css_px(16.0))
                .flex_none()
                .when(active, |icon| icon.text_color(palette.text_brand)),
        )
        .child(div().min_w_0().flex_1().truncate().child(label))
        .when_some(folder_hint, |element, hint| {
            element.child(
                div()
                    .flex_none()
                    .text_ui_xs()
                    .text_color(palette.text_tertiary)
                    .child(hint.to_owned()),
            )
        })
        .when(ring, |element| {
            element.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .border_2()
                    .border_color(palette.bg_brand),
            )
        })
}
