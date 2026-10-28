use std::mem;
use std::sync::Arc;

use gpui_kit::component::scroll::{ScrollableElement, ScrollbarAxis};
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, AppContext, Context, CursorStyle, DispatchPhase, Entity, EventEmitter, FocusHandle,
    Focusable, Image, ImageFormat, ImageSource, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, PinchEvent, Pixels, Point, Render,
    RenderImage, SMOOTH_SVG_SCALE_FACTOR, ScrollEvent, Styled, Subscription, SvgRenderer, Window,
    canvas, div, img, px,
};
use margin_breadcrumbs::Breadcrumbs;
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, css_px};
use margin_workbench::{CurrentVault, TabContent, WorkbenchEvent, run_blocking};

use crate::controls::{ZoomKey, scroll_key, zoom_key};
use crate::image_camera::ImageCamera;
use crate::image_stage::checkerboard;
use crate::image_toolbar::{format_of, toolbar};
use crate::zoom::wheel_factor;

#[derive(Debug)]
struct Svg {
    source: Arc<[u8]>,
    density: f32,
    pending: bool,
}

#[derive(Debug)]
struct Picture {
    image: Arc<RenderImage>,
    bytes: u64,
    alpha: bool,
    svg: Option<Svg>,
}

#[derive(Debug)]
enum Loaded {
    Loading,
    Ready(Picture),
    Failed,
}

#[derive(Debug)]
pub struct ImageView {
    path: RelativePath,
    crumbs: Entity<Breadcrumbs>,
    _crumbs_subscription: Subscription,
    focus_handle: FocusHandle,
    loaded: Loaded,
    camera: ImageCamera,
    revision: u64,
}

fn decode(
    format: ImageFormat,
    bytes: Vec<u8>,
    renderer: &SvgRenderer,
) -> Option<(Picture, (f32, f32))> {
    let size = bytes.len() as u64;
    let (image, density, svg) = if format == ImageFormat::Svg {
        let image = renderer.render_single_frame(&bytes, 1.0).ok()?;
        (image, SMOOTH_SVG_SCALE_FACTOR, Some(Arc::from(bytes)))
    } else {
        let image = Image::from_bytes(format, bytes)
            .to_image_data(renderer.clone())
            .ok()?;
        (image, 1.0, None)
    };
    let pixels = image.size(0);
    #[expect(clippy::cast_precision_loss, reason = "image sizes fit in f32")]
    let natural = (
        pixels.width.0 as f32 / density,
        pixels.height.0 as f32 / density,
    );
    let alpha = image.as_bytes(0).is_some_and(|data| {
        data.as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] < u8::MAX)
    });
    let picture = Picture {
        image,
        bytes: size,
        alpha,
        svg: svg.map(|source| Svg {
            source,
            density: 1.0,
            pending: false,
        }),
    };
    Some((picture, natural))
}

impl ImageView {
    pub fn new(path: RelativePath, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let crumbs = cx.new(|cx| Breadcrumbs::new(path.clone(), window, cx));
        let crumbs_subscription = cx.subscribe(&crumbs, |_, _, event: &WorkbenchEvent, cx| {
            cx.emit(event.clone());
        });
        let mut view = Self {
            path,
            crumbs,
            _crumbs_subscription: crumbs_subscription,
            focus_handle: cx.focus_handle(),
            loaded: Loaded::Loading,
            camera: ImageCamera::new(),
            revision: 0,
        };
        view.load(cx);
        view
    }

