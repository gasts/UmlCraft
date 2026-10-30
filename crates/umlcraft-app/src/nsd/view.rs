//! The view of one structogram: its drawing, zoom and panning, and export.
//!
//! The diagram is drawn by `umlcraft_nsd`'s SVG export and rasterized with resvg, so that the
//! window shows exactly what an export holds, turned texts and hatching included. The image is
//! made again only when the zoom or the screen's scale changes. The diagram stays a white sheet
//! in the dark theme too, as in Structorizer; the area around it follows the theme.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Sizable, Theme, h_flex, v_flex};
use gpui_kit::*;
use resvg::{tiny_skia, usvg};
use rust_i18n::t;
use umlcraft_nsd::Diagram;
use umlcraft_nsd::layout::Layout;

use crate::{ExportPng, ExportSvg, ResetZoom, ZoomIn, ZoomOut, ZoomToFit};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
/// The longest side of the image a diagram is rasterized to, within GPU texture limits; a
/// larger diagram at a high zoom is scaled up from it.
const MAX_RASTER: f32 = 8192.0;
/// Space around the diagram at the start and when fitted.
const MARGIN: f32 = 20.0;

thread_local! {
    /// The system's fonts, loaded once for all rasterizing.
    static OPTIONS: Rc<usvg::Options<'static>> = {
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        Rc::new(options)
    };
}

/// A window size that shows the whole diagram where it fits on a usual screen.
pub fn window_size(diagram: &Diagram) -> (f32, f32) {
    let (w, h) = Layout::of(diagram, umlcraft_nsd::Metrics::for_platform()).size;
    // Room for the margins and the toolbar.
    let w = (w as f32 + 2.0 * MARGIN + 20.0).clamp(480.0, 1200.0);
    let h = (h as f32 + 2.0 * MARGIN + 60.0).clamp(360.0, 900.0);
    (w, h)
}

/// The diagram rasterized for one scale.
struct Raster {
    scale: f32,
    image: Arc<RenderImage>,
    /// Its size in pixels of the image.
    size: (u32, u32),
}

pub struct NsdView {
    path: PathBuf,
    /// The diagram as SVG, drawn once.
    svg: String,
    /// The diagram's size at 100 %.
    size: (f32, f32),
    zoom: f32,
    /// Where the diagram's top left corner is in the drawing area.
    pan: Point<Pixels>,
    raster: Option<Raster>,
    /// The drawing area, as painted last.
    area: Rc<Cell<Bounds<Pixels>>>,
    /// Where a drag that pans started, in window coordinates, and the pan then.
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
    focus: FocusHandle,
    _theme: Subscription,
}

impl NsdView {
    pub fn new(
        path: PathBuf,
        diagram: Diagram,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let metrics = umlcraft_nsd::Metrics::for_platform();
        let (w, h) = Layout::of(&diagram, metrics).size;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        NsdView {
            path,
            svg: umlcraft_nsd::svg::render(&diagram, metrics),
            size: ((w + 1) as f32, (h + 1) as f32),
            zoom: 1.0,
            pan: point(px(MARGIN), px(MARGIN)),
            raster: None,
            area: Rc::default(),
            drag: None,
            focus,
            // The area around the diagram follows the theme the main window switches.
            _theme: cx.observe_global::<Theme>(|_, cx| cx.notify()),
        }
    }

    /// The diagram as an image at `scale` pixels per diagram pixel (less for a large diagram,
    /// see [`MAX_RASTER`]), its size in pixels and the scale it has.
    fn raster(
        &mut self,
        scale: f32,
        window: &mut Window,
    ) -> Option<(Arc<RenderImage>, (u32, u32), f32)> {
        let scale = scale.min(MAX_RASTER / self.size.0.max(self.size.1));
        if self.raster.as_ref().is_none_or(|r| (r.scale - scale).abs() > 1e-4) {
            // GPUI keeps every image it painted in the window's atlas until it is dropped
            // there; without this each zoom step would leave its image behind.
            if let Some(old) = self.raster.take() {
                window.drop_image(old.image).ok();
            }
            self.raster =
                rasterize(&self.svg, scale).map(|(image, size)| Raster { scale, image, size });
        }
        self.raster.as_ref().map(|r| (r.image.clone(), r.size, r.scale))
    }

