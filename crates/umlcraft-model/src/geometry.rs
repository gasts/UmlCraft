//! Diagram geometry in logical units (1.0 = one pixel at 100 % zoom).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn distance(self, other: Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.width, size.height)
    }

    /// Normalized rectangle spanned by two corner points, e.g. a rubber band.
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs())
    }

    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.bottom()
            && other.y <= self.bottom()
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub fn inflate(&self, d: f32) -> Self {
        Self::new(self.x - d, self.y - d, self.width + 2.0 * d, self.height + 2.0 * d)
    }

    pub fn union(&self, other: &Rect) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_basics() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(Point::new(10.0, 20.0)));
        assert!(r.contains(Point::new(110.0, 70.0)));
        assert!(!r.contains(Point::new(111.0, 70.0)));
        assert_eq!(r.center(), Point::new(60.0, 45.0));
        assert_eq!(
            Rect::from_corners(Point::new(5.0, 9.0), Point::new(1.0, 2.0)),
            Rect::new(1.0, 2.0, 4.0, 7.0)
        );
        assert_eq!(r.union(&Rect::new(0.0, 0.0, 5.0, 5.0)), Rect::new(0.0, 0.0, 110.0, 70.0));
        assert!(r.intersects(&Rect::new(100.0, 60.0, 50.0, 50.0)));
        assert!(!r.intersects(&Rect::new(200.0, 60.0, 50.0, 50.0)));
    }
}
