use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, Role, StatefulInteractiveElement, Styled, Subscription,
    Window, div,
};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::{CurrentVault, WorkbenchEvent};

use crate::breadcrumb_menu::BreadcrumbMenu;
use crate::crumb::{Crumb, crumbs};

#[derive(Debug)]
pub struct Breadcrumbs {
    path: RelativePath,
    menu: Entity<BreadcrumbMenu>,
    open_crumb: Option<usize>,
    _menu_events: Subscription,
}

impl EventEmitter<WorkbenchEvent> for Breadcrumbs {}

impl Breadcrumbs {
    pub fn new(path: RelativePath, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let menu = cx.new(BreadcrumbMenu::new);
        let menu_events = cx.subscribe(&menu, |breadcrumbs, _, event: &WorkbenchEvent, cx| {
            breadcrumbs.close_menu(cx);
            cx.emit(event.clone());
        });
        Self {
            path,
            menu,
            open_crumb: None,
            _menu_events: menu_events,
        }
    }

    pub fn set_path(&mut self, path: RelativePath, cx: &mut Context<'_, Self>) {
        if self.path != path {
            self.path = path;
            self.close_menu(cx);
        }
    }

    fn close_menu(&mut self, cx: &mut Context<'_, Self>) {
        self.open_crumb = None;
        self.menu.update(cx, |menu, _| menu.close());
        cx.notify();
    }

    fn set_open(&mut self, index: usize, open: bool, cx: &mut Context<'_, Self>) {
        if !open {
            if self.open_crumb == Some(index) {
                self.close_menu(cx);
            }
            return;
        }
        let crumbs = crumbs(&self.path, &CurrentVault::expect(cx).name);
        let Some(folder) = crumbs.get(index).map(Crumb::listed_folder) else {
            return;
        };
        let title = crumbs
            .iter()
            .find(|crumb| crumb.path() == &folder)
            .map(|crumb| crumb.label().to_owned())
            .unwrap_or_default();
        let source = self.path.clone();
        self.open_crumb = Some(index);
        self.menu
            .update(cx, |menu, cx| menu.show(folder, title, source, cx));
        cx.notify();
    }
}

impl Render for Breadcrumbs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let crumbs = crumbs(&self.path, &CurrentVault::expect(cx).name);
        if crumbs.is_empty() {
            return div().into_any_element();
        }
        let palette = MarginTheme::palette(cx);
        let last = crumbs.len() - 1;
        let menu_focus = self.menu.focus_handle(cx);
        let mut row = h_flex()
            .id("breadcrumbs")
            .role(Role::Navigation)
            .aria_label(t(cx, "breadcrumb_nav_label"))
            .w_full()
            .flex_none()
            .min_h(css_px(30.0))
            .px_4()
            .py_1p5()
            .overflow_x_scroll()
            .border_b_1()
            .border_color(palette.border_divider)
            .bg(palette.surface[0])
            .text_ui_xs()
            .text_color(palette.text_tertiary);
        for (index, crumb) in crumbs.iter().enumerate() {
            if index > 0 {
                row = row.child(
                    Icon::new(IconName::ChevronRight)
                        .size(css_px(12.0))
                        .flex_none()
                        .text_color(palette.text_tertiary),
                );
            }
            let browse = t_with(cx, "breadcrumb_browse", &[("name", crumb.label())]);
            let color = if index == last {
                palette.text_secondary
            } else {
                palette.text_tertiary
            };
            let trigger = Button::new(("breadcrumb", index))
                .ghost()
                .xsmall()
                .h_auto()
                .px_1()
                .py_0p5()
                .text_ui_xs()
                .font_weight(FontWeight::MEDIUM)
                .label(crumb.label().to_owned())
                .accessibility_label(browse)
                .text_color(color);
            let breadcrumbs = cx.entity();
            let menu = self.menu.clone();
            row = row.child(
                Popover::new(("breadcrumb-menu", index))
                    .trigger(trigger)
                    .open(self.open_crumb == Some(index))
                    .on_open_change(move |open, _window, cx| {
                        breadcrumbs.update(cx, |breadcrumbs, cx| {
                            breadcrumbs.set_open(index, *open, cx);
                        });
                    })
                    .track_focus(&menu_focus)
                    .p_1()
                    .content(move |_, _, _| menu.clone()),
            );
        }
        row.into_any_element()
    }
}
