//! Commands that add, remove, move, reorder, and rename items in spaces.

use std::path::PathBuf;

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::id::{ItemId, SpaceId};
use crate::item::{Item, ItemFacts};
use crate::layout::{GridSpec, position_items};

impl Desktop {
    pub(super) fn apply_add_item(
        &mut self,
        space: SpaceId,
        path: PathBuf,
        facts: ItemFacts,
        layout: GridSpec,
    ) -> CommandOutcome {
        if self.space(space).is_none() {
            return CommandOutcome::NotFound;
        }
        let item = Item::new(self.take_item_id(), path, facts);
        self.add_item(space, item);
        self.rearrange(space, layout);
        CommandOutcome::Applied
    }

    pub(super) fn apply_remove_item(&mut self, id: ItemId, layout: GridSpec) -> CommandOutcome {
        let Some(source) = self.space_containing(id) else {
            return CommandOutcome::NotFound;
        };
        self.remove_item(id);
        self.close_gap(source, layout);
        CommandOutcome::Applied
    }

    pub(super) fn apply_move_item(
        &mut self,
        id: ItemId,
        target: SpaceId,
        layout: GridSpec,
    ) -> CommandOutcome {
        if self.item(id).is_none() || self.space(target).is_none() {
            return CommandOutcome::NotFound;
        }
        let source = self.space_containing(id);
        if !self.move_item(id, target) {
            return CommandOutcome::Rejected;
        }
        self.rearrange(target, layout);
        if let Some(source) = source {
            self.close_gap(source, layout);
        }
        CommandOutcome::Applied
    }

    pub(super) fn apply_reorder_item(
        &mut self,
        id: ItemId,
        index: usize,
        layout: GridSpec,
    ) -> CommandOutcome {
        let Some(source) = self.space_containing(id) else {
            return CommandOutcome::NotFound;
        };
        self.with_space(source, |space| {
            let Some(current) = space.items.iter().position(|item| item.id == id) else {
                return CommandOutcome::NotFound;
            };
            if index >= space.items.len() {
                return CommandOutcome::Rejected;
            }
            if index != current {
                let item = space.items.remove(current);
                space.items.insert(index, item);
                position_items(space, layout);
            }
            CommandOutcome::Applied
        })
    }

    /// Moves the items of `ids` that share the first one's space to `index`,
    /// keeping their relative order.
    pub(super) fn apply_reorder_items(
        &mut self,
        ids: &[ItemId],
        index: usize,
        layout: GridSpec,
    ) -> CommandOutcome {
        let Some(&first) = ids.first() else {
            return CommandOutcome::Rejected;
        };
        let Some(source) = self.space_containing(first) else {
            return CommandOutcome::NotFound;
        };
        self.with_space(source, |space| {
            let moving: Vec<Item> = space
                .items
                .extract_if(.., |item| ids.contains(&item.id))
                .collect();
            if moving.is_empty() {
                return CommandOutcome::Rejected;
            }
            let index = index.min(space.items.len());
            space.items.splice(index..index, moving);
            position_items(space, layout);
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_rename_item(
        &mut self,
        id: ItemId,
        path: PathBuf,
        label: String,
    ) -> CommandOutcome {
        let Some(source) = self.space_containing(id) else {
            return CommandOutcome::NotFound;
        };
        self.with_space(source, |space| {
            let Some(item) = space.item_mut(id) else {
                return CommandOutcome::NotFound;
            };
            item.path = path;
            item.label = label;
            CommandOutcome::Applied
        })
    }
}
