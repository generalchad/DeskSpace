//! Commands on the loose desktop layer (items outside any space).

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::geometry::Point;
use crate::id::ItemId;
use crate::item::Item;
use crate::layout::GridSpec;

impl Desktop {
    pub(super) fn apply_sync_desktop(&mut self, items: Vec<Item>) -> CommandOutcome {
        let mut next = self.next_item_id().0;
        let reconciled = items
            .into_iter()
            .map(|mut entry| {
                match self.layer.items().iter().find(|old| old.path == entry.path) {
                    Some(old) => entry.id = old.id,
                    None => {
                        entry.id = ItemId(next);
                        next += 1;
                    }
                }
                entry
            })
            .collect();
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
