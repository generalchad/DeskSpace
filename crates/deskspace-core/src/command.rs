use std::path::PathBuf;

use crate::desktop::Desktop;
use crate::geometry::{Point, Rect};
use crate::id::{spaceId, ItemId};
use crate::item::Item;
use crate::layout::{arrange_space, clamp_to_bounds, resolve_collisions, snap_rect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Createspace { title: String, at: Point },
    Deletespace { id: spaceId },
    Renamespace { id: spaceId, title: String },
    Movespace { id: spaceId, delta: Point },
    SetspaceRect { id: spaceId, rect: Rect },
    ToggleLock { id: spaceId },
    ToggleRollup { id: spaceId },
    AddItem { space: spaceId, path: PathBuf },
    RemoveItem { id: ItemId },
    MoveItem { id: ItemId, target: spaceId },
    AutoArrange { id: spaceId },
    SnapAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    Applied,
    NotFound,
    Rejected,
}

impl CommandOutcome {
    pub fn is_applied(self) -> bool {
        matches!(self, Self::Applied)
    }
}

impl Desktop {
    pub fn apply(&mut self, command: Command, bounds: Rect) -> CommandOutcome {
        match command {
            Command::Createspace { title, at } => {
                let mut rect = Rect::new(at.x, at.y, 240, 120);
                rect = clamp_to_bounds(rect, bounds);
                self.add_space(title, rect);
                CommandOutcome::Applied
            }
            Command::Deletespace { id } => match self.remove_space(id) {
                Some(_) => CommandOutcome::Applied,
                None => CommandOutcome::NotFound,
            },
            Command::Renamespace { id, title } => match self.space_mut(id) {
                Some(space) => {
                    space.title = title;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
            Command::Movespace { id, delta } => {
                let snap = self.layout.snap;
                match self.space_mut(id) {
                    Some(space) => {
                        if space.locked {
                            return CommandOutcome::Rejected;
                        }
                        let moved = space.rect.offset(delta.x, delta.y);
                        let snapped = snap_rect(moved, snap);
                        space.rect = clamp_to_bounds(snapped, bounds);
                        CommandOutcome::Applied
                    }
                    None => CommandOutcome::NotFound,
                }
            }
            Command::SetspaceRect { id, rect } => {
                let snap = self.layout.snap;
                match self.space_mut(id) {
                    Some(space) => {
                        if space.locked {
                            return CommandOutcome::Rejected;
                        }
                        space.rect = clamp_to_bounds(snap_rect(rect, snap), bounds);
                        CommandOutcome::Applied
                    }
                    None => CommandOutcome::NotFound,
                }
            }
            Command::ToggleLock { id } => match self.space_mut(id) {
                Some(space) => {
                    space.locked = !space.locked;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
            Command::ToggleRollup { id } => match self.space_mut(id) {
                Some(space) => {
                    space.rolled_up = !space.rolled_up;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
            Command::AddItem { space, path } => {
                if self.space(space).is_none() {
                    return CommandOutcome::NotFound;
                }
                let item = Item::new(self.next_item_id(), path);
                self.add_item(space, item);
                self.rearrange(space);
                CommandOutcome::Applied
            }
            Command::RemoveItem { id } => match self.remove_item(id) {
                Some(_) => CommandOutcome::Applied,
                None => CommandOutcome::NotFound,
            },
            Command::MoveItem { id, target } => {
                if self.item(id).is_none() || self.space(target).is_none() {
                    return CommandOutcome::NotFound;
                }
                if self.move_item(id, target) {
                    self.rearrange(target);
                    CommandOutcome::Applied
                } else {
                    CommandOutcome::Rejected
                }
            }
            Command::AutoArrange { id } => {
                if self.space(id).is_some() {
                    self.rearrange(id);
                    CommandOutcome::Applied
                } else {
                    CommandOutcome::NotFound
                }
            }
            Command::SnapAll => {
                let layout = self.layout;
                let ids: Vec<spaceId> = self.spaces.iter().map(|space| space.id).collect();
                for id in ids {
                    if let Some(space) = self.space_mut(id) {
                        if !space.locked {
                            space.rect =
                                clamp_to_bounds(snap_rect(space.rect, layout.snap), bounds);
                        }
                    }
                }
                resolve_collisions(&mut self.spaces, layout.gap, bounds);
                CommandOutcome::Applied
            }
        }
    }

    fn rearrange(&mut self, id: spaceId) {
        let layout = self.layout;
        if let Some(space) = self.space_mut(id) {
            arrange_space(space, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect::new(0, 0, 1920, 1080)
    }

    #[test]
    fn create_then_delete_space() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::Createspace {
                title: "New".into(),
                at: Point::new(40, 40),
            },
            bounds(),
        );
        assert_eq!(desktop.spaces.len(), 1);
        let id = desktop.spaces[0].id;
        assert_eq!(
            desktop.apply(Command::Deletespace { id }, bounds()),
            CommandOutcome::Applied
        );
        assert!(desktop.spaces.is_empty());
    }

    #[test]
    fn locked_space_rejects_moves() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("Locked", Rect::new(0, 0, 100, 100));
        desktop.apply(Command::ToggleLock { id }, bounds());
        assert_eq!(
            desktop.apply(
                Command::Movespace {
                    id,
                    delta: Point::new(50, 0),
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(desktop.space(id).unwrap().rect.x, 0);
    }

    #[test]
    fn move_item_to_missing_space_is_not_found_and_keeps_item() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        desktop.apply(
            Command::AddItem {
                space: id,
                path: "file.txt".into(),
            },
            bounds(),
        );
        let item = desktop.spaces[0].items[0].id;
        assert_eq!(
            desktop.apply(
                Command::MoveItem {
                    id: item,
                    target: spaceId(999),
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
        assert_eq!(desktop.item_count(), 1);
    }

    #[test]
    fn snap_all_resolves_overlap() {
        let mut desktop = Desktop::new();
        desktop.add_space("A", Rect::new(3, 5, 200, 150));
        desktop.add_space("B", Rect::new(4, 6, 200, 150));
        desktop.apply(Command::SnapAll, bounds());
        let a = &desktop.spaces[0].rect;
        let b = &desktop.spaces[1].rect;
        assert_eq!(a.x % desktop.layout.snap, 0);
        assert!(!a.intersects(*b));
    }
}
