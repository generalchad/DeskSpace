use serde::{Deserialize, Serialize};

use crate::desktop_layer::DesktopLayer;
use crate::geometry::Rect;
use crate::id::{ItemId, SpaceId};
use crate::item::Item;
use crate::layout::{GridSpec, position_items};
use crate::space::{Space, SpaceLook};
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Desktop {
    #[serde(default)]
    pub(crate) spaces: Vec<Space>,
    /// The desktop's loose items, outside any space (ROADMAP D7).
    #[serde(default)]
    pub(crate) layer: DesktopLayer,
    /// The global grid. It is a setting, not model state, so it is public
    /// rather than behind a `Command`: the settings window edits, previews,
    /// and cancels it as it does the global `Theme`, it is not undoable, and
    /// every `Command` is applied against a layout passed in
    /// (`apply_with_layout`) rather than reading it.
    #[serde(default)]
    pub layout: GridSpec,
    /// The highest space id handed out this session.
    #[serde(skip)]
    space_high_water: HighWater,
    /// The highest item id handed out this session, in a space or the layer.
    #[serde(skip)]
    item_high_water: HighWater,
}

/// The highest id handed out this session, so an id freed by a delete is not
/// given to a newer space or item, and an undo can put the old one back
/// (ROADMAP D11). It is bookkeeping, not model state: it is never saved, it
/// starts at 0 after a load (the ids present cover what came before), and it
/// never makes two desktops unequal.
#[derive(Debug, Clone, Copy, Default)]
struct HighWater(u64);

impl PartialEq for HighWater {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for HighWater {}

impl Desktop {
    pub fn new() -> Self {
        Self::default()
    }

    /// The id the next new space gets: above every id present and every id
    /// handed out this session. A peek; `take_space_id` hands it out.
    pub fn next_space_id(&self) -> SpaceId {
        let present = self
            .spaces
            .iter()
            .map(|space| space.id.0)
            .max()
            .unwrap_or(0);
        SpaceId(present.max(self.space_high_water.0) + 1)
    }

    /// The id the next new item gets, in a space or the layer: above every id
    /// present and every id handed out this session. A peek; `take_item_id`
    /// hands it out.
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
        ItemId(space_max.max(layer_max).max(self.item_high_water.0) + 1)
    }

    pub(crate) fn take_space_id(&mut self) -> SpaceId {
        let id = self.next_space_id();
        self.space_high_water.0 = id.0;
        id
    }

    pub(crate) fn take_item_id(&mut self) -> ItemId {
        let id = self.next_item_id();
        self.item_high_water.0 = id.0;
        id
    }

    /// Raises the high-water marks to cover `space` and its items, which came
    /// from outside the allocator (a restore), so deleting them again does
    /// not free their ids.
    pub(crate) fn note_ids(&mut self, space: &Space) {
        self.space_high_water.0 = self.space_high_water.0.max(space.id.0);
        let items = space.items.iter().map(|item| item.id.0);
        self.item_high_water.0 = items.fold(self.item_high_water.0, u64::max);
    }

