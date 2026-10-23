//! The diagram editor surface: paints a [`Scene`] and handles pan, zoom, selection and dragging.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use umlcraft_model::{Id, Point as MPoint, Rect};

use crate::scene::{self, EdgeShape, EndMarker, Figure, LineStyle, NodeShape, Scene};
use crate::workspace::Workspace;
use crate::{DeleteSelection, ResetZoom};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
const GRID: f32 = 20.0;

#[derive(Clone, Copy)]
enum Drag {
    /// Moving a node; `start` is the logical grab point.
    Move {
        presentation: Id,
        start: MPoint,
        dx: f32,
        dy: f32,
    },
    Pan {
        last: Point<Pixels>,
    },
}

pub struct DiagramCanvas {
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
    zoom: f32,
    /// Screen offset of the logical origin, relative to the canvas bounds.
    pan: MPoint,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
}

impl DiagramCanvas {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        Self {
            workspace,
            focus_handle: cx.focus_handle(),
            zoom: 1.0,
            pan: MPoint::new(0.0, 0.0),
            bounds: Rc::new(Cell::new(Bounds::default())),
            drag: None,
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn reset_zoom(&mut self, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan = MPoint::new(0.0, 0.0);
        cx.notify();
    }

    /// Zooms by `factor`, keeping the canvas center fixed.
    pub fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let b = self.bounds.get();
        let center = point(b.size.width / 2.0, b.size.height / 2.0) + b.origin;
        self.zoom_at(center, factor);
        cx.notify();
    }

    fn zoom_at(&mut self, screen: Point<Pixels>, factor: f32) {
        let anchor = self.to_logical(screen);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let local = self.local(screen);
        self.pan = MPoint::new(local.x - anchor.x * self.zoom, local.y - anchor.y * self.zoom);
    }

    fn local(&self, screen: Point<Pixels>) -> MPoint {
        let origin = self.bounds.get().origin;
        MPoint::new(f32::from(screen.x - origin.x), f32::from(screen.y - origin.y))
    }

    fn to_logical(&self, screen: Point<Pixels>) -> MPoint {
        let local = self.local(screen);
        MPoint::new((local.x - self.pan.x) / self.zoom, (local.y - self.pan.y) / self.zoom)
    }

    fn current_scene(&self, cx: &App) -> Scene {
        let ws = self.workspace.read(cx);
        let Some(diagram) = ws.active_diagram else {
            return Scene { nodes: Vec::new(), edges: Vec::new() };
        };
        let moved = match self.drag {
            Some(Drag::Move { presentation, dx, dy, .. }) => Some((presentation, dx, dy)),
            _ => None,
        };
        scene::build(ws.doc.model(), diagram, moved)
    }

    fn on_left_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        let p = self.to_logical(ev.position);
        let hit = scene::hit_test(&self.current_scene(cx), p, 4.0 / self.zoom);
        let is_node = |ws: &Workspace, id: Id| {
            ws.active_diagram
                .and_then(|d| ws.doc.model().view(d))
                .and_then(|v| v.get(id))
                .is_some_and(|pr| !pr.is_edge())
        };
        self.workspace.update(cx, |ws, cx| {
            match hit {
                Some(id) => ws.select_presentation(id),
                None => ws.clear_selection(),
            }
            cx.notify();
        });
        if let Some(id) = hit
            && is_node(self.workspace.read(cx), id)
        {
            self.drag = Some(Drag::Move { presentation: id, start: p, dx: 0.0, dy: 0.0 });
        }
        cx.notify();
    }

    fn on_middle_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.drag = Some(Drag::Pan { last: ev.position });
        cx.notify();
    }

    fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        match self.drag {
            Some(Drag::Move { presentation, start, .. }) => {
                let p = self.to_logical(position);
                // Snap the movement to the grid.
                let snap = |v: f32| (v / GRID * 2.0).round() * GRID / 2.0;
                self.drag = Some(Drag::Move {
                    presentation,
                    start,
                    dx: snap(p.x - start.x),
                    dy: snap(p.y - start.y),
                });
            }
            Some(Drag::Pan { last }) => {
                self.pan =
                    self.pan.offset(f32::from(position.x - last.x), f32::from(position.y - last.y));
                self.drag = Some(Drag::Pan { last: position });
            }
            None => return,
        }
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(Drag::Move { presentation, dx, dy, .. }) = self.drag.take()
            && (dx != 0.0 || dy != 0.0)
        {
            self.workspace.update(cx, |ws, cx| {
                let Some(diagram) = ws.active_diagram else {
                    return;
                };
                let bounds = ws
                    .doc
                    .model()
                    .view(diagram)
                    .and_then(|v| v.get(presentation))
                    .and_then(|p| p.bounds());
                if let Some(b) = bounds {
                    let label = rust_i18n::t!("edit.move").to_string();
                    ws.doc
                        .edit(label, |tx| tx.set_bounds(diagram, presentation, b.translate(dx, dy)))
                        .expect("moving a node is always valid");
                    cx.notify();
                }
            });
        }
        cx.notify();
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = ev.delta.pixel_delta(px(16.0));
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        if ev.modifiers.control {
            self.zoom_at(ev.position, (1.0 + dy * 0.002).clamp(0.5, 2.0));
        } else if ev.modifiers.shift {
            self.pan = self.pan.offset(dy, dx);
        } else {
            self.pan = self.pan.offset(dx, dy);
        }
        cx.notify();
    }

    fn delete_selection(&mut self, _: &DeleteSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, cx| {
            let target = ws.selected_element;
            if let Some(element) = target {
                let label = rust_i18n::t!("edit.delete").to_string();
                if ws.doc.edit(label, |tx| tx.delete(element)).is_ok() {
                    ws.clear_selection();
                    cx.notify();
                }
            }
        });
    }
}

