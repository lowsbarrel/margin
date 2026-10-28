use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::{App, Context, ImageFormat, IntoElement, ParentElement, SharedString, Styled, div};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::{TabContent, file_title};

use crate::controls::{ZoomKey, icon_button};
use crate::external::format_size;
use crate::image_view::ImageView;

pub(crate) fn format_of(path: &RelativePath) -> Option<ImageFormat> {
    match path.extension()?.to_ascii_lowercase().as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::Webp),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "ico" => Some(ImageFormat::Ico),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        _ => None,
    }
}

pub(crate) fn zoom_percent(scale: f32, cx: &App) -> SharedString {
    #[expect(clippy::cast_possible_truncation, reason = "percent display")]
    let percent = ((scale * 100.0).round() as i32).to_string();
    t_with(cx, "viewer_zoom_percent", &[("percent", &percent)])
}

pub(crate) fn toolbar(view: &ImageView, cx: &Context<'_, ImageView>) -> impl IntoElement + use<> {
    let palette = MarginTheme::palette(cx);
    let camera = view.camera();
    let meta = view.meta().map(|(bytes, (width, height))| {
        let size = format_size(bytes, cx);
        t_with(
            cx,
            "viewer_image_meta",
            &[
                ("width", &width.round().to_string()),
                ("height", &height.round().to_string()),
                ("size", &size),
            ],
        )
    });
    let tertiary = palette.text_tertiary;
    h_flex()
        .gap_1()
        .p_1p5()
        .border_t_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface[1])
        .child(
            h_flex()
                .min_w_0()
                .gap_2()
                .px_2()
                .text_ui_xs()
                .whitespace_nowrap()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(palette.text_primary)
                        .child(file_title(view.path()).to_owned()),
                )
                .children(meta.map(|meta| div().flex_shrink_0().text_color(tertiary).child(meta))),
        )
        .child(div().flex_1())
        .child(
            icon_button(
                "zoom-out",
                IconName::ZoomOut,
                t(cx, "viewer_zoom_out"),
                tertiary,
            )
            .on_click(cx.listener(|view, _, _, cx| view.zoom(ZoomKey::Out, cx))),
        )
        .child(
            icon_button(
                "zoom-in",
                IconName::ZoomIn,
                t(cx, "viewer_zoom_in"),
                tertiary,
            )
            .on_click(cx.listener(|view, _, _, cx| view.zoom(ZoomKey::In, cx))),
        )
        .child(
            div()
                .min_w_12()
                .text_center()
                .text_ui_sm()
                .text_color(palette.text_primary)
                .child(zoom_percent(camera.current_scale(), cx)),
        )
        .child(
            icon_button("fit", IconName::Expand, t(cx, "viewer_fit"), tertiary)
                .toggled(camera.is_fit())
                .on_click(cx.listener(|view, _, _, cx| view.zoom(ZoomKey::Fit, cx))),
        )
        .child(
            icon_button(
                "actual",
                IconName::Maximize,
                t(cx, "viewer_actual_size"),
                tertiary,
            )
            .on_click(cx.listener(|view, _, _, cx| view.zoom(ZoomKey::Actual, cx))),
        )
}