    pub fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    pub fn space(&self, id: SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|space| space.id == id)
    }

    pub(crate) fn space_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
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

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.spaces
            .iter()
            .find_map(|space| space.item(id))
            .or_else(|| self.layer.item(id))
    }

    pub(crate) fn add_space(&mut self, title: impl Into<String>, rect: Rect) -> SpaceId {
        let id = self.take_space_id();
        self.spaces.push(Space::new(id, title, rect));
        id
    }

    pub(crate) fn remove_space(&mut self, id: SpaceId) -> Option<Space> {
        let index = self.spaces.iter().position(|space| space.id == id)?;
        Some(self.spaces.remove(index))
    }

    pub(crate) fn add_item(&mut self, space: SpaceId, item: Item) -> bool {
        match self.space_mut(space) {
            Some(target) => {
                target.push_item(item);
                true
            }
            None => false,
        }
    }

    pub(crate) fn remove_item(&mut self, item: ItemId) -> Option<Item> {
        for space in &mut self.spaces {
            if space.contains_item(item) {
                return space.remove_item(item);
            }
        }
        None
    }

    /// Moves `item` into `target`. Returns `false` without changing anything if
    /// the item or the target space does not exist, or the item is already there.
    pub(crate) fn move_item(&mut self, item: ItemId, target: SpaceId) -> bool {
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

    /// Folds every space's per-space overrides from older files into a
    /// customized look (`Space::merge_legacy_overrides`). Returns whether any
    /// space changed, so the caller knows to save.
    pub fn merge_legacy_overrides(&mut self, global: &Theme, system_dark: bool) -> bool {
        let layout = self.layout;
        let mut merged = false;
        for space in &mut self.spaces {
            merged |= space.merge_legacy_overrides(global, &layout, system_dark);
        }
        merged
    }

    /// Resets every space loaded from a v1 file. v1 stored 210 as the fill
    /// alpha, which no later setting means, so it is dropped rather than
    /// merged into a custom look. v1 also stored absolute item positions that
    /// went stale when the space moved; positions are now space-relative grid
    /// cells, so the items are placed again.
    pub fn reset_v1_spaces(&mut self) {
        let layout = self.layout;
        for space in &mut self.spaces {
            space.clear_legacy_overrides();
            position_items(space, layout);
        }
    }

    /// Clamps every space's own look, so a hand-edited out-of-range setting
    /// cannot leave the settings window showing no choice selected.
    pub fn clamp_looks(&mut self) {
        for space in &mut self.spaces {
            space.look = space.look.take().map(SpaceLook::clamped);
        }
    }

    /// Drops items a hand-edited file listed for a lens. Lens items are a live
    /// view of the folder and are never persisted (M4), so the next sync fills
    /// them again.
    pub fn clear_lens_items(&mut self) {
        for space in &mut self.spaces {
            if space.is_lens() {
                space.items.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Item;
    use crate::item::ItemFacts;
    use crate::layout::GAP_RANGE;

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
        desktop.add_item(
            space,
            Item::new(ItemId(5), "C:/missing/a.txt", ItemFacts::default()),
        );
        desktop.layer.push_item(Item::new(
            ItemId(9),
            "C:/missing/loose.txt",
            ItemFacts::default(),
        ));
        assert_eq!(desktop.next_item_id(), ItemId(10));
        assert!(desktop.item(ItemId(5)).is_some(), "space item is found");
        assert!(desktop.item(ItemId(9)).is_some(), "layer item is found");
    }

    #[test]
    fn handed_out_ids_do_not_make_desktops_unequal() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::ZERO);
        let saved = desktop.clone();
        desktop.remove_space(id);
        desktop.spaces.push(saved.spaces[0].clone());
        let reloaded = Desktop {
            spaces: saved.spaces.clone(),
            ..Desktop::default()
        };
        assert_eq!(desktop, saved);
        assert_eq!(reloaded, saved);
    }

    #[test]
    fn move_item_transfers_between_spaces() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::ZERO);
        let b = desktop.add_space("B", Rect::ZERO);
        let item = Item::new(desktop.next_item_id(), "file.txt", ItemFacts::default());
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
        let item = Item::new(desktop.next_item_id(), "file.txt", ItemFacts::default());
        let item_id = item.id;
        desktop.add_item(a, item);
        assert!(!desktop.move_item(item_id, SpaceId(999)));
        assert_eq!(desktop.space_containing(item_id), Some(a));
    }

    #[test]
    fn space_item_count_excludes_the_desktop_layer() {
        let mut desktop = Desktop::new();
        let space = desktop.add_space("A", Rect::ZERO);
        desktop.add_item(
            space,
            Item::new(ItemId(1), "in-space.txt", ItemFacts::default()),
        );
        desktop
            .layer
            .push_item(Item::new(ItemId(2), "loose.txt", ItemFacts::default()));
        assert_eq!(desktop.space_item_count(), 1);
        assert_eq!(desktop.layer().item_count(), 1);
    }

    #[test]
    fn folded_space_is_only_hit_on_its_title_bar() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 200, 200));
        desktop.space_mut(id).unwrap().folded = true;
        assert_eq!(
            desktop.space_at(crate::geometry::Point::new(10, 10)),
            Some(id)
        );
        assert_eq!(desktop.space_at(crate::geometry::Point::new(10, 100)), None);
    }

    #[test]
    fn clamp_looks_clamps_every_space() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        desktop.space_mut(id).unwrap().look = Some(SpaceLook {
            theme: Theme {
                icon_size: 1,
                ..Theme::default()
            },
            gap: 999,
            padding: -10,
        });
        desktop.clamp_looks();
        let look = desktop.space(id).unwrap().look.as_ref().unwrap();
        assert_eq!(look.theme.icon_size, crate::theme::ICON_SIZE_RANGE.0);
        assert_eq!(look.gap, GAP_RANGE.1);
        assert_eq!(look.padding, GAP_RANGE.0);
    }
}