impl Focusable for DiagramCanvas {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Logical → screen mapping for one frame.
#[derive(Clone, Copy)]
struct Transform {
    origin: Point<Pixels>,
    pan: MPoint,
    zoom: f32,
}

impl Transform {
    fn pt(&self, p: MPoint) -> Point<Pixels> {
        self.origin + point(px(self.pan.x + p.x * self.zoom), px(self.pan.y + p.y * self.zoom))
    }

    fn len(&self, v: f32) -> Pixels {
        px(v * self.zoom)
    }

    fn rect(&self, r: Rect) -> Bounds<Pixels> {
        Bounds::new(self.pt(r.origin()), size(self.len(r.width), self.len(r.height)))
    }

    fn to_logical(self, p: Point<Pixels>) -> MPoint {
        MPoint::new(
            (f32::from(p.x - self.origin.x) - self.pan.x) / self.zoom,
            (f32::from(p.y - self.origin.y) - self.pan.y) / self.zoom,
        )
    }
}

#[derive(Clone, Copy)]
struct Colors {
    background: Hsla,
    grid: Hsla,
    fill: Hsla,
    line: Hsla,
    text: Hsla,
    muted: Hsla,
    selection: Hsla,
}

struct Painter<'a> {
    t: Transform,
    c: Colors,
    font_family: SharedString,
    window: &'a mut Window,
    cx: &'a mut App,
}

