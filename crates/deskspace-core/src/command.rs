use std::path::PathBuf;

use crate::desktop::Desktop;
use crate::geometry::{Point, Rect};
use crate::id::{ItemId, SpaceId};
use crate::item::Item;
use crate::layout::{
    arrange_space, clamp_to_bounds, position_items, resize_rect, resolve_collisions, snap_rect,
};
use crate::space::ResizeEdges;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    CreateSpace {
        title: String,
        at: Point,
    },
    DeleteSpace {
        id: SpaceId,
    },
    RenameSpace {
        id: SpaceId,
        title: String,
    },
    MoveSpace {
        id: SpaceId,
        delta: Point,
    },
    SetSpaceRect {
        id: SpaceId,
        rect: Rect,
    },
    /// Drags the grabbed `edges` of a space to `to` (desktop coordinates).
    /// The space becomes user-sized and its items reflow to the new width.
    ResizeSpace {
        id: SpaceId,
        edges: ResizeEdges,
        to: Point,
    },
    /// Drops a user size so the space grows to fit its items again.
    FitToContents {
        id: SpaceId,
    },
    ToggleLock {
        id: SpaceId,
    },
    ToggleRollup {
        id: SpaceId,
    },
    AddItem {
        space: SpaceId,
        path: PathBuf,
    },
    RemoveItem {
        id: ItemId,
    },
    MoveItem {
        id: ItemId,
        target: SpaceId,
    },
    AutoArrange {
        id: SpaceId,
    },
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
            Command::CreateSpace { title, at } => {
                let mut rect = Rect::new(at.x, at.y, 240, 120);
                rect = clamp_to_bounds(rect, bounds);
                self.add_space(title, rect);
                CommandOutcome::Applied
            }
            Command::DeleteSpace { id } => match self.remove_space(id) {
                Some(_) => CommandOutcome::Applied,
                None => CommandOutcome::NotFound,
            },
            Command::RenameSpace { id, title } => match self.space_mut(id) {
                Some(space) => {
                    space.title = title;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
            Command::MoveSpace { id, delta } => {
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
            Command::SetSpaceRect { id, rect } => {
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
            Command::ResizeSpace { id, edges, to } => {
                let layout = self.layout;
                let Some(space) = self.space_mut(id) else {
                    return CommandOutcome::NotFound;
                };
                if space.locked || space.rolled_up || !edges.any() {
                    return CommandOutcome::Rejected;
                }
                let min = layout.min_space_size();
                space.rect = resize_rect(space.rect, edges, to, min, layout.snap, bounds);
                space.user_sized = true;
                position_items(space, layout);
                CommandOutcome::Applied
            }
            Command::FitToContents { id } => {
                let layout = self.layout;
                let Some(space) = self.space_mut(id) else {
                    return CommandOutcome::NotFound;
                };
                if space.locked {
                    return CommandOutcome::Rejected;
                }
                space.user_sized = false;
                arrange_space(space, layout);
                space.rect = clamp_to_bounds(space.rect, bounds);
                CommandOutcome::Applied
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
            Command::RemoveItem { id } => {
                let Some(source) = self.space_containing(id) else {
                    return CommandOutcome::NotFound;
                };
                self.remove_item(id);
                // Close the gap without resizing the space under the user.
                let layout = self.layout;
                if let Some(space) = self.space_mut(source) {
                    position_items(space, layout);
                }
                CommandOutcome::Applied
            }
            Command::MoveItem { id, target } => {
                if self.item(id).is_none() || self.space(target).is_none() {
                    return CommandOutcome::NotFound;
                }
                let source = self.space_containing(id);
                if self.move_item(id, target) {
                    self.rearrange(target);
                    if let Some(source) = source {
                        // Close the gap the item left, without resizing the
                        // source space under the user.
                        let layout = self.layout;
                        if let Some(space) = self.space_mut(source) {
                            position_items(space, layout);
                        }
                    }
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
                let ids: Vec<SpaceId> = self.spaces.iter().map(|space| space.id).collect();
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

    fn rearrange(&mut self, id: SpaceId) {
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
            Command::CreateSpace {
                title: "New".into(),
                at: Point::new(40, 40),
            },
            bounds(),
        );
        assert_eq!(desktop.spaces.len(), 1);
        let id = desktop.spaces[0].id;
        assert_eq!(
            desktop.apply(Command::DeleteSpace { id }, bounds()),
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
                Command::MoveSpace {
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
                    target: SpaceId(999),
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
        assert_eq!(desktop.item_count(), 1);
    }

    #[test]
    fn move_item_closes_the_gap_in_the_source_space() {
        let mut desktop = Desktop::new();
        let a = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let b = desktop.add_space("B", Rect::new(300, 0, 100, 100));
        for name in ["one.txt", "two.txt"] {
            desktop.apply(
                Command::AddItem {
                    space: a,
                    path: name.into(),
                },
                bounds(),
            );
        }
        let first = desktop.space(a).unwrap().items[0].id;
        desktop.apply(
            Command::MoveItem {
                id: first,
                target: b,
            },
            bounds(),
        );
        let remaining = &desktop.space(a).unwrap().items[0];
        assert_eq!(remaining.position, desktop.layout.grid_position(0));
    }

    #[test]
    fn resized_space_keeps_its_size_when_items_are_added() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let edges = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        let outcome = desktop.apply(
            Command::ResizeSpace {
                id,
                edges,
                to: Point::new(480, 320),
            },
            bounds(),
        );
        assert_eq!(outcome, CommandOutcome::Applied);
        let sized = desktop.space(id).unwrap().rect;
        assert_eq!(sized, Rect::new(0, 0, 480, 320));
        for index in 0..20 {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: format!("file{index}.txt").into(),
                },
                bounds(),
            );
        }
        let space = desktop.space(id).unwrap();
        assert_eq!(space.rect, sized);
        assert!(space.max_scroll(desktop.layout.cell(), desktop.layout.padding) > 0);

        desktop.apply(Command::FitToContents { id }, bounds());
        assert!(!desktop.space(id).unwrap().user_sized);
        assert_ne!(desktop.space(id).unwrap().rect.size(), sized.size());
    }

    #[test]
    fn locked_and_rolled_up_spaces_reject_resize() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 300, 300));
        let resize = Command::ResizeSpace {
            id,
            edges: ResizeEdges {
                right: true,
                ..ResizeEdges::NONE
            },
            to: Point::new(600, 100),
        };
        desktop.space_mut(id).unwrap().rolled_up = true;
        assert_eq!(
            desktop.apply(resize.clone(), bounds()),
            CommandOutcome::Rejected
        );
        desktop.space_mut(id).unwrap().rolled_up = false;
        desktop.space_mut(id).unwrap().locked = true;
        assert_eq!(desktop.apply(resize, bounds()), CommandOutcome::Rejected);
    }

    #[test]
    fn remove_item_closes_the_gap() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        for name in ["one.txt", "two.txt"] {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: name.into(),
                },
                bounds(),
            );
        }
        let first = desktop.space(id).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(Command::RemoveItem { id: first }, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.items.len(), 1);
        assert_eq!(space.items[0].position, desktop.layout.grid_position(0));
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
