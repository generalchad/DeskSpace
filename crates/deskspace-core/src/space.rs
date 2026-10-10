use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Rect, Size};
use crate::id::{ItemId, SpaceId};
use crate::item::Item;
use crate::theme::Color;

pub const DEFAULT_TITLE_HEIGHT: i32 = 28;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub title: String,
    pub rect: Rect,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub rolled_up: bool,
    /// Per-space multiplier on the theme's tint alpha; 255 keeps the theme's.
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    /// Per-space override of the body tint color (its alpha comes from the
    /// theme). `None` uses the theme's tint.
    #[serde(default)]
    pub tint: Option<Color>,
    /// Per-space override of the title and label text color. `None` uses the
    /// theme's.
    #[serde(default)]
    pub text: Option<Color>,
    /// Per-space icon size as a percent; `None` follows the global setting.
    #[serde(default)]
    pub icon_size: Option<u8>,
    #[serde(default = "default_title_height")]
    pub title_height: i32,
    /// Set once the user resizes the space. A user-sized space keeps its
    /// size, flows its items into as many columns as its width allows, and
    /// scrolls when they do not fit. Otherwise it grows to fit its items.
    #[serde(default)]
    pub user_sized: bool,
}

/// Which edges of a space a resize gesture moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResizeEdges {
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
}

impl ResizeEdges {
    pub const NONE: Self = Self {
        left: false,
        top: false,
        right: false,
        bottom: false,
    };

    pub const fn any(self) -> bool {
        self.left || self.top || self.right || self.bottom
    }
}

fn default_opacity() -> u8 {
    255
}

fn default_title_height() -> i32 {
    DEFAULT_TITLE_HEIGHT
}

impl Space {
    pub fn new(id: SpaceId, title: impl Into<String>, rect: Rect) -> Self {
        Self {
            id,
            title: title.into(),
            rect,
            items: Vec::new(),
            locked: false,
            rolled_up: false,
            opacity: default_opacity(),
            tint: None,
            text: None,
            icon_size: None,
            title_height: default_title_height(),
            user_sized: false,
        }
    }

