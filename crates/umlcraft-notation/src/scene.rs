//! Turns a diagram view into simple drawable shapes in logical coordinates.

use std::collections::HashSet;

use umlcraft_model::{
    Aggregation, ElementKind, Id, MessageSort, Model, Navigability, Point, Presentation, Rect,
    Shape, TypeRef, Visibility,
};

pub struct Scene {
    pub nodes: Vec<NodeShape>,
    pub edges: Vec<EdgeShape>,
}

pub struct NodeShape {
    pub id: Id,
    pub rect: Rect,
    pub figure: Figure,
}

pub enum Figure {
    Classifier {
        stereotype: Option<String>,
        name: String,
        is_abstract: bool,
        compartments: Vec<Vec<String>>,
    },
    Package {
        name: String,
    },
    Actor {
        name: String,
    },
    UseCase {
        name: String,
    },
    Lifeline {
        header: String,
    },
    Note {
        text: String,
    },
    Other {
        name: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EndMarker {
    None,
    OpenArrow,
    FilledArrow,
    HollowTriangle,
    HollowDiamond,
    FilledDiamond,
}

pub struct EdgeShape {
    pub id: Id,
    /// Polyline from the source border to the target border.
    pub points: Vec<Point>,
    pub line: LineStyle,
    pub source_marker: EndMarker,
    pub target_marker: EndMarker,
    pub labels: Vec<EdgeLabel>,
}

/// Height of an edge label line (11 pt text) in logical units.
pub const LABEL_HEIGHT: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelAlign {
    Left,
    Center,
    Right,
}

/// A text placed next to an edge. `anchor.y` is the top of the text; `anchor.x` is its left
/// edge, center or right edge depending on `align`.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeLabel {
    pub text: String,
    pub anchor: Point,
    pub align: LabelAlign,
}

/// An uncommitted change shown while the user drags: moved nodes and a resized node.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub moved: HashSet<Id>,
    pub dx: f32,
    pub dy: f32,
    pub resized: Option<(Id, Rect)>,
}

impl Preview {
    pub fn rect(&self, presentation: Id, rect: Rect) -> Rect {
        match self.resized {
            Some((id, r)) if id == presentation => r,
            _ if self.moved.contains(&presentation) => rect.translate(self.dx, self.dy),
            _ => rect,
        }
    }
}

/// Builds the scene of `diagram`, with `preview` applied.
pub fn build(model: &Model, diagram: Id, preview: &Preview) -> Scene {
    let Some(view) = model.view(diagram) else {
        return Scene { nodes: Vec::new(), edges: Vec::new() };
    };
    let rect_of = |p: &Presentation| p.bounds().map(|r| preview.rect(p.id, r));
    let nodes = view
        .presentations()
        .iter()
        .filter_map(|p| {
            let rect = rect_of(p)?;
            let element = p.element.and_then(|e| model.get(e));
            let figure = match element {
                Some(e) => figure_for(model, e.id()),
                None => Figure::Other { name: String::new() },
            };
            Some(NodeShape { id: p.id, rect, figure })
        })
        .collect();
    let edges = view
        .presentations()
        .iter()
        .filter_map(|p| {
            let Shape::Edge { source, target, waypoints } = &p.shape else {
                return None;
            };
            let source_rect = view.get(*source).and_then(rect_of)?;
            let target_rect = view.get(*target).and_then(rect_of)?;
            let kind = p.element.and_then(|e| model.get(e)).map(|e| e.kind());
            Some(edge_for(p.id, kind, source_rect, target_rect, waypoints))
        })
        .collect();
    Scene { nodes, edges }
}

fn figure_for(model: &Model, element: Id) -> Figure {
    let e = model.get(element).expect("caller checked");
    let name = e.name().to_owned();
    let stereotype = e.data().stereotypes.first().map(|s| format!("«{s}»"));
    let members = |pick: fn(&ElementKind) -> bool| -> Vec<String> {
        e.children()
            .iter()
            .filter_map(|&c| model.get(c))
            .filter(|c| pick(c.kind()))
            .map(|c| member_label(model, c.data().visibility, c.name(), c.kind()))
            .collect()
    };
    match e.kind() {
        ElementKind::Class(p) | ElementKind::DataType(p) => Figure::Classifier {
            stereotype,
            name,
            is_abstract: p.is_abstract,
            compartments: vec![
                members(|k| matches!(k, ElementKind::Attribute(_))),
                members(|k| matches!(k, ElementKind::Operation(_))),
            ],
        },
        ElementKind::Interface(_) => Figure::Classifier {
            stereotype: Some("«interface»".into()),
            name,
            is_abstract: false,
            compartments: vec![members(|k| matches!(k, ElementKind::Operation(_)))],
        },
        ElementKind::Enumeration(_) => Figure::Classifier {
            stereotype: Some("«enumeration»".into()),
            name,
            is_abstract: false,
            compartments: vec![members(|k| matches!(k, ElementKind::EnumerationLiteral))],
        },
        ElementKind::Package | ElementKind::Model => Figure::Package { name },
        ElementKind::Actor(_) => Figure::Actor { name },
        ElementKind::UseCase(_) => Figure::UseCase { name },
        ElementKind::Lifeline(p) => {
            let type_name = p.represents.and_then(|t| model.get(t)).map(|t| t.name());
            let header = match type_name {
                Some(t) => format!("{name} : {t}"),
                None => name,
            };
            Figure::Lifeline { header }
        }
        ElementKind::Comment(p) => Figure::Note { text: p.body.clone() },
        _ => Figure::Other { name },
    }
}

