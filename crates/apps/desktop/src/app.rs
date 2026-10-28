use std::sync::Arc;

use gpui_kit::component::TitleBar;
use gpui_kit::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use margin_i18n::Translator;
use margin_theme::{Appearance, MarginTheme};
use margin_workbench::{bind_keys, install_network_runtime};

use crate::container;
use crate::network::TokioNetwork;
use crate::window::MarginWindow;

const APP_ID: &str = "com.lowsbarrel.margin";

pub(crate) fn run() {
    let network = Arc::new(TokioNetwork::start());
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            MarginTheme::init(Appearance::System, cx);
            Translator::init(None, cx);
            install_network_runtime(network, cx);
            bind_keys(cx);
            let services = container::app_services()
                .expect("the per-user application data directory is writable");
            cx.set_global(services);
            open_main_window(cx);
            cx.activate(true);
        });
}

fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let options = WindowOptions {
        titlebar: Some(TitleBar::title_bar_options()),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(640.0), px(420.0))),
        app_id: Some(APP_ID.to_owned()),
        ..WindowOptions::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| MarginWindow::new(window, cx))
    })
    .expect("the platform can open the main window");
}
