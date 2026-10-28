use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::{App, Div, ElementId, ParentElement, SharedString, Styled, div, px};
use margin_i18n::t;
use margin_live_preview::{CodeLine, CodeLinePosition, Container, ContainerKind};
use margin_markdown::{CalloutKind, language_label};
use margin_theme::{CalloutTone, Palette, UI_FONT_FAMILY};

use crate::metrics::{self, TypeScale};
use crate::row_context::{RowAction, RowContext};

const CALLOUT_RADIUS: f32 = 4.0;

pub(crate) fn code_frame(
    body: Div,
    code: &CodeLine,
    item: usize,
    context: &RowContext,
    cx: &App,
) -> Div {
    let palette = context.palette();
    let frame = div()
        .w_full()
        .bg(palette.surface(2))
        .px(metrics::em(16.0 / 15.0));
    let empty_line = || div().w_full().h(TypeScale::Code.line_rems());
    match code.position {
        CodeLinePosition::OpenFence => {
            let frame = frame.rounded_t_lg().pt(metrics::em(8.0 / 15.0));
            if !code.fence_hidden {
                return frame.child(body);
            }
            let label = code
                .language
                .as_deref()
                .map(language_label)
                .unwrap_or_default();
            let handler = context.handler();
            let inner = code.content;
            frame.child(
                empty_line().child(
                    h_flex()
                        .size_full()
                        .justify_end()
                        .items_center()
                        .gap_1p5()
                        .font_family(UI_FONT_FAMILY)
                        .text_size(TypeScale::CodeLabel.size())
                        .text_color(palette.text_tertiary)
                        .child(SharedString::from(label))
                        .child(
                            Button::new(ElementId::NamedInteger("code-copy".into(), item as u64))
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::Copy))
                                .tooltip(t(cx, "editor_copy_code"))
                                .on_click(move |_, window, cx| {
                                    handler(RowAction::CopyCode { content: inner }, window, cx);
                                }),
                        ),
                ),
            )
        }
        CodeLinePosition::CloseFence => {
            let frame = frame.rounded_b_lg();
            if code.fence_hidden {
                frame.child(empty_line())
            } else {
                frame.child(body)
            }
        }
        CodeLinePosition::Body => frame.child(body),
    }
}

pub(crate) fn container_frame(body: Div, container: Container, palette: &Palette) -> Div {
    match container.kind {
        ContainerKind::Quote => div()
            .w_full()
            .border_l_2()
            .border_color(palette.text_brand)
            .pl(metrics::em(1.0))
            .child(body),
        ContainerKind::Callout(kind) => {
            let color = palette.callout(callout_tone(kind));
            let radius = px(CALLOUT_RADIUS);
            let mut frame = div()
                .w_full()
                .border_l_2()
                .border_color(color)
                .bg(color.opacity(0.08))
                .pl(metrics::em(1.6))
                .pr(metrics::em(0.8));
            if container.first {
                frame = frame.rounded_tr(radius).pt(metrics::em(0.8));
                if !container.last {
                    frame = frame.pb(metrics::em(1.0));
                }
            }
            if container.last {
                frame = frame.rounded_br(radius).pb(metrics::em(1.8));
            }
            frame.child(body)
        }
    }
}

pub(crate) const fn callout_tone(kind: CalloutKind) -> CalloutTone {
    match kind {
        CalloutKind::Note => CalloutTone::Note,
        CalloutKind::Abstract => CalloutTone::Abstract,
        CalloutKind::Info => CalloutTone::Info,
        CalloutKind::Todo => CalloutTone::Todo,
        CalloutKind::Tip => CalloutTone::Tip,
        CalloutKind::Success => CalloutTone::Success,
        CalloutKind::Question => CalloutTone::Question,
        CalloutKind::Warning => CalloutTone::Warning,
        CalloutKind::Failure => CalloutTone::Failure,
        CalloutKind::Danger => CalloutTone::Danger,
        CalloutKind::Bug => CalloutTone::Bug,
        CalloutKind::Example => CalloutTone::Example,
        CalloutKind::Quote => CalloutTone::Quote,
    }
}

pub(crate) const fn callout_icon(kind: CalloutKind) -> IconName {
    match kind {
        CalloutKind::Note => IconName::Pencil,
        CalloutKind::Abstract => IconName::ClipboardList,
        CalloutKind::Info => IconName::Info,
        CalloutKind::Todo => IconName::CircleCheck,
        CalloutKind::Tip => IconName::Flame,
        CalloutKind::Success => IconName::Check,
        CalloutKind::Question => IconName::CircleQuestionMark,
        CalloutKind::Warning => IconName::TriangleAlert,
        CalloutKind::Failure => IconName::X,
        CalloutKind::Danger => IconName::Zap,
        CalloutKind::Bug => IconName::Bug,
        CalloutKind::Example => IconName::List,
        CalloutKind::Quote => IconName::Quote,
    }
}