fn type_label(model: &Model, type_ref: &Option<TypeRef>) -> Option<String> {
    match type_ref {
        Some(TypeRef::Element(id)) => model.get(*id).map(|t| t.name().to_owned()),
        Some(TypeRef::Text(text)) => Some(text.clone()),
        None => None,
    }
}

/// `- name : Type [1..*] = init` resp. `+ op(p : T) : R`.
pub fn member_label(
    model: &Model,
    visibility: Visibility,
    name: &str,
    kind: &ElementKind,
) -> String {
    let vis = visibility.symbol();
    match kind {
        ElementKind::Attribute(p) => {
            let mut s = format!("{vis} {name}");
            if let Some(t) = type_label(model, &p.type_ref) {
                s.push_str(&format!(" : {t}"));
            }
            if let Some(m) = p.multiplicity {
                s.push_str(&format!(" [{m}]"));
            }
            if !p.default_value.is_empty() {
                s.push_str(&format!(" = {}", p.default_value));
            }
            s
        }
        ElementKind::Operation(p) => {
            let mut s = format!("{vis} {name}()");
            if let Some(t) = type_label(model, &p.return_type) {
                s.push_str(&format!(" : {t}"));
            }
            s
        }
        _ => name.to_owned(),
    }
}

fn edge_for(
    id: Id,
    kind: Option<&ElementKind>,
    source: Rect,
    target: Rect,
    waypoints: &[Point],
) -> EdgeShape {
    let mut points = Vec::with_capacity(waypoints.len() + 2);
    let first_toward = waypoints.first().copied().unwrap_or(target.center());
    let last_from = waypoints.last().copied().unwrap_or(source.center());
    points.push(border_point(source, first_toward));
    points.extend_from_slice(waypoints);
    points.push(border_point(target, last_from));

    let mut edge = EdgeShape {
        id,
        points,
        line: LineStyle::Solid,
        source_marker: EndMarker::None,
        target_marker: EndMarker::None,
        labels: Vec::new(),
    };
    let (mut label, mut source_label, mut target_label) = (None, None, None);
    let diamond = |a: Aggregation| match a {
        Aggregation::None => EndMarker::None,
        Aggregation::Shared => EndMarker::HollowDiamond,
        Aggregation::Composite => EndMarker::FilledDiamond,
    };
    match kind {
        Some(ElementKind::Association(p)) => {
            let [a, b] = &p.ends;
            edge.source_marker = diamond(a.aggregation);
            edge.target_marker = diamond(b.aggregation);
            if b.navigability == Navigability::Navigable && edge.target_marker == EndMarker::None {
                edge.target_marker = EndMarker::OpenArrow;
            }
            if a.navigability == Navigability::Navigable && edge.source_marker == EndMarker::None {
                edge.source_marker = EndMarker::OpenArrow;
            }
            let end_label = |role: &str, m: Option<umlcraft_model::Multiplicity>| {
                let text = [role.to_owned(), m.map(|m| m.to_string()).unwrap_or_default()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                (!text.is_empty()).then_some(text)
            };
            source_label = end_label(&a.role, a.multiplicity);
            target_label = end_label(&b.role, b.multiplicity);
        }
        Some(ElementKind::Generalization(_)) => edge.target_marker = EndMarker::HollowTriangle,
        Some(ElementKind::Realization(_)) => {
            edge.line = LineStyle::Dashed;
            edge.target_marker = EndMarker::HollowTriangle;
        }
        Some(ElementKind::Dependency(_)) => {
            edge.line = LineStyle::Dashed;
            edge.target_marker = EndMarker::OpenArrow;
        }
        Some(ElementKind::Include(_)) => {
            edge.line = LineStyle::Dashed;
            edge.target_marker = EndMarker::OpenArrow;
            label = Some("«include»".to_owned());
        }
        Some(ElementKind::Extend(_)) => {
            edge.line = LineStyle::Dashed;
            edge.target_marker = EndMarker::OpenArrow;
            label = Some("«extend»".to_owned());
        }
        Some(ElementKind::Message(p)) => match p.sort {
            MessageSort::Asynchronous => edge.target_marker = EndMarker::OpenArrow,
            MessageSort::Reply => {
                edge.line = LineStyle::Dashed;
                edge.target_marker = EndMarker::OpenArrow;
            }
            _ => edge.target_marker = EndMarker::FilledArrow,
        },
        _ => edge.line = LineStyle::Dashed,
    }

    let pts = &edge.points;
    let n = pts.len();
    edge.labels.extend(label.map(|text| middle_label(text, pts[n / 2 - 1], pts[n / 2])));
    edge.labels.extend(source_label.map(|text| end_label(text, pts[0], pts[1])));
    edge.labels.extend(target_label.map(|text| end_label(text, pts[n - 1], pts[n - 2])));
    edge
}

/// Below this slope a segment counts as horizontal (resp. vertical) for label placement.
const STRAIGHT: f32 = 0.2;

/// Places an end label (role, multiplicity) next to the segment `tip`–`from`, a little away
/// from the node at `tip`, on the side of the line it does not run into.
fn end_label(text: String, tip: Point, from: Point) -> EdgeLabel {
    const GAP: f32 = 10.0;
    let len = tip.distance(from).max(1.0);
    let (ux, uy) = ((from.x - tip.x) / len, (from.y - tip.y) / len);
    let base = Point::new(tip.x + ux * GAP, tip.y + uy * GAP);
    if ux.abs() >= uy.abs() {
        // Mostly horizontal: the text extends away from the node, above the line unless the
        // line rises noticeably in that direction.
        let align = if ux >= 0.0 { LabelAlign::Left } else { LabelAlign::Right };
        let y = if uy < -STRAIGHT { base.y + 3.0 } else { base.y - LABEL_HEIGHT - 3.0 };
        EdgeLabel { text, anchor: Point::new(base.x, y), align }
    } else {
        // Mostly vertical: beside the line, right of it unless the line leans to the right.
        let (x, align) = if ux > STRAIGHT {
            (base.x - 6.0, LabelAlign::Right)
        } else {
            (base.x + 6.0, LabelAlign::Left)
        };
        let y = if uy >= 0.0 { base.y } else { base.y - LABEL_HEIGHT };
        EdgeLabel { text, anchor: Point::new(x, y), align }
    }
}

/// Places a name or stereotype label at the middle of the segment `a`–`b`.
fn middle_label(text: String, a: Point, b: Point) -> EdgeLabel {
    let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        EdgeLabel {
            text,
            anchor: Point::new(mid.x, mid.y - LABEL_HEIGHT - 3.0),
            align: LabelAlign::Center,
        }
    } else {
        EdgeLabel {
            text,
            anchor: Point::new(mid.x + 6.0, mid.y - LABEL_HEIGHT / 2.0),
            align: LabelAlign::Left,
        }
    }
}

