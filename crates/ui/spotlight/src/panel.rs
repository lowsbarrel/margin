use std::time::Duration;

use gpui_kit::component::input::{Enter, Escape, MoveDown, MoveUp};
use gpui_kit::component::v_flex;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, Context, InteractiveElement, IntoElement,
    ListSizingBehavior, MouseButton, ParentElement, Render, Styled, Window, div, ease_out_quint,
    list, px,
};
use margin_theme::{MOTION_SHORT, MarginTheme};
use margin_workbench::{OpenSpotlight, OpenTarget, ReplaceInNote, SearchVault};

use crate::ask_view;
use crate::chrome::{self, LeadingIcon, ReplaceAll, ReplaceToggle, scaled};
use crate::query::Query;
use crate::spotlight::Spotlight;

const OFFSET_SHARE: f32 = 0.18;
const MAX_OFFSET: f32 = 120.0;
const SLIDE: f32 = 6.0;

impl Render for Spotlight {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.is_open() {
            return div().into_any_element();
        }
        let palette = MarginTheme::palette(cx).clone();
        let query = self.results().query().clone();
        let icon = if self.results().is_searching() || self.ask().is_running() {
            LeadingIcon::Busy
        } else if query.is_ask() {
            LeadingIcon::Ask
        } else {
            LeadingIcon::Search
        };
        let toggle = if query.is_tags() || query.is_ask() {
            ReplaceToggle::Hidden
        } else if self.shows_replace() {
            ReplaceToggle::On
        } else {
            ReplaceToggle::Off
        };
        let query_input = self.query_input().clone();
        let header = chrome::search_bar(&query_input, icon, toggle, cx);
        let replace = (toggle == ReplaceToggle::On).then(|| {
            let input = self.replace_input().clone();
            let count = self.results().hits().len();
            let state = if self.is_replacing() {
                ReplaceAll::Running
            } else if self.results().can_replace() {
                ReplaceAll::Ready
            } else {
                ReplaceAll::Unavailable
            };
            chrome::replace_bar(&input, count, state, cx)
        });
        let crumb = match (query.is_tags(), self.results().selected_tag()) {
            (true, Some(tag)) => {
                let tag = tag.to_owned();
                let count = self.results().items().len();
                Some(chrome::tag_crumb(&tag, count, cx))
            }
            _ => None,
        };
        let sections = [Some(header), replace, crumb, body(self, &query, cx)];
        let top = (window.viewport_size().height * OFFSET_SHARE).min(px(MAX_OFFSET));
        div()
            .id("spotlight")
            .size_full()
            .flex()
            .justify_center()
            .items_start()
            .px(scaled(16.0))
            .pt(top)
            .bg(palette.bg_overlay)
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss(cx)),
            )
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                cx.stop_propagation();
                this.move_selection(-1, cx);
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                cx.stop_propagation();
                this.move_selection(1, cx);
            }))
            .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                cx.stop_propagation();
                let target = if action.secondary {
                    OpenTarget::NewTab
                } else {
                    OpenTarget::ActivePane
                };
                this.confirm(target, window, cx);
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                cx.stop_propagation();
                this.escape(window, cx);
            }))
            .capture_action(|_: &OpenSpotlight, _, cx| cx.stop_propagation())
            .capture_action(|_: &SearchVault, _, cx| cx.stop_propagation())
            .on_action(cx.listener(|this, _: &ReplaceInNote, _, cx| this.toggle_replace(cx)))
            .child(frame(sections.into_iter().flatten(), cx))
            .into_any_element()
    }
}

fn frame(sections: impl Iterator<Item = AnyElement>, cx: &Context<'_, Spotlight>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let motion = if cx.reduce_motion() {
        Duration::ZERO
    } else {
        MOTION_SHORT
    };
    v_flex()
        .id("spotlight-panel")
        .w(scaled(640.0))
        .max_w_full()
        .max_h(scaled(520.0))
        .overflow_hidden()
        .rounded(scaled(20.0))
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(0))
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(sections)
        .with_animation(
            "spotlight-enter",
            Animation::new(motion).with_easing(ease_out_quint()),
            |panel, delta| panel.opacity(delta).mt(scaled(-SLIDE * (1.0 - delta))),
        )
        .into_any_element()
}

fn body(spotlight: &Spotlight, query: &Query, cx: &Context<'_, Spotlight>) -> Option<AnyElement> {
    let results = spotlight.results();
    if query.is_ask() {
        return Some(ask_view::render(spotlight.ask(), cx));
    }
    if !results.rows().is_empty() {
        return Some(
            list(
                results.list(),
                cx.processor(|this: &mut Spotlight, row, window, cx| {
                    this.render_row(row, window, cx)
                }),
            )
            .with_sizing_behavior(ListSizingBehavior::Infer)
            .flex_shrink_1()
            .min_h_0()
            .p_1p5()
            .into_any_element(),
        );
    }
    let key = match query {
        Query::Tags { .. } if results.tags_loading() => "tags_loading",
        Query::Tags { .. } if results.selected_tag().is_some() => "tags_no_files",
        Query::Tags { .. } => "tags_empty",
        Query::Search(_) if results.is_searching() => return None,
        Query::Search(_) => "spotlight_no_results",
        Query::Empty | Query::Ask(_) => "spotlight_hint",
    };
    Some(chrome::empty_state(key, cx))
}
