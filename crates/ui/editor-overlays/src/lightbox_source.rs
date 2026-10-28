use std::sync::Arc;

use gpui_kit::{App, ImageSource, ImgResourceLoader, RenderImage, Window};

pub(crate) enum Loaded {
    Pending,
    Ready(Arc<RenderImage>),
    Unsized,
    Failed,
}

pub(crate) fn load(source: &ImageSource, window: &mut Window, cx: &mut App) -> Loaded {
    let result = match source {
        ImageSource::Resource(resource) => window.use_asset::<ImgResourceLoader>(resource, cx),
        ImageSource::Render(image) => Some(Ok(Arc::clone(image))),
        ImageSource::Custom(loader) => loader(window, cx),
        ImageSource::Image(_) => return Loaded::Unsized,
    };
    match result {
        None => Loaded::Pending,
        Some(Ok(image)) => Loaded::Ready(image),
        Some(Err(_)) => Loaded::Failed,
    }
}