/// Where the line from the center of `rect` toward `toward` leaves the rectangle.
pub fn border_point(rect: Rect, toward: Point) -> Point {
    let c = rect.center();
    let (dx, dy) = (toward.x - c.x, toward.y - c.y);
    if dx == 0.0 && dy == 0.0 {
        return c;
    }
    let sx = if dx != 0.0 { (rect.width / 2.0) / dx.abs() } else { f32::INFINITY };
    let sy = if dy != 0.0 { (rect.height / 2.0) / dy.abs() } else { f32::INFINITY };
    let s = sx.min(sy).min(1.0);
    Point::new(c.x + dx * s, c.y + dy * s)
}

/// Distance from `p` to the segment `a`–`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = vx * vx + vy * vy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * vx + (p.y - a.y) * vy) / len2).clamp(0.0, 1.0)
    };
    p.distance(Point::new(a.x + t * vx, a.y + t * vy))
}

/// Topmost node or edge under `p` (logical coordinates), with `tolerance` for thin lines.
pub fn hit_test(scene: &Scene, p: Point, tolerance: f32) -> Option<Id> {
    let edge = scene
        .edges
        .iter()
        .rev()
        .find(|e| e.points.windows(2).any(|w| distance_to_segment(p, w[0], w[1]) <= tolerance));
    if let Some(edge) = edge {
        return Some(edge.id);
    }
    scene.nodes.iter().rev().find(|n| n.rect.contains(p)).map(|n| n.id)
}

