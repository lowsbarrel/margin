use std::sync::Arc;

use gpui_kit::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, div, img, px};
use margin_i18n::t;
use margin_theme::MarginTheme;

use crate::diagram_cache::DiagramState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramKind {
    Math,
    Mermaid,
}

#[must_use]
pub fn diagram_element(
    state: &DiagramState,
    kind: DiagramKind,
    source: &str,
    cx: &App,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    match state {
        DiagramState::Ready(diagram) => {
            let image = img(Arc::clone(&diagram.image)).w(px(diagram.width));
            match kind {
                DiagramKind::Math => image.h(px(diagram.height)).flex_shrink_0(),
                DiagramKind::Mermaid => image
                    .max_w_full()
                    .aspect_ratio(diagram.width / diagram.height),
            }
            .into_any_element()
        }
        DiagramState::Empty => {
            let key = match kind {
                DiagramKind::Math => "editor_empty_math",
                DiagramKind::Mermaid => "editor_empty_diagram",
            };
            div()
                .text_color(palette.text_tertiary)
                .child(t(cx, key))
                .into_any_element()
        }
        DiagramState::Pending => match kind {
            DiagramKind::Math => div()
                .text_color(palette.text_tertiary)
                .child(SharedString::from(source.to_owned()))
                .into_any_element(),
            DiagramKind::Mermaid => div().into_any_element(),
        },
        DiagramState::Failed(message) => {
            let text = match kind {
                DiagramKind::Math => SharedString::from(source.to_owned()),
                DiagramKind::Mermaid => message.clone(),
            };
            div()
                .text_color(palette.text_negative)
                .child(text)
                .into_any_element()
        }
    }
}
