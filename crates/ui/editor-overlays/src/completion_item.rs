use std::ops::Range;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    ClickEvent, Context, Div, FontWeight, HighlightStyle, InteractiveElement, ParentElement, Role,
    Stateful, StatefulInteractiveElement, Styled, StyledText, div,
};
use margin_editor_commands::fuzzy_match;
use margin_theme::{MarginTheme, Palette, css_px};

use crate::completion::Completion;
use crate::completion_menu::CompletionMenu;

const ROW_HEIGHT: f32 = 44.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompletionItem {
    icon: IconName,
    label: String,
    description: String,
    highlights: Vec<Range<usize>>,
    completion: Completion,
}

fn matched(query: &str, label: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    fuzzy_match(&query.to_lowercase(), label).map_or_else(Vec::new, |found| found.ranges)
}

impl CompletionItem {
    pub(crate) fn new(icon: IconName, label: String, query: &str, completion: Completion) -> Self {
        let highlights = matched(query, &label);
        Self {
            icon,
            label,
            description: String::new(),
            highlights,
            completion,
        }
    }

    pub(crate) fn described(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub(crate) const fn completion(&self) -> &Completion {
        &self.completion
    }

    #[cfg(test)]
    pub(crate) const fn icon(&self) -> IconName {
        self.icon
    }

    #[cfg(test)]
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    #[cfg(test)]
    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    #[cfg(test)]
    pub(crate) fn highlights(&self) -> &[Range<usize>] {
        &self.highlights
    }

    fn styled_label(&self, palette: &Palette) -> StyledText {
        let style = HighlightStyle {
            color: Some(palette.text_brand),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..HighlightStyle::default()
        };
        StyledText::new(self.label.clone())
            .with_highlights(self.highlights.iter().map(|range| (range.clone(), style)))
    }

    pub(crate) fn row(
        &self,
        index: usize,
        selected: bool,
        cx: &Context<'_, CompletionMenu>,
    ) -> Stateful<Div> {
        let palette = MarginTheme::palette(cx);
        let badge = div()
            .flex()
            .flex_none()
            .size(css_px(30.0))
            .items_center()
            .justify_center()
            .rounded(css_px(8.0))
            .border_1()
            .map(|badge| {
                if selected {
                    badge
                        .bg(palette.brand(12.0))
                        .border_color(palette.brand(24.0))
                        .text_color(palette.text_brand)
                } else {
                    badge
                        .bg(palette.surface(1))
                        .border_color(palette.border_secondary)
                        .text_color(palette.text_secondary)
                }
            })
            .child(Icon::new(self.icon));
        let text = v_flex()
            .min_w_0()
            .flex_1()
            .child(
                div()
                    .truncate()
                    .text_size(css_px(14.0))
                    .line_height(css_px(18.2))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.text_primary)
                    .child(self.styled_label(palette)),
            )
            .when(!self.description.is_empty(), |text| {
                text.child(
                    div()
                        .truncate()
                        .text_size(css_px(12.0))
                        .line_height(css_px(15.6))
                        .text_color(palette.text_tertiary)
                        .child(self.description.clone()),
                )
            });
        h_flex()
            .id(index)
            .role(Role::ListBoxOption)
            .aria_label(self.label.clone())
            .aria_selected(selected)
            .h(css_px(ROW_HEIGHT))
            .w_full()
            .gap(css_px(10.0))
            .px(css_px(8.0))
            .rounded(css_px(8.0))
            .cursor_pointer()
            .map(|row| {
                if selected {
                    row.bg(palette.surface(1))
                } else {
                    row.hover(|style| style.bg(palette.surface(1)))
                }
            })
            .child(badge)
            .child(text)
            .on_click(cx.listener(move |menu, _: &ClickEvent, _, cx| menu.click(index, cx)))
    }
}