/// Nodes lying completely inside `area`, e.g. a rubber band.
pub fn nodes_in(scene: &Scene, area: Rect) -> Vec<Id> {
    scene.nodes.iter().filter(|n| area.contains_rect(&n.rect)).map(|n| n.id).collect()
}

/// One of the eight resize handles around a selected node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    /// Relative position on the rectangle: (0, 0) top left … (1, 1) bottom right.
    fn factors(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (0.0, 0.0),
            Self::Top => (0.5, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::Right => (1.0, 0.5),
            Self::BottomRight => (1.0, 1.0),
            Self::Bottom => (0.5, 1.0),
            Self::BottomLeft => (0.0, 1.0),
            Self::Left => (0.0, 0.5),
        }
    }

    pub fn position(self, r: Rect) -> Point {
        let (fx, fy) = self.factors();
        Point::new(r.x + r.width * fx, r.y + r.height * fy)
    }

    /// The handle of `r` within `tolerance` of `p`, if any.
    pub fn at(r: Rect, p: Point, tolerance: f32) -> Option<Handle> {
        Self::ALL.into_iter().find(|h| {
            let hp = h.position(r);
            (hp.x - p.x).abs() <= tolerance && (hp.y - p.y).abs() <= tolerance
        })
    }

    /// `r` resized by dragging this handle by (`dx`, `dy`), never smaller than `min`.
    pub fn resize(self, r: Rect, dx: f32, dy: f32, min: f32) -> Rect {
        let (fx, fy) = self.factors();
        let (mut left, mut top, mut right, mut bottom) = (r.x, r.y, r.right(), r.bottom());
        if fx == 0.0 {
            left = (left + dx).min(right - min);
        } else if fx == 1.0 {
            right = (right + dx).max(left + min);
        }
        if fy == 0.0 {
            top = (top + dy).min(bottom - min);
        } else if fy == 1.0 {
            bottom = (bottom + dy).max(top + min);
        }
        Rect::new(left, top, right - left, bottom - top)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

/// New bounds that align `rects` to the first one, like most diagram editors do.
pub fn align(rects: &[Rect], alignment: Alignment) -> Vec<Rect> {
    let Some(reference) = rects.first().copied() else { return Vec::new() };
    rects
        .iter()
        .map(|r| {
            let (x, y) = match alignment {
                Alignment::Left => (reference.x, r.y),
                Alignment::HorizontalCenter => (reference.center().x - r.width / 2.0, r.y),
                Alignment::Right => (reference.right() - r.width, r.y),
                Alignment::Top => (r.x, reference.y),
                Alignment::VerticalCenter => (r.x, reference.center().y - r.height / 2.0),
                Alignment::Bottom => (r.x, reference.bottom() - r.height),
            };
            Rect::new(x, y, r.width, r.height)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use umlcraft_model::sample::order_system;

    #[test]
    fn border_point_leaves_through_the_right_side() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(border_point(r, Point::new(300.0, 25.0)), Point::new(100.0, 25.0));
        assert_eq!(border_point(r, Point::new(50.0, -100.0)), Point::new(50.0, 0.0));
        assert_eq!(border_point(r, Point::new(50.0, 25.0)), Point::new(50.0, 25.0));
    }

    #[test]
    fn sample_class_diagram_scene() {
        let (doc, ids) = order_system();
        let scene = build(doc.model(), ids.class_diagram, &Preview::default());
        assert_eq!(scene.nodes.len(), 3);
        assert_eq!(scene.edges.len(), 2);
        let generalization = scene
            .edges
            .iter()
            .find(|e| e.target_marker == EndMarker::HollowTriangle)
            .expect("generalization edge");
        // Premium (80,260 160x60) → Customer (80,80 160x90): straight up between the boxes.
        assert_eq!(generalization.points, vec![Point::new(160.0, 260.0), Point::new(160.0, 170.0)]);
        let customer = scene
            .nodes
            .iter()
            .find(|n| matches!(&n.figure, Figure::Classifier { name, .. } if name == "Customer"))
            .unwrap();
        let Figure::Classifier { compartments, .. } = &customer.figure else { unreachable!() };
        assert_eq!(compartments[0], vec!["- name : String".to_owned()]);
    }

    #[test]
    fn association_end_labels_sit_above_a_horizontal_line() {
        let (doc, ids) = order_system();
        let scene = build(doc.model(), ids.class_diagram, &Preview::default());
        let association = scene.edges.iter().find(|e| e.labels.len() == 2).unwrap();
        // Order (source, right) → Customer (target, left): the line slopes from y ≈ 128.7 to 126.3.
        let pts = &association.points;
        let [star, role] = [&association.labels[0], &association.labels[1]];
        assert_eq!(star.text, "*");
        assert_eq!(role.text, "customer 1");
        for (label, end) in [(star, pts[0]), (role, pts[pts.len() - 1])] {
            let clearance = end.y - (label.anchor.y + LABEL_HEIGHT);
            assert!(clearance >= 2.5, "{label:?} is only {clearance} above the line");
        }
        // Each label extends from its node into the gap between the classes.
        assert_eq!(role.align, LabelAlign::Left);
        assert!((role.anchor.x - 250.0).abs() < 0.5);
        assert_eq!(star.align, LabelAlign::Right);
        assert!((star.anchor.x - 370.0).abs() < 0.5);
    }

    #[test]
    fn end_labels_avoid_sloped_and_vertical_lines() {
        let text = || "1".to_owned();
        // Line rising to the right of the node: label goes below it.
        let l = end_label(text(), Point::new(0.0, 100.0), Point::new(100.0, 50.0));
        assert_eq!(l.align, LabelAlign::Left);
        assert!(l.anchor.y > 100.0 - 10.0 * 0.45);
        // Vertical line going down: label right of it, starting below the tip.
        let l = end_label(text(), Point::new(50.0, 0.0), Point::new(50.0, 100.0));
        assert_eq!((l.align, l.anchor), (LabelAlign::Left, Point::new(56.0, 10.0)));
        // Vertical line leaning right: label moves to the left side.
        let l = end_label(text(), Point::new(50.0, 0.0), Point::new(90.0, 100.0));
        assert_eq!(l.align, LabelAlign::Right);
        assert!(l.anchor.x < 50.0 + 10.0 * 0.38);
        // Middle labels: centered above horizontal, beside vertical segments.
        let m = middle_label(text(), Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        assert_eq!((m.align, m.anchor), (LabelAlign::Center, Point::new(50.0, -17.0)));
        let m = middle_label(text(), Point::new(0.0, 0.0), Point::new(0.0, 100.0));
        assert_eq!((m.align, m.anchor), (LabelAlign::Left, Point::new(6.0, 43.0)));
    }

    #[test]
    fn dragging_moves_node_and_attached_edges() {
        let (doc, ids) = order_system();
        let node = doc.model().shown_on(ids.premium)[0].1;
        let preview = Preview { moved: HashSet::from([node]), dx: 0.0, dy: 40.0, resized: None };
        let scene = build(doc.model(), ids.class_diagram, &preview);
        let moved = scene.nodes.iter().find(|n| n.id == node).unwrap();
        assert_eq!(moved.rect.y, 300.0);
        assert!(hit_test(&scene, Point::new(100.0, 310.0), 4.0) == Some(node));
        assert!(hit_test(&scene, Point::new(600.0, 600.0), 4.0).is_none());
    }

    #[test]
    fn handles_resize_and_respect_minimum() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert_eq!(Handle::at(r, Point::new(111.0, 59.0), 4.0), Some(Handle::BottomRight));
        assert_eq!(Handle::at(r, Point::new(60.0, 10.0), 4.0), Some(Handle::Top));
        assert_eq!(Handle::at(r, Point::new(60.0, 35.0), 4.0), None);
        assert_eq!(
            Handle::BottomRight.resize(r, 20.0, 10.0, 20.0),
            Rect::new(10.0, 10.0, 120.0, 60.0)
        );
        assert_eq!(Handle::TopLeft.resize(r, -5.0, 100.0, 20.0), Rect::new(5.0, 40.0, 105.0, 20.0));
        assert_eq!(Handle::Top.resize(r, 30.0, 0.0, 20.0), r, "edge handles ignore the other axis");
    }

    #[test]
    fn rubber_band_and_alignment() {
        let (doc, ids) = order_system();
        let scene = build(doc.model(), ids.class_diagram, &Preview::default());
        assert_eq!(nodes_in(&scene, Rect::new(0.0, 0.0, 300.0, 400.0)).len(), 2);
        let rects = [Rect::new(0.0, 0.0, 100.0, 50.0), Rect::new(200.0, 80.0, 60.0, 30.0)];
        assert_eq!(align(&rects, Alignment::Right)[1], Rect::new(40.0, 80.0, 60.0, 30.0));
        assert_eq!(align(&rects, Alignment::VerticalCenter)[1], Rect::new(200.0, 10.0, 60.0, 30.0));
        assert_eq!(align(&rects, Alignment::Top)[0], rects[0]);
    }
}
