use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Rect, Size};
use crate::space::{DEFAULT_TITLE_HEIGHT, ResizeEdges, Space};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridSpec {
    pub cell_width: i32,
    pub cell_height: i32,
    pub gap: i32,
    pub padding: i32,
    pub title_height: i32,
    pub columns: i32,
    pub snap: i32,
}

impl Default for GridSpec {
    fn default() -> Self {
        Self {
            cell_width: 96,
            cell_height: 104,
            gap: 10,
            padding: 12,
            title_height: DEFAULT_TITLE_HEIGHT,
            columns: 4,
            snap: 16,
        }
    }
}

impl GridSpec {
    pub const fn cell(&self) -> Size {
        Size::new(self.cell_width, self.cell_height)
    }

    pub fn grid_position(&self, index: usize) -> Point {
        self.grid_position_in(index, self.columns)
    }

    pub fn grid_position_in(&self, index: usize, columns: i32) -> Point {
        let columns = columns.max(1) as usize;
        let column = (index % columns) as i32;
        let row = (index / columns) as i32;
        Point::new(
            self.padding + column * (self.cell_width + self.gap),
            self.padding + row * (self.cell_height + self.gap),
        )
    }

    /// How many cell columns fit in a space `width` pixels wide (at least 1).
    pub fn columns_for(&self, width: i32) -> i32 {
        let step = (self.cell_width + self.gap).max(1);
        ((width - self.padding * 2 + self.gap) / step).max(1)
    }

    /// The smallest space a user can resize to: one cell plus padding.
    pub fn min_space_size(&self) -> Size {
        Size::new(
            self.padding * 2 + self.cell_width,
            self.title_height + self.padding * 2 + self.cell_height,
        )
    }

    /// The pixels one mouse-wheel notch scrolls: one row of cells.
    pub fn scroll_step(&self) -> i32 {
        self.cell_height + self.gap
    }
}

/// Moves the `edges` of `rect` to `point`, snapped to `snap`, keeping the
/// opposite edges fixed. The result is at least `min` and stays inside
/// `bounds`.
pub fn resize_rect(
    rect: Rect,
    edges: ResizeEdges,
    point: Point,
    min: Size,
    snap: i32,
    bounds: Rect,
) -> Rect {
    let mut left = rect.left();
    let mut top = rect.top();
    let mut right = rect.right();
    let mut bottom = rect.bottom();
    if edges.left {
        left = snap_to_grid(point.x, snap)
            .max(bounds.left())
            .min(right - min.width);
    }
    if edges.right {
        right = snap_to_grid(point.x, snap)
            .min(bounds.right())
            .max(left + min.width);
    }
    if edges.top {
        top = snap_to_grid(point.y, snap)
            .max(bounds.top())
            .min(bottom - min.height);
    }
    if edges.bottom {
        bottom = snap_to_grid(point.y, snap)
            .min(bounds.bottom())
            .max(top + min.height);
    }
    Rect::new(left, top, right - left, bottom - top)
}

/// Sizes the space to fit its items (unless the user sized it) and lays the
/// items out. Locked spaces are left alone.
pub fn arrange_space(space: &mut Space, spec: GridSpec) {
    if space.locked {
        return;
    }
    if space.user_sized {
        position_items(space, spec);
        return;
    }
    space.title_height = spec.title_height;
    let columns = spec.columns.max(1) as usize;
    let rows = space.items.len().div_ceil(columns).max(1) as i32;
    let width = spec.padding * 2
        + spec.columns.max(1) * spec.cell_width
        + (spec.columns.max(1) - 1) * spec.gap;
    let height =
        spec.title_height + spec.padding * 2 + rows * spec.cell_height + (rows - 1) * spec.gap;
    space.rect.width = width;
    space.rect.height = height;
    position_items(space, spec);
}

/// Places items on the grid. Positions are relative to the space's content
/// area, so moving the space never has to touch its items. User-sized spaces
/// use as many columns as their width allows; others use `spec.columns`.
pub fn position_items(space: &mut Space, spec: GridSpec) {
    let columns = if space.user_sized {
        spec.columns_for(space.rect.width)
    } else {
        spec.columns
    };
    for (index, item) in space.items.iter_mut().enumerate() {
        item.position = spec.grid_position_in(index, columns);
    }
}

pub fn snap_to_grid(value: i32, grid: i32) -> i32 {
    if grid <= 1 {
        return value;
    }
    (value as f32 / grid as f32).round() as i32 * grid
}

pub fn snap_rect(rect: Rect, grid: i32) -> Rect {
    if grid <= 1 {
        return rect;
    }
    Rect::new(
        snap_to_grid(rect.x, grid),
        snap_to_grid(rect.y, grid),
        rect.width,
        rect.height,
    )
}

