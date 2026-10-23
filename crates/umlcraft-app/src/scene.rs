//! Turns a diagram view into simple drawable shapes in logical coordinates.
//!
//! This is the seed of the planned `umlcraft-notation` crate: it knows UML notation but nothing
//! about GPUI, so it can move out once the class, use case and sequence notations grow.

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
    pub label: Option<String>,
    pub source_label: Option<String>,
    pub target_label: Option<String>,
}

/// Builds the scene. `moved` previews a node being dragged: (presentation, dx, dy).
pub fn build(model: &Model, diagram: Id, moved: Option<(Id, f32, f32)>) -> Scene {
    let Some(view) = model.view(diagram) else {
        return Scene { nodes: Vec::new(), edges: Vec::new() };
    };
    let rect_of = |p: &Presentation| {
        p.bounds().map(|r| match moved {
            Some((id, dx, dy)) if id == p.id => r.translate(dx, dy),
            _ => r,
        })
    };
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
        label: None,
        source_label: None,
        target_label: None,
    };
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
            edge.source_label = end_label(&a.role, a.multiplicity);
            edge.target_label = end_label(&b.role, b.multiplicity);
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
            edge.label = Some("«include»".into());
        }
        Some(ElementKind::Extend(_)) => {
            edge.line = LineStyle::Dashed;
            edge.target_marker = EndMarker::OpenArrow;
            edge.label = Some("«extend»".into());
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
    edge
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
        let scene = build(doc.model(), ids.class_diagram, None);
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
    fn dragging_moves_node_and_attached_edges() {
        let (doc, ids) = order_system();
        let node = doc.model().shown_on(ids.premium)[0].1;
        let scene = build(doc.model(), ids.class_diagram, Some((node, 0.0, 40.0)));
        let moved = scene.nodes.iter().find(|n| n.id == node).unwrap();
        assert_eq!(moved.rect.y, 300.0);
        assert!(hit_test(&scene, Point::new(100.0, 310.0), 4.0) == Some(node));
        assert!(hit_test(&scene, Point::new(600.0, 600.0), 4.0).is_none());
    }
}
