use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, HighlightStyle, InteractiveElement,
    IntoElement, ParentElement, Stateful, StatefulInteractiveElement, Styled, StyledText, div,
    rems,
};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, Palette, TextScale};
use margin_workbench::{FileKind, OpenTarget};

use crate::chrome::{icon, quiet_button, scaled};
use crate::highlight;
use crate::items::{Group, Item, Row};
use crate::results::Results;
use crate::spotlight::Spotlight;

const NAME_SUFFIXES: [&str; 2] = [".md", ".canvas"];

struct FileLine<'item> {
    path: &'item RelativePath,
    name: &'item str,
    icon: IconName,
    snippet: Option<&'item str>,
}

pub(crate) fn render(
    results: &Results,
    row: usize,
    show_replace: bool,
    cx: &Context<'_, Spotlight>,
) -> AnyElement {
    let index = match results.rows().get(row).copied() {
        Some(Row::Header { group, count }) => return header(group, count, cx),
        Some(Row::Item(index)) => index,
        None => return div().into_any_element(),
    };
    let Some(item) = results.items().get(index) else {
        return div().into_any_element();
    };
    let palette = MarginTheme::palette(cx).clone();
    let selected = results.selected() == index;
    let icon_color = if selected {
        palette.text_brand
    } else {
        palette.text_tertiary
    };
    let shell = row_shell(index, selected, &palette, cx);
    let line = match item {
        Item::Tag { tag, count } => {
            return shell
                .child(icon(IconName::Hash, 16.0).text_color(icon_color))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(MONO_FONT_FAMILY)
                        .text_ui_sm()
                        .text_color(palette.text_primary)
                        .child(format!("#{tag}")),
                )
                .child(
                    div()
                        .ml_auto()
                        .flex_shrink_0()
                        .text_ui_xs()
                        .text_color(palette.text_tertiary)
                        .child(count.to_string()),
                )
                .into_any_element();
        }
        Item::Name { path, name } => FileLine {
            path,
            name,
            icon: file_icon(path),
            snippet: None,
        },
        Item::TagFile(path) => FileLine {
            path,
            name: path.file_name(),
            icon: IconName::FileText,
            snippet: None,
        },
        Item::Content(hit) => FileLine {
            path: &hit.path,
            name: hit.path.file_name(),
            icon: IconName::FileText,
            snippet: Some(&hit.snippet),
        },
    };
    let query = results.query().search_text().unwrap_or_default();
    let replace = (show_replace && line.snippet.is_some()).then(|| line.path.clone());
    shell
        .child(
            icon(line.icon, 16.0)
                .flex_shrink_0()
                .self_start()
                .mt_0p5()
                .text_color(icon_color),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(title_line(&line, &palette))
                .children(
                    line.snippet
                        .map(highlight::one_line)
                        .filter(|text| !text.is_empty())
                        .map(|text| snippet_line(text, query, &palette)),
                ),
        )
        .when_some(replace, |row, path| {
            row.child(replace_button(index, path, cx))
        })
        .into_any_element()
}

fn open_target(event: &ClickEvent, cx: &App) -> OpenTarget {
    if event.modifiers().secondary(cx) {
        OpenTarget::NewTab
    } else {
        OpenTarget::ActivePane
    }
}

fn header(group: Group, count: usize, cx: &Context<'_, Spotlight>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let count = count.to_string();
    let title = match group {
        Group::Names => t_with(cx, "spotlight_group_names", &[("count", &count)]),
        Group::Contents => t_with(cx, "spotlight_group_contents", &[("count", &count)]),
        Group::Tags => t(cx, "spotlight_group_tags"),
    };
    div()
        .px_2p5()
        .pt_2()
        .pb_1()
        .text_ui_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.text_tertiary)
        .when(group == Group::Contents, |header| {
            header
                .mt_1()
                .pt_3()
                .border_t_1()
                .border_color(palette.border_secondary)
        })
        .child(title.to_uppercase())
        .into_any_element()
}

fn row_shell(
    index: usize,
    selected: bool,
    palette: &Palette,
    cx: &Context<'_, Spotlight>,
) -> Stateful<Div> {
    h_flex()
        .id(("spotlight-row", index))
        .relative()
        .w_full()
        .gap_2p5()
        .px_2p5()
        .py_1p5()
        .rounded(scaled(16.0))
        .cursor_pointer()
        .when(selected, |row| {
            row.bg(palette.brand(12.0)).child(
                div()
                    .absolute()
                    .top_2()
                    .bottom_2()
                    .left_0()
                    .w(rems(0.125))
                    .rounded_full()
                    .bg(palette.bg_brand),
            )
        })
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.select(index, cx);
            }
        }))
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            let target = open_target(event, cx);
            this.activate(index, target, window, cx);
        }))
}

fn display_name(name: &str) -> &str {
    NAME_SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(name)
}

fn title_line(line: &FileLine<'_>, palette: &Palette) -> impl IntoElement {
    let folder = line.path.parent().filter(|parent| !parent.is_root());
    h_flex()
        .min_w_0()
        .items_baseline()
        .gap_2()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_ui_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.text_primary)
                .child(display_name(line.name).to_owned()),
        )
        .when_some(folder, |title, folder| {
            title.child(
                div()
                    .ml_auto()
                    .min_w_0()
                    .truncate()
                    .text_ui_xs()
                    .text_color(palette.text_tertiary)
                    .child(folder.as_str().to_owned()),
            )
        })
}

fn snippet_line(text: String, query: &str, palette: &Palette) -> impl IntoElement {
    let style = HighlightStyle {
        background_color: Some(palette.brand(32.0)),
        color: Some(palette.text_primary),
        font_weight: Some(FontWeight::MEDIUM),
        ..HighlightStyle::default()
    };
    let ranges = highlight::match_ranges(&text, query);
    div()
        .min_w_0()
        .truncate()
        .text_ui_xs()
        .text_color(palette.text_secondary)
        .child(
            StyledText::new(text).with_highlights(ranges.into_iter().map(|range| (range, style))),
        )
}

fn replace_button(
    index: usize,
    path: RelativePath,
    cx: &Context<'_, Spotlight>,
) -> impl IntoElement {
    let label = t(cx, "spotlight_replace_all_in_file");
    Button::new(("spotlight-replace-file", index))
        .custom(quiet_button(false, cx))
        .xsmall()
        .self_center()
        .icon(icon(IconName::Replace, 12.0))
        .accessibility_label(label.clone())
        .tooltip(label)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.replace(vec![path.clone()], window, cx);
        }))
}

fn file_icon(path: &RelativePath) -> IconName {
    match FileKind::of(path) {
        FileKind::Note => IconName::FileText,
        FileKind::Image => IconName::Image,
        FileKind::Pdf => IconName::FileType,
        FileKind::Canvas | FileKind::Other => IconName::Hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_and_canvas_names_drop_their_extension() {
        assert_eq!(display_name("Roadmap.md"), "Roadmap");
        assert_eq!(display_name("Board.canvas"), "Board");
        assert_eq!(display_name("photo.png"), "photo.png");
    }
}
