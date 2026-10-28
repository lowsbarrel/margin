use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::TitleBar;
use gpui_kit::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use margin_i18n::{Translator, t};
use margin_theme::{Appearance, MarginTheme};
use margin_workbench::{Platform, Quit, Updates, bind_keys, install_network_runtime};

use crate::http_client::ReqwestHttpClient;
use crate::icons::AppAssets;
#[cfg(target_os = "macos")]
use crate::menus;
use crate::network::TokioNetwork;
use crate::window::MarginWindow;
use crate::{container, updater};

const APP_ID: &str = "com.lowsbarrel.margin";

pub fn run() {
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            init(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            #[cfg(target_os = "macos")]
            menus::install(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            open_main_window(cx);
            cx.activate(true);
        });
}

pub fn init(cx: &mut App) {
    let data_dir =
        margin_fs::app_data_dir().expect("the per-user application data directory is writable");
    init_with_data_dir(data_dir, cx);
}

pub fn init_with_data_dir(data_dir: PathBuf, cx: &mut App) {
    gpui_kit::init(cx);
    MarginTheme::init(Appearance::System, cx);
    Translator::init(None, cx);
    let network = TokioNetwork::start();
    cx.set_http_client(Arc::new(ReqwestHttpClient::new(network.handle())));
    install_network_runtime(Arc::new(network), cx);
    Platform::install(
        Arc::new(margin_fs::reveal_in_file_manager),
        Arc::new(margin_fs::open_with_default_app),
        cx,
    );
    Updates::install_channel(updater::updater(), cx);
    bind_keys(cx);
    cx.set_global(container::app_services(data_dir));
}

fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(t(cx, "app_display_name")),
            ..TitleBar::title_bar_options()
        }),
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