    fn load(&mut self, cx: &Context<'_, Self>) {
        self.revision += 1;
        let revision = self.revision;
        let files = Arc::clone(&CurrentVault::expect(cx).files);
        let path = self.path.clone();
        let renderer = cx.svg_renderer();
        let task = run_blocking(cx, move || {
            let format = format_of(&path)?;
            let bytes = files.read_file(&path).ok()?;
            decode(format, bytes, &renderer)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                let (loaded, natural) = result
                    .map_or((Loaded::Failed, None), |(picture, natural)| {
                        (Loaded::Ready(picture), Some(natural))
                    });
                if let Loaded::Ready(previous) = mem::replace(&mut view.loaded, loaded) {
                    cx.drop_image(previous.image, None);
                }
                view.camera.set_natural(natural);
                cx.notify();
            })
        })
        .detach();
    }

    fn refresh_svg(&mut self, window: &Window, cx: &Context<'_, Self>) {
        let Loaded::Ready(Picture { svg: Some(svg), .. }) = &mut self.loaded else {
            return;
        };
        let wanted =
            (self.camera.current_scale() * window.scale_factor() / SMOOTH_SVG_SCALE_FACTOR * 4.0)
                .ceil()
                / 4.0;
        if svg.pending || wanted <= svg.density {
            return;
        }
        svg.pending = true;
        let source = Arc::clone(&svg.source);
        let renderer = cx.svg_renderer();
        let revision = self.revision;
        let task = run_blocking(cx, move || {
            renderer.render_single_frame(&source, wanted).ok()
        });
        cx.spawn(async move |this, cx| {
            let image = task.await;
            this.update(cx, |view, cx| {
                let Loaded::Ready(picture) = &mut view.loaded else {
                    return;
                };
                let Some(svg) = picture.svg.as_mut().filter(|_| view.revision == revision) else {
                    return;
                };
                svg.pending = false;
                svg.density = wanted;
                if let Some(image) = image {
                    cx.drop_image(mem::replace(&mut picture.image, image), None);
                    cx.notify();
                }
            })
        })
        .detach();
    }

    pub(crate) const fn camera(&self) -> &ImageCamera {
        &self.camera
    }

    pub(crate) fn zoom(&mut self, key: ZoomKey, cx: &mut Context<'_, Self>) {
        self.camera.apply(key);
        cx.notify();
    }

    pub(crate) fn meta(&self) -> Option<(u64, (f32, f32))> {
        match (&self.loaded, self.camera.natural()) {
            (Loaded::Ready(picture), Some(natural)) => Some((picture.bytes, natural)),
            _ => None,
        }
    }

    fn anchor(&self, position: Point<Pixels>) -> (f32, f32) {
        let center = self.camera.stage().get().viewport.center();
        (
            f32::from(position.x - center.x),
            f32::from(position.y - center.y),
        )
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(key) = zoom_key(event, cx) {
            self.camera.apply(key);
        } else if let Some(key) = scroll_key(event) {
            self.camera.scroll_key(key);
        } else {
            return;
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn on_wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let delta = event.delta.pixel_delta(px(16.));
        if event.modifiers.control || event.modifiers.platform {
            let anchor = self.anchor(event.position);
            self.camera
                .zoom_by(wheel_factor(-f32::from(delta.y)), anchor);
        } else {
            self.camera
                .scroll_by(f32::from(delta.x), f32::from(delta.y));
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn on_pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let anchor = self.anchor(event.position);
        self.camera.zoom_by(1.0 + event.delta, anchor);
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.focus_handle.focus(window, cx);
        self.camera.start_pan(event.position);
    }

    fn on_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        let changed = if event.pressed_button == Some(MouseButton::Left) {
            self.camera.move_pan(event.position)
        } else {
            self.camera.end_pan()
        };
        if changed {
            cx.notify();
        }
    }

    fn render_stage(&self, cx: &Context<'_, Self>) -> impl IntoElement + use<> {
        let palette = MarginTheme::palette(cx);
        let entity = cx.entity();
        let dragger = self.camera.is_pressed().then(|| cx.entity());
        let tracker = canvas(
            move |bounds, _, cx| {
                entity.update(cx, |view, cx| {
                    if view.camera.set_viewport(bounds) {
                        cx.notify();
                    }
                });
            },
            move |_, (), window, _| {
                if let Some(dragger) = dragger {
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            dragger.update(cx, |view, cx| view.on_drag(event, cx));
                        }
                    });
                }
            },
        )
        .absolute()
        .size_full();
        let stage = div()
            .id("image-stage")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .cursor(if self.camera.is_panning() {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .on_scroll(cx.listener(Self::on_wheel))
            .on_pinch(cx.listener(Self::on_pinch))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.camera.end_pan() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.camera.end_pan() {
                        cx.notify();
                    }
                }),
            )
            .child(tracker);
        match &self.loaded {
            Loaded::Loading => stage,
            Loaded::Failed => stage.flex().items_center().justify_center().child(
                div()
                    .max_w_80()
                    .px_6()
                    .text_center()
                    .text_size(css_px(13.0))
                    .text_color(palette.text_tertiary)
                    .child(t(cx, "viewer_image_failed")),
            ),
            Loaded::Ready(picture) => {
                let stage_state = self.camera.stage().get();
                let (width, height) = stage_state.content;
                let (view_width, view_height) = stage_state.viewport_size();
                let frame = div()
                    .absolute()
                    .left(px((view_width - width) / 2.0 + stage_state.offset.0))
                    .top(px((view_height - height) / 2.0 + stage_state.offset.1))
                    .w(px(width))
                    .h(px(height))
                    .shadow_md()
                    .when(picture.alpha, |frame| {
                        frame.child(checkerboard(palette.surface[0], palette.surface[2]))
                    })
                    .child(
                        img(ImageSource::Render(Arc::clone(&picture.image)))
                            .absolute()
                            .size_full(),
                    );
                stage
                    .child(frame)
                    .scrollbar(self.camera.stage(), ScrollbarAxis::Both)
            }
        }
    }
}

impl Render for ImageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.refresh_svg(window, cx);
        let palette = MarginTheme::palette(cx);
        v_flex()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .overflow_hidden()
            .bg(palette.surface[2])
            .child(self.crumbs.clone())
            .child(self.render_stage(cx))
            .child(toolbar(self, cx))
    }
}

impl Focusable for ImageView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<WorkbenchEvent> for ImageView {}

impl TabContent for ImageView {
    fn path(&self) -> &RelativePath {
        &self.path
    }

    fn rename(&mut self, to: RelativePath, cx: &mut Context<'_, Self>) {
        self.crumbs
            .update(cx, |crumbs, cx| crumbs.set_path(to.clone(), cx));
        self.path = to;
        cx.notify();
    }

    fn flush(&mut self, _cx: &mut Context<'_, Self>) {}

    fn reload(&mut self, cx: &mut Context<'_, Self>) {
        self.load(cx);
    }
}
