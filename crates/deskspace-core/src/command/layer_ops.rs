//! Commands on the loose desktop layer (items outside any space).

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::geometry::Point;
use crate::id::ItemId;
use crate::item::Item;
use crate::layout::GridSpec;

impl Desktop {
    pub(super) fn apply_sync_desktop(&mut self, items: Vec<Item>) -> CommandOutcome {
        let mut reconciled = Vec::with_capacity(items.len());
        for mut entry in items {
            let known = self
                .layer
                .items()
                .iter()
                .find(|old| old.path == entry.path)
                .map(|old| old.id);
            entry.id = known.unwrap_or_else(|| self.take_item_id());
            reconciled.push(entry);
        }
        self.layer.replace_items(reconciled);
        CommandOutcome::Applied
    }

    pub(super) fn apply_move_to_desktop(
        &mut self,
        item: ItemId,
        layout: GridSpec,
    ) -> CommandOutcome {
        let Some(source) = self.space_containing(item) else {
            return CommandOutcome::NotFound;
        };
        let Some(mut moved) = self.remove_item(item) else {
            return CommandOutcome::NotFound;
        };
        moved.position = Point::ZERO;
        self.layer.push_item(moved);
        self.close_gap(source, layout);
        CommandOutcome::Applied
    }

    pub(super) fn apply_set_desktop_item_position(
        &mut self,
        item: ItemId,
        position: Point,
    ) -> CommandOutcome {
        match self.layer.item_mut(item) {
            Some(entry) => {
                entry.position = position;
                CommandOutcome::Applied
            }
            None => CommandOutcome::NotFound,
        }
    }
}
