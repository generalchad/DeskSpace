use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Rect, Size};
use crate::id::ItemId;
use crate::item::Item;

/// The desktop's loose items: the Desktop-folder files no space holds. It has
/// no chrome (no title bar, border, lock, resize, roll-up, or sort) and is
/// drawn on a chrome-less desktop-hosted window behind the spaces (ROADMAP D7).
///
/// The shell is the source of truth for membership and positions; the layer is
/// a refreshed cache (`Command::SyncDesktop`), so its ids are reconciled by
/// path across refreshes. Unlike a space's items, whose `position` is relative
/// to the space's content area, a layer item's `position` is absolute, in
/// virtual-desktop coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DesktopLayer {
    items: Vec<Item>,
}

impl DesktopLayer {
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    /// The item whose cell contains `point` (virtual-desktop coordinates).
    pub fn item_at(&self, point: Point, cell: Size) -> Option<ItemId> {
        self.items
            .iter()
            .find(|item| Rect::from_point_size(item.position, cell).contains(point))
            .map(|item| item.id)
    }

    pub fn push_item(&mut self, item: Item) {
        if !self.items.iter().any(|entry| entry.id == item.id) {
            self.items.push(item);
        }
    }

    pub fn replace_items(&mut self, items: Vec<Item>) {
        self.items = items;
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemFacts;

    #[test]
    fn item_at_hits_absolute_cells() {
        let mut layer = DesktopLayer::default();
        let mut item = Item::new(ItemId(1), "C:/missing/a.txt", ItemFacts::default());
        item.position = Point::new(40, 60);
        layer.push_item(item);
        let cell = Size::new(96, 104);
        assert_eq!(layer.item_at(Point::new(50, 70), cell), Some(ItemId(1)));
        assert_eq!(layer.item_at(Point::new(200, 200), cell), None);
    }

    #[test]
    fn push_item_ignores_a_duplicate_id() {
        let mut layer = DesktopLayer::default();
        layer.push_item(Item::new(
            ItemId(1),
            "C:/missing/a.txt",
            ItemFacts::default(),
        ));
        layer.push_item(Item::new(
            ItemId(1),
            "C:/missing/b.txt",
            ItemFacts::default(),
        ));
        assert_eq!(layer.item_count(), 1);
    }
}
