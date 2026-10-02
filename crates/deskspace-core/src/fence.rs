use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Rect, Size};
use crate::id::{spaceId, ItemId};
use crate::item::Item;

pub const DEFAULT_TITLE_HEIGHT: i32 = 28;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct space {
    pub id: spaceId,
    pub title: String,
    pub rect: Rect,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub rolled_up: bool,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default = "default_title_height")]
    pub title_height: i32,
}

fn default_opacity() -> u8 {
    210
}

fn default_title_height() -> i32 {
    DEFAULT_TITLE_HEIGHT
}

impl space {
    pub fn new(id: spaceId, title: impl Into<String>, rect: Rect) -> Self {
        Self {
            id,
            title: title.into(),
            rect,
            items: Vec::new(),
            locked: false,
            rolled_up: false,
            opacity: default_opacity(),
            title_height: default_title_height(),
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

    pub fn required_size(&self, cell: Size, gap: i32, padding: i32, columns: i32) -> Size {
        let columns = columns.max(1);
        let rows = self.items.len().div_ceil(columns as usize).max(1) as i32;
        Size::new(
            padding * 2 + columns * cell.width + (columns - 1) * gap,
            self.title_height + padding * 2 + rows * cell.height + (rows - 1) * gap,
        )
    }

    pub fn resize_to_fit(&mut self, cell: Size, gap: i32, padding: i32, columns: i32) {
        let size = self.required_size(cell, gap, padding, columns);
        self.rect.width = size.width;
        self.rect.height = size.height;
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
        let mut space = space::new(spaceId(1), "F", Rect::new(10, 20, 200, 300));
        assert_eq!(space.visible_rect(), space.rect);
        space.rolled_up = true;
        assert_eq!(
            space.visible_rect(),
            Rect::new(10, 20, 200, DEFAULT_TITLE_HEIGHT)
        );
        assert!(space.content_rect().is_empty());
    }
}