impl Painter<'_> {
    fn line(&mut self, points: &[MPoint], width: f32, color: Hsla, style: LineStyle) {
        if points.len() < 2 {
            return;
        }
        let mut path = PathBuilder::stroke(px(width));
        if style == LineStyle::Dashed {
            path = path.dash_array(&[px(6.0 * self.t.zoom), px(4.0 * self.t.zoom)]);
        }
        path.move_to(self.t.pt(points[0]));
        for &p in &points[1..] {
            path.line_to(self.t.pt(p));
        }
        if let Ok(path) = path.build() {
            self.window.paint_path(path, color);
        }
    }

    fn polygon(&mut self, points: &[MPoint], fill: Option<Hsla>, stroke: Hsla) {
        let screen: Vec<_> = points.iter().map(|&p| self.t.pt(p)).collect();
        if let Some(fill) = fill {
            let mut path = PathBuilder::fill();
            path.add_polygon(&screen, true);
            if let Ok(path) = path.build() {
                self.window.paint_path(path, fill);
            }
        }
        let mut path = PathBuilder::stroke(px(1.0));
        path.add_polygon(&screen, true);
        if let Ok(path) = path.build() {
            self.window.paint_path(path, stroke);
        }
    }

    fn ellipse(&mut self, center: MPoint, rx: f32, ry: f32, fill: Option<Hsla>, stroke: Hsla) {
        let build = |mut path: PathBuilder, t: &Transform| {
            let radii = point(t.len(rx), t.len(ry));
            path.move_to(t.pt(center.offset(rx, 0.0)));
            path.arc_to(radii, px(0.0), false, true, t.pt(center.offset(-rx, 0.0)));
            path.arc_to(radii, px(0.0), false, true, t.pt(center.offset(rx, 0.0)));
            path.close();
            path.build()
        };
        if let Some(fill) = fill
            && let Ok(path) = build(PathBuilder::fill(), &self.t)
        {
            self.window.paint_path(path, fill);
        }
        if let Ok(path) = build(PathBuilder::stroke(px(1.0)), &self.t) {
            self.window.paint_path(path, stroke);
        }
    }

    fn rect(&mut self, r: Rect, fill: Hsla, border: Hsla, style: BorderStyle) {
        self.window.paint_quad(quad(self.t.rect(r), px(0.0), fill, px(1.0), border, style));
    }

    /// Draws one line of text. `x` is the left edge, or the center with `TextAlign::Center`
    /// and a `width`. Returns the logical width.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        bold: bool,
        italic: bool,
        color: Hsla,
        align: TextAlign,
        width: Option<f32>,
    ) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let mut f = font(self.font_family.clone());
        if bold {
            f = f.bold();
        }
        if italic {
            f = f.italic();
        }
        let text: SharedString = text.to_owned().into();
        let run = TextRun { len: text.len(), font: f, color, ..Default::default() };
        let line = self.window.text_system().shape_line(text, self.t.len(font_size), &[run], None);
        let line_height = self.t.len(font_size * 1.3);
        let measured = f32::from(line.width()) / self.t.zoom;
        line.paint(
            self.t.pt(MPoint::new(x, y)),
            line_height,
            align,
            width.map(|w| self.t.len(w)),
            self.window,
            self.cx,
        )
        .ok();
        measured
    }

    fn grid(&mut self, bounds: Bounds<Pixels>) {
        let step = GRID * self.t.zoom;
        if step < 8.0 {
            return;
        }
        let top_left = self.t.to_logical(bounds.origin);
        let bottom_right = self.t.to_logical(bounds.bottom_right());
        let mut x = (top_left.x / GRID).floor() * GRID;
        while x <= bottom_right.x {
            let sx = self.t.pt(MPoint::new(x, 0.0)).x;
            self.window.paint_quad(fill(
                Bounds::new(point(sx, bounds.origin.y), size(px(1.0), bounds.size.height)),
                self.c.grid,
            ));
            x += GRID;
        }
        let mut y = (top_left.y / GRID).floor() * GRID;
        while y <= bottom_right.y {
            let sy = self.t.pt(MPoint::new(0.0, y)).y;
            self.window.paint_quad(fill(
                Bounds::new(point(bounds.origin.x, sy), size(bounds.size.width, px(1.0))),
                self.c.grid,
            ));
            y += GRID;
        }
    }

    fn node(&mut self, node: &NodeShape) {
        let r = node.rect;
        let c = self.c;
        let center = TextAlign::Center;
        match &node.figure {
            Figure::Classifier { stereotype, name, is_abstract, compartments } => {
                self.rect(r, c.fill, c.line, BorderStyle::Solid);
                let mut y = r.y + 4.0;
                if let Some(s) = stereotype {
                    self.text(s, r.x, y, 11.0, false, false, c.text, center, Some(r.width));
                    y += 14.0;
                }
                self.text(name, r.x, y, 13.0, true, *is_abstract, c.text, center, Some(r.width));
                y += 22.0;
                for members in compartments {
                    if y >= r.bottom() {
                        break;
                    }
                    self.line(
                        &[MPoint::new(r.x, y), MPoint::new(r.right(), y)],
                        1.0,
                        c.line,
                        LineStyle::Solid,
                    );
                    y += 3.0;
                    for m in members {
                        if y + 15.0 > r.bottom() {
                            break;
                        }
                        self.text(
                            m,
                            r.x + 6.0,
                            y,
                            12.0,
                            false,
                            false,
                            c.text,
                            TextAlign::Left,
                            None,
                        );
                        y += 16.0;
                    }
                    y += 3.0;
                }
            }
            Figure::Package { name } => {
                let tab = Rect::new(r.x, r.y, (r.width * 0.4).min(80.0), 14.0);
                self.rect(tab, c.fill, c.line, BorderStyle::Solid);
                let body = Rect::new(r.x, r.y + 14.0, r.width, r.height - 14.0);
                self.rect(body, c.fill, c.line, BorderStyle::Solid);
                self.text(
                    name,
                    r.x,
                    body.y + 6.0,
                    13.0,
                    true,
                    false,
                    c.text,
                    center,
                    Some(r.width),
                );
            }
            Figure::Actor { name } => {
                let cx = r.x + r.width / 2.0;
                let head = (r.width.min(r.height) * 0.18).max(6.0);
                let neck = r.y + head * 2.0;
                let hip = r.y + r.height * 0.62;
                self.ellipse(MPoint::new(cx, r.y + head), head, head, Some(c.fill), c.line);
                let solid = LineStyle::Solid;
                self.line(&[MPoint::new(cx, neck), MPoint::new(cx, hip)], 1.0, c.line, solid);
                let arms = r.y + r.height * 0.38;
                self.line(
                    &[MPoint::new(r.x, arms), MPoint::new(r.right(), arms)],
                    1.0,
                    c.line,
                    solid,
                );
                self.line(
                    &[
                        MPoint::new(r.x, r.bottom()),
                        MPoint::new(cx, hip),
                        MPoint::new(r.right(), r.bottom()),
                    ],
                    1.0,
                    c.line,
                    solid,
                );
                self.text(
                    name,
                    r.x - 40.0,
                    r.bottom() + 4.0,
                    12.0,
                    false,
                    false,
                    c.text,
                    center,
                    Some(r.width + 80.0),
                );
            }
            Figure::UseCase { name } => {
                self.ellipse(r.center(), r.width / 2.0, r.height / 2.0, Some(c.fill), c.line);
                self.text(
                    name,
                    r.x,
                    r.center().y - 8.0,
                    12.0,
                    false,
                    false,
                    c.text,
                    center,
                    Some(r.width),
                );
            }
            Figure::Lifeline { header } => {
                let head = Rect::new(r.x, r.y, r.width, 32.0);
                self.rect(head, c.fill, c.line, BorderStyle::Solid);
                self.text(
                    header,
                    r.x,
                    r.y + 8.0,
                    12.0,
                    false,
                    false,
                    c.text,
                    center,
                    Some(r.width),
                );
                let x = r.x + r.width / 2.0;
                self.line(
                    &[MPoint::new(x, head.bottom()), MPoint::new(x, r.bottom())],
                    1.0,
                    c.line,
                    LineStyle::Dashed,
                );
            }
            Figure::Note { text } => {
                let fold = 10.0;
                self.polygon(
                    &[
                        r.origin(),
                        MPoint::new(r.right() - fold, r.y),
                        MPoint::new(r.right(), r.y + fold),
                        MPoint::new(r.right(), r.bottom()),
                        MPoint::new(r.x, r.bottom()),
                    ],
                    Some(c.fill),
                    c.line,
                );
                self.text(
                    text,
                    r.x + 6.0,
                    r.y + 6.0,
                    12.0,
                    false,
                    false,
                    c.text,
                    TextAlign::Left,
                    None,
                );
            }
            Figure::Other { name } => {
                self.rect(r, c.fill, c.muted, BorderStyle::Dashed);
                self.text(name, r.x, r.y + 6.0, 12.0, false, false, c.text, center, Some(r.width));
            }
        }
    }

    fn marker(&mut self, tip: MPoint, from: MPoint, marker: EndMarker) {
        let (dx, dy) = (tip.x - from.x, tip.y - from.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 || marker == EndMarker::None {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        let at = |back: f32, side: f32| {
            MPoint::new(tip.x - ux * back - uy * side, tip.y - uy * back + ux * side)
        };
        let c = self.c;
        match marker {
            EndMarker::None => {}
            EndMarker::OpenArrow => {
                self.line(&[at(10.0, 5.0), tip, at(10.0, -5.0)], 1.0, c.line, LineStyle::Solid);
            }
            EndMarker::FilledArrow => {
                self.polygon(&[tip, at(10.0, 5.0), at(10.0, -5.0)], Some(c.line), c.line)
            }
            EndMarker::HollowTriangle => {
                self.polygon(&[tip, at(14.0, 8.0), at(14.0, -8.0)], Some(c.background), c.line)
            }
            EndMarker::HollowDiamond | EndMarker::FilledDiamond => {
                let fill = if marker == EndMarker::FilledDiamond { c.line } else { c.background };
                self.polygon(
                    &[tip, at(8.0, 5.0), at(16.0, 0.0), at(8.0, -5.0)],
                    Some(fill),
                    c.line,
                );
            }
        }
    }

    fn edge(&mut self, edge: &EdgeShape, selected: bool) {
        let pts = &edge.points;
        let color = if selected { self.c.selection } else { self.c.line };
        self.line(pts, if selected { 2.0 } else { 1.0 }, color, edge.line);
        let n = pts.len();
        self.marker(pts[n - 1], pts[n - 2], edge.target_marker);
        self.marker(pts[0], pts[1], edge.source_marker);
        let muted = self.c.text;
        if let Some(label) = &edge.label {
            let mid = MPoint::new(
                (pts[n / 2 - 1].x + pts[n / 2].x) / 2.0,
                (pts[n / 2 - 1].y + pts[n / 2].y) / 2.0,
            );
            self.text(
                label,
                mid.x - 60.0,
                mid.y - 18.0,
                11.0,
                false,
                false,
                muted,
                TextAlign::Center,
                Some(120.0),
            );
        }
        let end_label = |tip: MPoint, from: MPoint| {
            let (dx, dy) = (from.x - tip.x, from.y - tip.y);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            MPoint::new(tip.x + dx / len * 22.0 + 6.0, tip.y + dy / len * 22.0 - 6.0)
        };
        if let Some(label) = &edge.source_label {
            let p = end_label(pts[0], pts[1]);
            self.text(label, p.x, p.y, 11.0, false, false, muted, TextAlign::Left, None);
        }
        if let Some(label) = &edge.target_label {
            let p = end_label(pts[n - 1], pts[n - 2]);
            self.text(label, p.x, p.y, 11.0, false, false, muted, TextAlign::Left, None);
        }
    }

    fn selection(&mut self, r: Rect) {
        let outline = r.inflate(3.0);
        let s = self.c.selection;
        self.window.paint_quad(quad(
            self.t.rect(outline),
            px(2.0),
            transparent_black(),
            px(1.5),
            s,
            BorderStyle::Solid,
        ));
        for (x, y) in [
            (outline.x, outline.y),
            (outline.right(), outline.y),
            (outline.x, outline.bottom()),
            (outline.right(), outline.bottom()),
        ] {
            let handle = Rect::new(
                x - 3.5 / self.t.zoom,
                y - 3.5 / self.t.zoom,
                7.0 / self.t.zoom,
                7.0 / self.t.zoom,
            );
            self.window.paint_quad(fill(self.t.rect(handle), s));
        }
    }
}

impl Render for DiagramCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scene = self.current_scene(cx);
        let selected = self.workspace.read(cx).selected_presentation;
        let theme = cx.theme();
        let colors = Colors {
            background: theme.background,
            grid: Hsla { a: if theme.is_dark() { 0.25 } else { 0.45 }, ..theme.border },
            fill: if theme.is_dark() {
                theme.secondary
            } else {
                hsla(48.0 / 360.0, 1.0, 0.95, 1.0)
            },
            line: theme.foreground,
            text: theme.foreground,
            muted: theme.muted_foreground,
            selection: theme.blue,
        };
        let font_family = theme.font_family.clone();
        let (pan, zoom) = (self.pan, self.zoom);
        let bounds_cell = self.bounds.clone();
        let dragging = self.drag.is_some();
        let this = cx.entity();
        let _ = window;

        div()
            .id("diagram-canvas")
            .key_context("Canvas")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .bg(colors.background)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(|this, _: &ResetZoom, _, cx| this.reset_zoom(cx)))
            .child(
                canvas(
                    move |bounds, _, _| bounds_cell.set(bounds),
                    move |bounds, (), window, cx| {
                        if dragging {
                            let target = this.clone();
                            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    target.update(cx, |c, cx| c.drag_to(ev.position, cx));
                                }
                            });
                            let target = this.clone();
                            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    target.update(cx, |c, cx| c.end_drag(cx));
                                }
                            });
                        }
                        let t = Transform { origin: bounds.origin, pan, zoom };
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            let mut p = Painter { t, c: colors, font_family, window, cx };
                            p.grid(bounds);
                            for node in &scene.nodes {
                                p.node(node);
                            }
                            for edge in &scene.edges {
                                p.edge(edge, Some(edge.id) == selected);
                            }
                            if let Some(node) = scene.nodes.iter().find(|n| Some(n.id) == selected)
                            {
                                p.selection(node.rect);
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}