pub fn clamp_to_bounds(rect: Rect, bounds: Rect) -> Rect {
    let max_x = (bounds.right() - rect.width).max(bounds.x);
    let max_y = (bounds.bottom() - rect.height).max(bounds.y);
    Rect::new(
        rect.x.clamp(bounds.x, max_x),
        rect.y.clamp(bounds.y, max_y),
        rect.width,
        rect.height,
    )
}

pub fn resolve_collisions(spaces: &mut [Space], gap: i32, bounds: Rect) {
    let mut placed: Vec<Rect> = Vec::with_capacity(spaces.len());
    for space in spaces.iter_mut() {
        if space.locked {
            placed.push(space.rect);
            continue;
        }
        let mut candidate = space.rect;
        let mut attempts = 0;
        while placed.iter().any(|other| other.intersects(candidate)) && attempts < 64 {
            candidate.y += candidate.height + gap;
            candidate = clamp_to_bounds(candidate, bounds);
            attempts += 1;
        }
        space.rect = candidate;
        placed.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{ItemId, SpaceId};
    use crate::item::Item;

    fn space_with_items(count: usize) -> Space {
        let mut space = Space::new(SpaceId(1), "Test", Rect::new(0, 0, 10, 10));
        for index in 0..count {
            space.push_item(Item::new(ItemId(index as u64 + 1), format!("item{index}")));
        }
        space
    }

    #[test]
    fn arrange_positions_items_inside_content() {
        let spec = GridSpec::default();
        let mut space = space_with_items(5);
        space.rect.x = 100;
        space.rect.y = 200;
        arrange_space(&mut space, spec);

        let content = space.content_rect();
        let local_content = Rect::new(0, 0, content.width, content.height);
        assert_eq!(space.items.len(), 5);
        for item in &space.items {
            assert!(local_content.contains(item.position));
        }
    }

    #[test]
    fn moving_a_space_keeps_item_positions() {
        let spec = GridSpec::default();
        let mut space = space_with_items(3);
        arrange_space(&mut space, spec);
        let before: Vec<Point> = space.items.iter().map(|item| item.position).collect();
        space.set_position(Point::new(500, 400));
        let after: Vec<Point> = space.items.iter().map(|item| item.position).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn snap_rounds_to_nearest_multiple() {
        assert_eq!(snap_to_grid(23, 16), 16);
        assert_eq!(snap_to_grid(25, 16), 32);
        assert_eq!(snap_to_grid(7, 0), 7);
    }

    #[test]
    fn clamp_keeps_rect_inside_bounds() {
        let bounds = Rect::new(0, 0, 800, 600);
        let rect = Rect::new(900, 700, 100, 100);
        let clamped = clamp_to_bounds(rect, bounds);
        assert_eq!(clamped, Rect::new(700, 500, 100, 100));
    }

    #[test]
    fn columns_follow_width() {
        let spec = GridSpec::default();
        // 12 + 96 + 12 fits one column; each further column needs 106.
        assert_eq!(spec.columns_for(120), 1);
        assert_eq!(spec.columns_for(225), 1);
        assert_eq!(spec.columns_for(226), 2);
        assert_eq!(spec.columns_for(0), 1);
    }

    #[test]
    fn user_sized_spaces_keep_size_and_reflow() {
        let spec = GridSpec::default();
        let mut space = space_with_items(5);
        space.user_sized = true;
        space.rect = Rect::new(0, 0, 226, 150);
        arrange_space(&mut space, spec);
        assert_eq!(space.rect, Rect::new(0, 0, 226, 150));
        assert_eq!(space.items[2].position, Point::new(12, 12 + 104 + 10));
        assert_eq!(space.items[1].position.y, space.items[0].position.y);
    }

    #[test]
    fn resize_moves_only_grabbed_edges_and_respects_minimum() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let min = Size::new(120, 160);
        let rect = Rect::new(400, 400, 400, 300);
        let bottom_right = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        let grown = resize_rect(rect, bottom_right, Point::new(1001, 899), min, 16, bounds);
        assert_eq!(grown, Rect::new(400, 400, 608, 496));

        let left = ResizeEdges {
            left: true,
            ..ResizeEdges::NONE
        };
        let squeezed = resize_rect(rect, left, Point::new(790, 500), min, 16, bounds);
        assert_eq!(squeezed, Rect::new(680, 400, 120, 300));

        let clamped = resize_rect(rect, bottom_right, Point::new(5000, 5000), min, 16, bounds);
        assert_eq!((clamped.right(), clamped.bottom()), (1920, 1080));
    }

    #[test]
    fn collisions_push_spaces_down() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let mut spaces = vec![
            Space::new(SpaceId(1), "A", Rect::new(0, 0, 200, 200)),
            Space::new(SpaceId(2), "B", Rect::new(0, 0, 200, 200)),
        ];
        resolve_collisions(&mut spaces, 10, bounds);
        assert!(!spaces[0].rect.intersects(spaces[1].rect));
    }
}
