use serde::{Deserialize, Serialize};

use crate::space::{DEFAULT_TITLE_HEIGHT, space};
use crate::geometry::{Point, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        let columns = self.columns.max(1) as usize;
        let column = (index % columns) as i32;
        let row = (index / columns) as i32;
        Point::new(
            self.padding + column * (self.cell_width + self.gap),
            self.padding + row * (self.cell_height + self.gap),
        )
    }
}

pub fn arrange_space(space: &mut space, spec: GridSpec) {
    if space.locked {
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

    let content = space.content_rect();
    for (index, item) in space.items.iter_mut().enumerate() {
        let local = spec.grid_position(index);
        item.position = Point::new(content.x + local.x, content.y + local.y);
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

pub fn resolve_collisions(spaces: &mut [space], gap: i32, bounds: Rect) {
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
    use crate::id::{spaceId, ItemId};
    use crate::item::Item;

    fn space_with_items(count: usize) -> space {
        let mut space = space::new(spaceId(1), "Test", Rect::new(0, 0, 10, 10));
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
        assert_eq!(space.items.len(), 5);
        for item in &space.items {
            assert!(content.contains(item.position));
        }
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
    fn collisions_push_spaces_down() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let mut spaces = vec![
            space::new(spaceId(1), "A", Rect::new(0, 0, 200, 200)),
            space::new(spaceId(2), "B", Rect::new(0, 0, 200, 200)),
        ];
        resolve_collisions(&mut spaces, 10, bounds);
        assert!(!spaces[0].rect.intersects(spaces[1].rect));
    }
}