    pub fn title_bar(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.width, self.title_height)
    }

    /// The area the space occupies on screen: only the title bar while rolled up.
    pub fn visible_rect(&self) -> Rect {
        if self.rolled_up {
            self.title_bar()
        } else {
            self.rect
        }
    }

    pub fn content_rect(&self) -> Rect {
        if self.rolled_up {
            return Rect::new(
                self.rect.x,
                self.rect.y + self.title_height,
                self.rect.width,
                0,
            );
        }
        Rect::new(
            self.rect.x,
            self.rect.y + self.title_height,
            self.rect.width,
            (self.rect.height - self.title_height).max(0),
        )
    }

    /// The item whose grid cell contains `point` (desktop coordinates) when
    /// the content is scrolled down by `scroll` pixels.
    pub fn item_at(&self, point: Point, cell: Size, scroll: i32) -> Option<ItemId> {
        let content = self.content_rect();
        if !content.contains(point) {
            return None;
        }
        let local = Point::new(point.x - content.x, point.y - content.y + scroll);
        self.items
            .iter()
            .find(|item| Rect::from_point_size(item.position, cell).contains(local))
            .map(|item| item.id)
    }

    /// Height of the laid-out items including the bottom padding.
    pub fn content_extent(&self, cell: Size, padding: i32) -> i32 {
        self.items
            .iter()
            .map(|item| item.position.y + cell.height + padding)
            .max()
            .unwrap_or(0)
    }

    /// How far the content can scroll before its end reaches the bottom.
    pub fn max_scroll(&self, cell: Size, padding: i32) -> i32 {
        (self.content_extent(cell, padding) - self.content_rect().height).max(0)
    }

    /// The edges a resize grabbed at `point` would move. Corners are wider
    /// than `border` so they are easy to hit. Locked and rolled-up spaces
    /// cannot be resized.
    pub fn resize_edges_at(&self, point: Point, border: i32) -> ResizeEdges {
        if self.locked || self.rolled_up || !self.rect.contains(point) {
            return ResizeEdges::NONE;
        }
        let corner = border * 2;
        let from_left = point.x - self.rect.x;
        let from_right = self.rect.right() - 1 - point.x;
        let from_top = point.y - self.rect.y;
        let from_bottom = self.rect.bottom() - 1 - point.y;
        let near_vertical_edge = from_left < corner || from_right < corner;
        let near_horizontal_edge = from_top < corner || from_bottom < corner;
        ResizeEdges {
            left: from_left < border || (from_left < corner && near_horizontal_edge),
            right: from_right < border || (from_right < corner && near_horizontal_edge),
            top: from_top < border || (from_top < corner && near_vertical_edge),
            bottom: from_bottom < border || (from_bottom < corner && near_vertical_edge),
        }
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn contains_item(&self, id: ItemId) -> bool {
        self.items.iter().any(|item| item.id == id)
    }

    pub fn push_item(&mut self, item: Item) {
        if !self.contains_item(item.id) {
            self.items.push(item);
        }
    }

    pub fn remove_item(&mut self, id: ItemId) -> Option<Item> {
        let index = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(index))
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn set_position(&mut self, origin: Point) {
        self.rect = self.rect.with_position(origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolled_up_space_collapses_to_title_bar() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(10, 20, 200, 300));
        assert_eq!(space.visible_rect(), space.rect);
        space.rolled_up = true;
        assert_eq!(
            space.visible_rect(),
            Rect::new(10, 20, 200, DEFAULT_TITLE_HEIGHT)
        );
        assert!(space.content_rect().is_empty());
    }

    #[test]
    fn item_at_uses_content_relative_cells() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(100, 100, 300, 300));
        let mut item = Item::new(ItemId(7), "a.txt");
        item.position = Point::new(10, 10);
        space.push_item(item);
        let cell = Size::new(50, 50);
        let inside = Point::new(100 + 20, 100 + DEFAULT_TITLE_HEIGHT + 20);
        assert_eq!(space.item_at(inside, cell, 0), Some(ItemId(7)));
        assert_eq!(space.item_at(Point::new(100 + 200, 300), cell, 0), None);
        space.rolled_up = true;
        assert_eq!(space.item_at(inside, cell, 0), None);
    }

    #[test]
    fn item_at_accounts_for_scroll() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 300, 100));
        let mut item = Item::new(ItemId(3), "deep.txt");
        item.position = Point::new(10, 200);
        space.push_item(item);
        let cell = Size::new(50, 50);
        let point = Point::new(20, DEFAULT_TITLE_HEIGHT + 20);
        assert_eq!(space.item_at(point, cell, 0), None);
        assert_eq!(space.item_at(point, cell, 190), Some(ItemId(3)));
    }

    #[test]
    fn max_scroll_is_overflow_beyond_the_content_area() {
        let mut space = Space::new(SpaceId(1), "F", Rect::new(0, 0, 300, 128));
        assert_eq!(space.max_scroll(Size::new(50, 50), 10), 0);
        let mut item = Item::new(ItemId(1), "a");
        item.position = Point::new(10, 200);
        space.push_item(item);
        // Extent 200 + 50 + 10 = 260; content height 128 - 28 = 100.
        assert_eq!(space.max_scroll(Size::new(50, 50), 10), 160);
    }

    #[test]
    fn resize_edges_cover_borders_and_wider_corners() {
        let space = Space::new(SpaceId(1), "F", Rect::new(100, 100, 200, 200));
        let edges = |x, y| space.resize_edges_at(Point::new(x, y), 6);
        assert_eq!(edges(200, 200), ResizeEdges::NONE);
        assert!(edges(101, 200).left && !edges(101, 200).top);
        assert!(edges(299, 200).right);
        assert!(edges(200, 299).bottom);
        let corner = edges(290, 297);
        assert!(corner.right && corner.bottom && !corner.left && !corner.top);
        let mut locked = space.clone();
        locked.locked = true;
        assert!(!locked.resize_edges_at(Point::new(101, 200), 6).any());
    }
}
