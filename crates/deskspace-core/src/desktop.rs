use serde::{Deserialize, Serialize};

use crate::space::space;
use crate::geometry::Rect;
use crate::id::{spaceId, ItemId};
use crate::item::Item;
use crate::layout::GridSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Desktop {
    #[serde(default)]
    pub spaces: Vec<space>,
    #[serde(default)]
    pub layout: GridSpec,
}

impl Desktop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_space_id(&self) -> spaceId {
        spaceId(
            self.spaces
                .iter()
                .map(|space| space.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        )
    }

    pub fn next_item_id(&self) -> ItemId {
        ItemId(
            self.spaces
                .iter()
                .flat_map(|space| space.items.iter())
                .map(|item| item.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        )
    }

    pub fn space(&self, id: spaceId) -> Option<&space> {
        self.spaces.iter().find(|space| space.id == id)
    }

    pub fn space_mut(&mut self, id: spaceId) -> Option<&mut space> {
        self.spaces.iter_mut().find(|space| space.id == id)
    }

    pub fn space_containing(&self, id: ItemId) -> Option<spaceId> {
        self.spaces
            .iter()
            .find(|space| space.contains_item(id))
            .map(|space| space.id)
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.spaces.iter().find_map(|space| space.item(id))
    }

    /// The item referring to `path`, in whichever space holds it.
    pub fn item_with_path(&self, path: &std::path::Path) -> Option<ItemId> {
        self.spaces
            .iter()
            .flat_map(|space| space.items.iter())
            .find(|item| item.path == path)
            .map(|item| item.id)
    }

    pub fn add_space(&mut self, title: impl Into<String>, rect: Rect) -> spaceId {
        let id = self.next_space_id();
        self.spaces.push(space::new(id, title, rect));
        id
    }

    pub fn remove_space(&mut self, id: spaceId) -> Option<space> {
        let index = self.spaces.iter().position(|space| space.id == id)?;
        Some(self.spaces.remove(index))
    }

    pub fn add_item(&mut self, space: spaceId, item: Item) -> bool {
        match self.space_mut(space) {
            Some(target) => {
                target.push_item(item);
                true
            }
            None => false,
        }
    }

    pub fn remove_item(&mut self, item: ItemId) -> Option<Item> {
        for space in &mut self.spaces {
            if space.contains_item(item) {
                return space.remove_item(item);
            }
        }
        None
    }

    /// Moves `item` into `target`. Returns `false` without changing anything if
    /// the item or the target space does not exist, or the item is already there.
    pub fn move_item(&mut self, item: ItemId, target: spaceId) -> bool {
        let Some(target_index) = self.spaces.iter().position(|space| space.id == target) else {
            return false;
        };
        if self.spaces[target_index].contains_item(item) {
            return false;
        }
        let Some(mut moved) = self.remove_item(item) else {
            return false;
        };
        moved.position = crate::geometry::Point::ZERO;
        self.spaces[target_index].push_item(moved);
        true
    }

    pub fn item_count(&self) -> usize {
        self.spaces.iter().map(space::item_count).sum()
    }

    pub fn space_at(&self, point: crate::geometry::Point) -> Option<spaceId> {
        self.spaces
            .iter()
            .rev()
            .find(|space| space.visible_rect().contains(point))
            .map(|space| space.id)
    }

    pub fn sort_by_id(&mut self) {
        self.spaces.sort_by_key(|space| space.id.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Item;

    #[test]
    fn ids_increment_across_spaces() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let b = desktop.add_space("B", Rect::new(200, 0, 100, 100));
        assert_ne!(a, b);
        assert_eq!(desktop.spaces.len(), 2);
    }

    #[test]
    fn move_item_transfers_between_spaces() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::ZERO);
        let b = desktop.add_space("B", Rect::ZERO);
        let item = Item::new(desktop.next_item_id(), "file.txt");
        let item_id = item.id;
        desktop.add_item(a, item);
        assert_eq!(desktop.space_containing(item_id), Some(a));
        assert!(desktop.move_item(item_id, b));
        assert_eq!(desktop.space_containing(item_id), Some(b));
    }

    #[test]
    fn finds_items_by_path() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::ZERO);
        let item = Item::new(desktop.next_item_id(), "C:/x/file.txt");
        let id = item.id;
        desktop.add_item(a, item);
        assert_eq!(
            desktop.item_with_path(std::path::Path::new("C:/x/file.txt")),
            Some(id)
        );
        assert_eq!(desktop.item_with_path(std::path::Path::new("C:/y")), None);
    }

    #[test]
    fn move_item_to_missing_space_keeps_item() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::ZERO);
        let item = Item::new(desktop.next_item_id(), "file.txt");
        let item_id = item.id;
        desktop.add_item(a, item);
        assert!(!desktop.move_item(item_id, spaceId(999)));
        assert_eq!(desktop.space_containing(item_id), Some(a));
    }

    #[test]
    fn rolled_up_space_is_only_hit_on_its_title_bar() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 200, 200));
        desktop.space_mut(id).unwrap().rolled_up = true;
        assert_eq!(
            desktop.space_at(crate::geometry::Point::new(10, 10)),
            Some(id)
        );
        assert_eq!(desktop.space_at(crate::geometry::Point::new(10, 100)), None);
    }
}
