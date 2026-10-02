use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn translate(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const ZERO: Self = Self {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_point_size(origin: Point, size: Size) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            width: size.width,
            height: size.height,
        }
    }

    pub const fn left(self) -> i32 {
        self.x
    }

    pub const fn top(self) -> i32 {
        self.y
    }

    pub const fn right(self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub const fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    pub const fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    pub const fn center(self) -> Point {
        Point::new(self.x + self.width / 2, self.y + self.height / 2)
    }

    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub const fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.y >= self.y && point.x < self.right() && point.y < self.bottom()
    }

    pub fn intersects(self, other: Self) -> bool {
        !(other.x >= self.right()
            || other.right() <= self.x
            || other.y >= self.bottom()
            || other.bottom() <= self.y)
    }

    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, right - x, bottom - y)
    }

    pub const fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    pub const fn with_position(self, origin: Point) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            width: self.width,
            height: self.height,
        }
    }

    pub const fn with_size(self, size: Size) -> Self {
        Self {
            x: self.x,
            y: self.y,
            width: size.width,
            height: size.height,
        }
    }

    pub fn inset(self, amount: i32) -> Self {
        Self::new(
            self.x + amount,
            self.y + amount,
            (self.width - amount * 2).max(0),
            (self.height - amount * 2).max(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_respects_exclusive_bounds() {
        let rect = Rect::new(10, 10, 100, 50);
        assert!(rect.contains(Point::new(10, 10)));
        assert!(rect.contains(Point::new(109, 59)));
        assert!(!rect.contains(Point::new(110, 30)));
        assert!(!rect.contains(Point::new(30, 60)));
    }

    #[test]
    fn intersection_is_symmetric() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert!(a.intersects(b));
        assert!(b.intersects(a));
        assert!(!a.intersects(Rect::new(20, 20, 5, 5)));
    }

    #[test]
    fn union_grows_to_cover_both() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(20, 30, 10, 10);
        let u = a.union(b);
        assert_eq!(u, Rect::new(0, 0, 30, 40));
    }
}
