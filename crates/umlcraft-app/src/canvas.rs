//! The diagram editor surface: paints a [`Scene`] and handles tools, selection and dragging.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use umlcraft_model::{Id, Point as MPoint, Rect};
use umlcraft_notation::palette::Tool;
use umlcraft_notation::scene::{
    self, EdgeShape, EndMarker, Figure, Handle, LabelAlign, LineStyle, NodeShape, Preview, Scene,
};

use crate::workspace::Workspace;
use crate::{Cancel, DeleteSelection, RemoveFromDiagram, ResetZoom, SelectAll, ZoomToFit};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
const GRID: f32 = 20.0;
/// Smallest node edge when resizing.
const MIN_NODE: f32 = 20.0;
/// How close (in screen pixels) the pointer must be to grab a resize handle.
const HANDLE_GRAB: f32 = 6.0;

/// Snaps a logical coordinate or distance to half the grid.
fn snap(v: f32) -> f32 {
    (v / GRID * 2.0).round() * GRID / 2.0
}

#[derive(Clone, Copy)]
enum Drag {
    /// Moving the selected nodes; `start` is the logical grab point.
    Move {
        start: MPoint,
        dx: f32,
        dy: f32,
    },
    Resize {
        presentation: Id,
        handle: Handle,
        original: Rect,
        start: MPoint,
        rect: Rect,
    },
    RubberBand {
        start: MPoint,
        current: MPoint,
        extend: bool,
    },
    /// Drawing a new connection from `source`. Survives a click on the source itself, so both
    /// drag-and-drop and click-click work.
    Connect {
        source: Id,
        current: MPoint,
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
    /// Zoom and pan of diagrams that are not active right now.
    viewports: HashMap<Id, (f32, MPoint)>,
    shown_diagram: Option<Id>,
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
            viewports: HashMap::new(),
            shown_diagram: None,
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

    /// Fits the whole diagram into the visible area (at most 150 %).
    pub fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        let ws = self.workspace.read(cx);
        let content = ws.active_diagram.and_then(|d| ws.doc.model().view(d)?.content_bounds());
        let b = self.bounds.get();
        let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
        let Some(content) = content.filter(|_| w > 0.0 && h > 0.0) else { return };
        let margin = 40.0;
        let fit_w = (w - 2.0 * margin) / content.width.max(1.0);
        let fit_h = (h - 2.0 * margin) / content.height.max(1.0);
        self.zoom = fit_w.min(fit_h).clamp(MIN_ZOOM, 1.5);
        let c = content.center();
        self.pan = MPoint::new(w / 2.0 - c.x * self.zoom, h / 2.0 - c.y * self.zoom);
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

    /// Keeps a separate zoom and pan per diagram.
    fn switch_viewport(&mut self, diagram: Option<Id>) {
        if diagram == self.shown_diagram {
            return;
        }
        if let Some(old) = self.shown_diagram {
            self.viewports.insert(old, (self.zoom, self.pan));
        }
        (self.zoom, self.pan) = diagram
            .and_then(|d| self.viewports.get(&d).copied())
            .unwrap_or((1.0, MPoint::new(0.0, 0.0)));
        self.shown_diagram = diagram;
        self.drag = None;
    }

    fn preview(&self, ws: &Workspace) -> Preview {
        match self.drag {
            Some(Drag::Move { dx, dy, .. }) => {
                Preview { moved: ws.selection.iter().copied().collect(), dx, dy, resized: None }
            }
            Some(Drag::Resize { presentation, rect, .. }) => {
                Preview { resized: Some((presentation, rect)), ..Preview::default() }
            }
            _ => Preview::default(),
        }
    }

    fn current_scene(&self, cx: &App) -> Scene {
        let ws = self.workspace.read(cx);
        match ws.active_diagram {
            Some(diagram) => scene::build(ws.doc.model(), diagram, &self.preview(ws)),
            None => Scene { nodes: Vec::new(), edges: Vec::new() },
        }
    }

    fn node_at(scene: &Scene, p: MPoint) -> Option<Id> {
        scene.nodes.iter().rev().find(|n| n.rect.contains(p)).map(|n| n.id)
    }

    fn on_left_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        let p = self.to_logical(ev.position);
        let scene = self.current_scene(cx);
        let (tool, selection) = {
            let ws = self.workspace.read(cx);
            (ws.tool, ws.selection.clone())
        };
        let extend = ev.modifiers.shift || ev.modifiers.control;

        match tool {
            Tool::Node(node_tool) => {
                let origin = MPoint::new(snap(p.x), snap(p.y));
                self.workspace.update(cx, |ws, cx| {
                    ws.create_node(node_tool, origin);
                    ws.tool = Tool::Select;
                    cx.notify();
                });
            }
            Tool::Edge(edge_tool) => match (self.drag, Self::node_at(&scene, p)) {
                // Second click of click-click mode.
                (Some(Drag::Connect { source, .. }), Some(target)) if target != source => {
                    self.drag = None;
                    self.finish_connect(edge_tool, source, target, cx);
                }
                (_, Some(source)) => self.drag = Some(Drag::Connect { source, current: p }),
                (_, None) => self.drag = None,
            },
            Tool::Select => {
                // A resize handle of the single selected node wins over everything else.
                if let [single] = selection[..]
                    && let Some(node) = scene.nodes.iter().find(|n| n.id == single)
                    && let Some(handle) = Handle::at(node.rect, p, HANDLE_GRAB / self.zoom)
                {
                    self.drag = Some(Drag::Resize {
                        presentation: single,
                        handle,
                        original: node.rect,
                        start: p,
                        rect: node.rect,
                    });
                    cx.notify();
                    return;
                }
                match scene::hit_test(&scene, p, 4.0 / self.zoom) {
                    Some(id) => {
                        let already = selection.contains(&id);
                        self.workspace.update(cx, |ws, cx| {
                            if extend {
                                ws.toggle(id);
                            } else if !already {
                                ws.select(vec![id]);
                            }
                            cx.notify();
                        });
                        let is_node = scene.nodes.iter().any(|n| n.id == id);
                        if is_node && self.workspace.read(cx).selection.contains(&id) {
                            self.drag = Some(Drag::Move { start: p, dx: 0.0, dy: 0.0 });
                        }
                    }
                    None => {
                        if !extend {
                            self.workspace.update(cx, |ws, cx| {
                                ws.clear_selection();
                                cx.notify();
                            });
                        }
                        self.drag = Some(Drag::RubberBand { start: p, current: p, extend });
                    }
                }
            }
        }
        cx.notify();
    }

