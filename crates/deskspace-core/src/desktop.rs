use serde::{Deserialize, Serialize};

use crate::desktop_layer::DesktopLayer;
use crate::geometry::Rect;
use crate::id::{ItemId, SpaceId};
use crate::item::Item;
use crate::layout::GridSpec;
use crate::space::Space;
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Desktop {
    /// Files written before spaces were renamed from "spaces" use `spaces`.
    #[serde(default, alias = "spaces")]
    pub spaces: Vec<Space>,
    /// The desktop's loose items, outside any space (ROADMAP D7).
    #[serde(default)]
    pub layer: DesktopLayer,
    #[serde(default)]
    pub layout: GridSpec,
}

impl Desktop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_space_id(&self) -> SpaceId {
        SpaceId(
            self.spaces
                .iter()
                .map(|space| space.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        )
    }

    pub fn next_item_id(&self) -> ItemId {
        let space_max = self
            .spaces
            .iter()
            .flat_map(|space| space.items.iter())
            .map(|item| item.id.0)
            .max()
            .unwrap_or(0);
        let layer_max = self
            .layer
            .items()
            .iter()
            .map(|item| item.id.0)
            .max()
            .unwrap_or(0);
        ItemId(space_max.max(layer_max) + 1)
    }

    pub fn space(&self, id: SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|space| space.id == id)
    }

    pub fn space_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|space| space.id == id)
    }

    pub fn space_containing(&self, id: ItemId) -> Option<SpaceId> {
        self.spaces
            .iter()
            .find(|space| space.contains_item(id))
            .map(|space| space.id)
    }

    /// The loose desktop layer: the items outside any space.
    pub fn layer(&self) -> &DesktopLayer {
        &self.layer
    }

    pub fn layer_mut(&mut self) -> &mut DesktopLayer {
        &mut self.layer
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.spaces
            .iter()
            .find_map(|space| space.item(id))
            .or_else(|| self.layer.item(id))
    }

    pub fn add_space(&mut self, title: impl Into<String>, rect: Rect) -> SpaceId {
        let id = self.next_space_id();
        self.spaces.push(Space::new(id, title, rect));
        id
    }

    pub fn remove_space(&mut self, id: SpaceId) -> Option<Space> {
        let index = self.spaces.iter().position(|space| space.id == id)?;
        Some(self.spaces.remove(index))
    }

    pub fn add_item(&mut self, space: SpaceId, item: Item) -> bool {
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
    pub fn move_item(&mut self, item: ItemId, target: SpaceId) -> bool {
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

    /// The number of items held by spaces. The desktop layer's loose items are
    /// deliberately excluded: they are the desktop's own, not organized into a
    /// space, so the tray tooltip counts only what DeskSpace manages
    /// (`DesktopLayer::item_count` counts the layer separately).
    pub fn space_item_count(&self) -> usize {
        self.spaces.iter().map(Space::item_count).sum()
    }

    pub fn space_at(&self, point: crate::geometry::Point) -> Option<SpaceId> {
        self.spaces
            .iter()
            .rev()
            .find(|space| space.visible_rect().contains(point))
            .map(|space| space.id)
    }

    pub fn sort_by_id(&mut self) {
        self.spaces.sort_by_key(|space| space.id.0);
    }

    /// Folds every space's per-space overrides from older files into a
    /// customized look (`Space::fold_legacy_overrides`). Returns whether any
    /// space changed, so the caller knows to save.
    pub fn fold_legacy_overrides(&mut self, global: &Theme, system_dark: bool) -> bool {
        let layout = self.layout;
        let mut folded = false;
        for space in &mut self.spaces {
            folded |= space.fold_legacy_overrides(global, &layout, system_dark);
        }
        folded
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
    fn next_item_id_spans_spaces_and_the_layer() {
        let mut desktop = Desktop::new();
        let space = desktop.add_space("A", Rect::ZERO);
        desktop.add_item(space, Item::new(ItemId(5), "C:/missing/a.txt"));
        desktop
            .layer_mut()
            .push_item(Item::new(ItemId(9), "C:/missing/loose.txt"));
        assert_eq!(desktop.next_item_id(), ItemId(10));
        assert!(desktop.item(ItemId(5)).is_some(), "space item is found");
        assert!(desktop.item(ItemId(9)).is_some(), "layer item is found");
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
    fn move_item_to_missing_space_keeps_item() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::ZERO);
        let item = Item::new(desktop.next_item_id(), "file.txt");
        let item_id = item.id;
        desktop.add_item(a, item);
        assert!(!desktop.move_item(item_id, SpaceId(999)));
        assert_eq!(desktop.space_containing(item_id), Some(a));
    }

    #[test]
    fn space_item_count_excludes_the_desktop_layer() {
        let mut desktop = Desktop::new();
        let space = desktop.add_space("A", Rect::ZERO);
        desktop.add_item(space, Item::new(ItemId(1), "in-space.txt"));
        desktop
            .layer_mut()
            .push_item(Item::new(ItemId(2), "loose.txt"));
        assert_eq!(desktop.space_item_count(), 1);
        assert_eq!(desktop.layer().item_count(), 1);
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
