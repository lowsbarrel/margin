use std::path::Path;
use std::sync::Arc;

use gpui_kit::{
    App, Entity, ImageSource, IntoElement, Pixels, RenderOnce, Resource, RetainAllImageCache,
    SMOOTH_SVG_SCALE_FACTOR, SharedUri, Styled, Window, img, px,
};

use crate::embed_resolution::{EmbedSize, ImageLocation};
use crate::vault_paths::extension_of;

#[derive(Debug, IntoElement)]
pub struct EmbedImage {
    resource: Resource,
    cache: Entity<RetainAllImageCache>,
    svg: bool,
    size: Option<EmbedSize>,
    max_width: Pixels,
}

#[must_use]
pub fn embed_image(
    location: &ImageLocation,
    size: Option<EmbedSize>,
    max_width: Pixels,
    cache: &Entity<RetainAllImageCache>,
) -> EmbedImage {
    let name = match location {
        ImageLocation::Local(path) => path.to_string_lossy().into_owned(),
        ImageLocation::Remote(url) => url.clone(),
    };
    EmbedImage {
        resource: image_resource(location),
        cache: cache.clone(),
        svg: extension_of(&name) == "svg",
        size,
        max_width,
    }
}

#[must_use]
pub fn image_resource(location: &ImageLocation) -> Resource {
    match location {
        ImageLocation::Local(path) => Resource::Path(Arc::<Path>::from(path.as_path())),
        ImageLocation::Remote(url) => Resource::Uri(SharedUri::from(url.clone())),
    }
}

#[must_use]
pub fn cached_image(resource: Resource, cache: &Entity<RetainAllImageCache>) -> ImageSource {
    let cache = cache.clone();
    ImageSource::Custom(Arc::new(move |window, cx| {
        cache.update(cx, |cache, cx| cache.load(&resource, window, cx))
    }))
}

impl RenderOnce for EmbedImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let resource = self.resource.clone();
        let intrinsic = self
            .cache
            .update(cx, |cache, cx| cache.load(&resource, window, cx))
            .and_then(Result::ok)
            .map(|image| {
                let pixels = image.size(0);
                let scale = if self.svg {
                    SMOOTH_SVG_SCALE_FACTOR
                } else {
                    1.0
                };
                (
                    device_to_f32(pixels.width.0) / scale,
                    device_to_f32(pixels.height.0) / scale,
                )
            });
        let element = img(cached_image(self.resource, &self.cache)).rounded_lg();
        match fitted(intrinsic, self.size, f32::from(self.max_width)) {
            Some((width, height)) => element.w(px(width)).h(px(height)),
            None => element.max_w(self.max_width),
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "image dimensions are far below f32 precision"
)]
fn device_to_f32(value: i32) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "image dimensions are far below f32 precision"
)]
fn requested(value: u32) -> f32 {
    value as f32
}

fn fitted(
    intrinsic: Option<(f32, f32)>,
    size: Option<EmbedSize>,
    max_width: f32,
) -> Option<(f32, f32)> {
    let (width, height) = match (size, intrinsic) {
        (
            Some(EmbedSize {
                width,
                height: Some(height),
            }),
            _,
        ) => (requested(width), requested(height)),
        (Some(EmbedSize { width, .. }), Some((natural_width, natural_height)))
            if natural_width > 0.0 =>
        {
            let width = requested(width);
            (width, width * natural_height / natural_width)
        }
        (None, Some(natural)) => natural,
        _ => return None,
    };
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    if width > max_width {
        return Some((max_width, height * max_width / width));
    }
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_keep_their_aspect_ratio_inside_the_column() {
        assert_eq!(
            fitted(Some((400.0, 200.0)), None, 600.0),
            Some((400.0, 200.0))
        );
        assert_eq!(
            fitted(Some((1200.0, 600.0)), None, 600.0),
            Some((600.0, 300.0))
        );
        let width_only = Some(EmbedSize {
            width: 100,
            height: None,
        });
        assert_eq!(
            fitted(Some((400.0, 200.0)), width_only, 600.0),
            Some((100.0, 50.0))
        );
        assert_eq!(fitted(None, width_only, 600.0), None);
        let both = Some(EmbedSize {
            width: 1000,
            height: Some(100),
        });
        assert_eq!(fitted(None, both, 500.0), Some((500.0, 50.0)));
    }
}