    fn finish_connect(
        &mut self,
        tool: umlcraft_notation::palette::EdgeTool,
        source: Id,
        target: Id,
        cx: &mut Context<Self>,
    ) {
        self.workspace.update(cx, |ws, cx| {
            ws.connect(tool, source, target);
            ws.tool = Tool::Select;
            cx.notify();
        });
    }

    fn on_middle_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.drag = Some(Drag::Pan { last: ev.position });
        cx.notify();
    }

    fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let p = self.to_logical(position);
        self.drag = match self.drag {
            Some(Drag::Move { start, .. }) => {
                Some(Drag::Move { start, dx: snap(p.x - start.x), dy: snap(p.y - start.y) })
            }
            Some(Drag::Resize { presentation, handle, original, start, .. }) => {
                let rect =
                    handle.resize(original, snap(p.x - start.x), snap(p.y - start.y), MIN_NODE);
                Some(Drag::Resize { presentation, handle, original, start, rect })
            }
            Some(Drag::RubberBand { start, extend, .. }) => {
                Some(Drag::RubberBand { start, current: p, extend })
            }
            Some(Drag::Connect { source, .. }) => Some(Drag::Connect { source, current: p }),
            Some(Drag::Pan { last }) => {
                self.pan =
                    self.pan.offset(f32::from(position.x - last.x), f32::from(position.y - last.y));
                Some(Drag::Pan { last: position })
            }
            None => return,
        };
        cx.notify();
    }

    fn end_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let p = self.to_logical(position);
        match self.drag.take() {
            Some(Drag::Move { dx, dy, .. }) => self.workspace.update(cx, |ws, cx| {
                ws.move_selection(dx, dy);
                cx.notify();
            }),
            Some(Drag::Resize { presentation, original, rect, .. }) if rect != original => {
                self.workspace.update(cx, |ws, cx| {
                    ws.set_bounds(presentation, rect);
                    cx.notify();
                });
            }
            Some(Drag::RubberBand { start, extend, .. }) => {
                let hits = scene::nodes_in(&self.current_scene(cx), Rect::from_corners(start, p));
                self.workspace.update(cx, |ws, cx| {
                    if extend {
                        let mut selection = ws.selection.clone();
                        selection.extend(hits.into_iter().filter(|h| !ws.selection.contains(h)));
                        ws.select(selection);
                    } else {
                        ws.select(hits);
                    }
                    cx.notify();
                });
            }
            Some(Drag::Connect { source, current }) => {
                let tool = self.workspace.read(cx).tool;
                match (tool, Self::node_at(&self.current_scene(cx), p)) {
                    (Tool::Edge(edge_tool), Some(target)) if target != source => {
                        self.finish_connect(edge_tool, source, target, cx);
                    }
                    // Released on the source: wait for the second click.
                    (Tool::Edge(_), Some(_)) => {
                        self.drag = Some(Drag::Connect { source, current });
                    }
                    _ => {}
                }
            }
            _ => {}
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

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.drag = None;
        self.workspace.update(cx, |ws, cx| {
            ws.tool = Tool::Select;
            cx.notify();
        });
        cx.notify();
    }

    fn update_workspace(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Workspace)) {
        self.workspace.update(cx, |ws, cx| {
            f(ws);
            cx.notify();
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
        // Wide enough for any role name; alignment does the actual placement.
        const BOX: f32 = 200.0;
        let text = self.c.text;
        for label in &edge.labels {
            let MPoint { x, y } = label.anchor;
            let (left, align, width) = match label.align {
                LabelAlign::Left => (x, TextAlign::Left, None),
                LabelAlign::Center => (x - BOX / 2.0, TextAlign::Center, Some(BOX)),
                LabelAlign::Right => (x - BOX, TextAlign::Right, Some(BOX)),
            };
            self.text(&label.text, left, y, 11.0, false, false, text, align, width);
        }
    }

    fn selection(&mut self, r: Rect, handles: bool) {
        let s = self.c.selection;
        self.window.paint_quad(quad(
            self.t.rect(r.inflate(3.0)),
            px(2.0),
            transparent_black(),
            px(1.5),
            s,
            BorderStyle::Solid,
        ));
        if handles {
            let half = 3.5 / self.t.zoom;
            for handle in Handle::ALL {
                let p = handle.position(r);
                let square = Rect::new(p.x - half, p.y - half, 2.0 * half, 2.0 * half);
                self.window.paint_quad(quad(
                    self.t.rect(square),
                    px(1.0),
                    self.c.background,
                    px(1.5),
                    s,
                    BorderStyle::Solid,
                ));
            }
        }
    }

    fn rubber_band(&mut self, area: Rect) {
        let s = self.c.selection;
        self.window.paint_quad(quad(
            self.t.rect(area),
            px(0.0),
            Hsla { a: 0.08, ..s },
            px(1.0),
            s,
            BorderStyle::Dashed,
        ));
    }
}

impl Render for DiagramCanvas {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (active, tool) = {
            let ws = self.workspace.read(cx);
            (ws.active_diagram, ws.tool)
        };
        self.switch_viewport(active);
        let scene = self.current_scene(cx);
        let selection = self.workspace.read(cx).selection.clone();
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
        let (pan, zoom, drag) = (self.pan, self.zoom, self.drag);
        let bounds_cell = self.bounds.clone();
        let this = cx.entity();
        let cursor = if tool == Tool::Select { CursorStyle::Arrow } else { CursorStyle::Crosshair };

        div()
            .id("diagram-canvas")
            .key_context("Canvas")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .bg(colors.background)
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(|this, _: &DeleteSelection, _, cx| {
                this.update_workspace(cx, Workspace::delete_selection);
            }))
            .on_action(cx.listener(|this, _: &RemoveFromDiagram, _, cx| {
                this.update_workspace(cx, Workspace::remove_selection_from_diagram);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.update_workspace(cx, Workspace::select_all);
            }))
            .on_action(cx.listener(|this, _: &ResetZoom, _, cx| this.reset_zoom(cx)))
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.zoom_to_fit(cx)))
            .child(
                canvas(
                    move |bounds, _, _| bounds_cell.set(bounds),
                    move |bounds, (), window, cx| {
                        if drag.is_some() {
                            let target = this.clone();
                            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    target.update(cx, |c, cx| c.drag_to(ev.position, cx));
                                }
                            });
                            let target = this.clone();
                            window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble
                                    && matches!(ev.button, MouseButton::Left | MouseButton::Middle)
                                {
                                    target.update(cx, |c, cx| c.end_drag(ev.position, cx));
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
                                p.edge(edge, selection.contains(&edge.id));
                            }
                            let selected_nodes: Vec<&NodeShape> =
                                scene.nodes.iter().filter(|n| selection.contains(&n.id)).collect();
                            let handles = selected_nodes.len() == 1 && selection.len() == 1;
                            for node in selected_nodes {
                                p.selection(node.rect, handles);
                            }
                            match drag {
                                Some(Drag::RubberBand { start, current, .. }) => {
                                    p.rubber_band(Rect::from_corners(start, current));
                                }
                                Some(Drag::Connect { source, current }) => {
                                    if let Some(node) = scene.nodes.iter().find(|n| n.id == source)
                                    {
                                        let from = scene::border_point(node.rect, current);
                                        let s = p.c.selection;
                                        p.line(&[from, current], 1.5, s, LineStyle::Dashed);
                                    }
                                }
                                _ => {}
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}