    fn set_zoom(&mut self, zoom: f32, anchor: Point<Pixels>, cx: &mut Context<Self>) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        // Keep the point under `anchor` (relative to the area) where it is.
        let factor = zoom / self.zoom;
        self.pan = point(
            anchor.x - (anchor.x - self.pan.x) * factor,
            anchor.y - (anchor.y - self.pan.y) * factor,
        );
        self.zoom = zoom;
        cx.notify();
    }

    fn centre(&self) -> Point<Pixels> {
        let area = self.area.get();
        point(area.size.width / 2.0, area.size.height / 2.0)
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom * 1.25, self.centre(), cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom / 1.25, self.centre(), cx);
    }

    fn reset_zoom(&mut self, _: &ResetZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan = point(px(MARGIN), px(MARGIN));
        cx.notify();
    }

    fn zoom_to_fit(&mut self, _: &ZoomToFit, _: &mut Window, cx: &mut Context<Self>) {
        let area = self.area.get().size;
        let fit_w = (f32::from(area.width) - 2.0 * MARGIN) / self.size.0;
        let fit_h = (f32::from(area.height) - 2.0 * MARGIN) / self.size.1;
        self.zoom = fit_w.min(fit_h).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = point(
            (area.width - px(self.size.0 * self.zoom)) / 2.0,
            (area.height - px(self.size.1 * self.zoom)) / 2.0,
        );
        cx.notify();
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = ev.delta.pixel_delta(px(20.0));
        if ev.modifiers.secondary() {
            let origin = self.area.get().origin;
            let anchor = point(ev.position.x - origin.x, ev.position.y - origin.y);
            let factor = (1.0 + f32::from(delta.y) / 200.0).clamp(0.5, 2.0);
            self.set_zoom(self.zoom * factor, anchor, cx);
        } else {
            self.pan = point(self.pan.x + delta.x, self.pan.y + delta.y);
            cx.notify();
        }
    }

    fn export(&mut self, png: bool, window: &mut Window, cx: &mut Context<Self>) {
        let extension = if png { "png" } else { "svg" };
        let stem = self.path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let suggested = format!("{}.{extension}", stem.unwrap_or_default());
        let directory = self.path.parent().map(PathBuf::from).unwrap_or_default();
        let rx = cx.prompt_for_new_path(&directory, Some(&suggested));
        let svg = self.svg.clone();
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) {
                path
            } else {
                path.with_extension(extension)
            };
            let written = cx
                .background_executor()
                .spawn(async move {
                    let bytes = if png {
                        // `from_svg` takes images made at Astah's 4/3 pixels per unit; ours
                        // have one.
                        let scale = crate::png::SCALE * umlcraft_notation::svg::SCALE;
                        crate::png::from_svg(&svg, scale)?
                    } else {
                        svg.into_bytes()
                    };
                    anyhow::Ok(std::fs::write(&path, bytes)?)
                })
                .await;
            if let Err(err) = written {
                cx.update(|window, cx| {
                    let rx = window.prompt(
                        PromptLevel::Critical,
                        &t!("dialog.image_failed"),
                        Some(&err.to_string()),
                        &[t!("dialog.ok").as_ref()],
                        cx,
                    );
                    cx.spawn(async move |_| rx.await.ok()).detach();
                })
                .ok();
            }
        })
        .detach();
    }

    fn export_svg(&mut self, _: &ExportSvg, window: &mut Window, cx: &mut Context<Self>) {
        self.export(false, window, cx);
    }

    fn export_png(&mut self, _: &ExportPng, window: &mut Window, cx: &mut Context<Self>) {
        self.export(true, window, cx);
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = (self.zoom * 100.0).round();
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(cx.theme().border);
        let action_button = |id: &'static str, label: String, action: Box<dyn Action>| {
            Button::new(id).ghost().small().label(label).on_click(move |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
            })
        };
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(action_button("export-svg", t!("menu.export_svg").into(), Box::new(ExportSvg)))
            .child(action_button("export-png", t!("menu.export_png").into(), Box::new(ExportPng)))
            .child(separator())
            .child(action_button("zoom-out", "−".into(), Box::new(ZoomOut)))
            .child(
                action_button("zoom-reset", format!("{zoom} %"), Box::new(ResetZoom))
                    .tooltip(t!("toolbar.zoom_reset")),
            )
            .child(action_button("zoom-in", "+".into(), Box::new(ZoomIn)))
            .child(action_button("zoom-fit", t!("toolbar.zoom_fit").into(), Box::new(ZoomToFit)))
    }
}

/// The SVG rasterized at `scale`, as GPUI takes images (BGRA, not premultiplied).
fn rasterize(svg: &str, scale: f32) -> Option<(Arc<RenderImage>, (u32, u32))> {
    let tree = OPTIONS.with(|options| usvg::Tree::from_str(svg, options)).ok()?;
    let size = tree.size().to_int_size().scale_by(scale)?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let (w, h) = (pixmap.width(), pixmap.height());
    let mut buffer = image::ImageBuffer::from_raw(w, h, pixmap.take())?;
    for pixel in buffer.as_chunks_mut::<4>().0 {
        swap_rgba_pa_to_bgra(pixel);
    }
    Some((Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])), (w, h)))
}

impl Render for NsdView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let area = self.area.clone();
        let surround = cx.theme().muted;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .track_focus(&self.focus)
            .key_context("NsdView")
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::reset_zoom))
            .on_action(cx.listener(Self::zoom_to_fit))
            .on_action(cx.listener(Self::export_svg))
            .on_action(cx.listener(Self::export_png))
            .child(self.toolbar(cx))
            .child(
                div()
                    .id("structogram")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .bg(surround)
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, _, _| {
                            this.drag = Some((ev.position, this.pan));
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if let Some((start, pan)) = this.drag {
                            if ev.pressed_button != Some(MouseButton::Left) {
                                this.drag = None;
                                return;
                            }
                            this.pan = point(
                                pan.x + ev.position.x - start.x,
                                pan.y + ev.position.y - start.y,
                            );
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = None),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| area.set(bounds),
                            move |bounds, _, window, cx| {
                                let scale = window.scale_factor();
                                let (pan, zoom, raster) = this.update(cx, |this, _| {
                                    (this.pan, this.zoom, this.raster(this.zoom * scale, window))
                                });
                                let Some((image, (w, h), raster_scale)) = raster else {
                                    return;
                                };
                                // The image covers the diagram at the zoom, whatever its scale.
                                let shown = zoom / raster_scale;
                                let size = size(px(w as f32 * shown), px(h as f32 * shown));
                                let image_bounds = Bounds::new(
                                    point(bounds.origin.x + pan.x, bounds.origin.y + pan.y),
                                    size,
                                );
                                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                                    window
                                        .paint_image(
                                            image_bounds,
                                            image_bounds,
                                            Corners::default(),
                                            image,
                                            0,
                                            false,
                                        )
                                        .ok();
                                });
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}

impl Focusable for NsdView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
