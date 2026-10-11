use std::cmp::Ordering;
use std::path::PathBuf;

use crate::desktop::Desktop;
use crate::geometry::{Point, Rect, Size};
use crate::id::{ItemId, SpaceId};
use crate::item::{Item, ItemKind};
use crate::layout::{
    GridSpec, arrange_space, clamp_to_bounds, position_items, resize_rect, resolve_collisions,
    snap_rect,
};
use crate::space::{ResizeEdges, SpaceLook};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Creates a space at `at` (its top-left in desktop coordinates) with the
    /// configured `size`, optionally starting locked.
    CreateSpace {
        title: String,
        at: Point,
        size: Size,
        locked: bool,
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
    /// Re-lays out a space for the current (scaled) grid. A locked space keeps
    /// its rect and takes only the new title height and item positions; an
    /// unlocked one is arranged to fit its items.
    Reflow {
        id: SpaceId,
    },
    /// Reorders a space's items by `key` and re-lays them out.
    SortSpace {
        id: SpaceId,
        key: SortKey,
        descending: bool,
    },
    /// Turns a space's own look on or off. Turning it on for the first time
    /// starts from `seed` (the global settings); turning it off keeps the look,
    /// so turning it back on restores it.
    CustomizeSpace {
        id: SpaceId,
        on: bool,
        seed: SpaceLook,
    },
    /// Sets a space's customization exactly: whether it is on, and its own
    /// look (clamped). Settings edits pass `custom: true`; Cancel restores a
    /// snapshot, which may have no look yet.
    SetSpaceLook {
        id: SpaceId,
        custom: bool,
        look: Option<SpaceLook>,
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
    /// Moves `id` to slot `index` within its space, a manual rearrangement.
    ReorderItem {
        id: ItemId,
        index: usize,
    },
    /// Moves several items (a drag of a multi-selection) to `index`, keeping
    /// their relative order.
    ReorderItems {
        ids: Vec<ItemId>,
        index: usize,
    },
    /// Updates an item after its file was renamed on disk.
    RenameItem {
        id: ItemId,
        path: PathBuf,
        label: String,
    },
    AutoArrange {
        id: SpaceId,
    },
    SnapAll,
    /// Replaces the loose desktop layer from a shell snapshot, reconciling by
    /// path so `ItemId`s survive refreshes: an existing path keeps its id, a
    /// new one is allocated, and a path that is gone is dropped. Order follows
    /// `items`.
    SyncDesktop {
        items: Vec<Item>,
    },
    /// Moves a space's item out to the loose desktop layer. The app positions
    /// it with `SetDesktopItemPosition` once the shell reports where it landed.
    MoveToDesktop {
        item: ItemId,
    },
    /// Sets a loose item's absolute position (virtual-desktop coordinates).
    SetDesktopItemPosition {
        item: ItemId,
        position: Point,
    },
}

/// How `Command::SortSpace` orders a space's items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Kind,
    Size,
    Modified,
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
    /// Applies `command` against the desktop's stored (logical, base) layout.
    pub fn apply(&mut self, command: Command, bounds: Rect) -> CommandOutcome {
        let layout = self.layout;
        self.apply_with_layout(command, layout, bounds)
    }

    /// Applies `command` against an effective layout: the stored grid scaled
    /// for the monitor's DPI and the icon-size setting. The stored `layout`
    /// stays logical and is never rewritten by scaling.
    pub fn apply_with_layout(
        &mut self,
        command: Command,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        match command {
            Command::CreateSpace {
                title,
                at,
                size,
                locked,
            } => {
                let mut rect = Rect::new(at.x, at.y, size.width, size.height);
                rect = clamp_to_bounds(rect, bounds);
                let id = self.add_space(title, rect);
                if locked && let Some(space) = self.space_mut(id) {
                    space.locked = true;
                }
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
                let snap = layout.snap;
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
                let snap = layout.snap;
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
            Command::Reflow { id } => {
                let Some(space) = self.space_mut(id) else {
                    return CommandOutcome::NotFound;
                };
                if space.locked {
                    // A locked space keeps its size; the scale still reaches its
                    // title height and item positions.
                    space.title_height = layout.title_height;
                    position_items(space, layout);
                } else {
                    // `arrange_space` resizes auto spaces, reflows user-sized
                    // ones, and applies the title height either way.
                    arrange_space(space, layout);
                }
                CommandOutcome::Applied
            }
            Command::SortSpace {
                id,
                key,
                descending,
            } => {
                let Some(space) = self.space_mut(id) else {
                    return CommandOutcome::NotFound;
                };
                space.items.sort_by(|a, b| {
                    let order = match key {
                        SortKey::Name => compare_names(&a.label, &b.label),
                        SortKey::Kind => kind_rank(a.kind)
                            .cmp(&kind_rank(b.kind))
                            .then_with(|| compare_names(&a.label, &b.label)),
                        SortKey::Size => a
                            .size
                            .unwrap_or(0)
                            .cmp(&b.size.unwrap_or(0))
                            .then_with(|| compare_names(&a.label, &b.label)),
                        SortKey::Modified => a
                            .modified
                            .unwrap_or(0)
                            .cmp(&b.modified.unwrap_or(0))
                            .then_with(|| compare_names(&a.label, &b.label)),
                    };
                    if descending { order.reverse() } else { order }
                });
                position_items(space, layout);
                CommandOutcome::Applied
            }
            Command::CustomizeSpace { id, on, seed } => match self.space_mut(id) {
                Some(space) => {
                    if on && space.look.is_none() {
                        space.look = Some(seed.clamped());
                    }
                    space.custom = on;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
            Command::SetSpaceLook { id, custom, look } => match self.space_mut(id) {
                Some(space) => {
                    space.look = look.map(SpaceLook::clamped);
                    space.custom = custom;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
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
                self.rearrange(space, layout);
                CommandOutcome::Applied
            }
            Command::RemoveItem { id } => {
                let Some(source) = self.space_containing(id) else {
                    return CommandOutcome::NotFound;
                };
                self.remove_item(id);
                // Close the gap without resizing the space under the user.
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
                    self.rearrange(target, layout);
                    if let Some(source) = source {
                        // Close the gap the item left, without resizing the
                        // source space under the user.
                        if let Some(space) = self.space_mut(source) {
                            position_items(space, layout);
                        }
                    }
                    CommandOutcome::Applied
                } else {
                    CommandOutcome::Rejected
                }
            }
            Command::ReorderItem { id, index } => {
                let Some(source) = self.space_containing(id) else {
                    return CommandOutcome::NotFound;
                };
                let Some(space) = self.space_mut(source) else {
                    return CommandOutcome::NotFound;
                };
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
            }
            Command::ReorderItems { ids, index } => {
                let Some(id) = ids.first() else {
                    return CommandOutcome::Rejected;
                };
                let Some(source) = self.space_containing(*id) else {
                    return CommandOutcome::NotFound;
                };
                let Some(space) = self.space_mut(source) else {
                    return CommandOutcome::NotFound;
                };
                let mut moving: Vec<Item> = Vec::new();
                space.items.retain(|item| {
                    if ids.contains(&item.id) {
                        moving.push(item.clone());
                        false
                    } else {
                        true
                    }
                });
                if moving.is_empty() {
                    return CommandOutcome::Rejected;
                }
                let index = index.min(space.items.len());
                for (offset, item) in moving.into_iter().enumerate() {
                    space.items.insert(index + offset, item);
                }
                position_items(space, layout);
                CommandOutcome::Applied
            }
            Command::AutoArrange { id } => {
                if self.space(id).is_some() {
                    self.rearrange(id, layout);
                    CommandOutcome::Applied
                } else {
                    CommandOutcome::NotFound
                }
            }
            Command::RenameItem { id, path, label } => {
                let Some(source) = self.space_containing(id) else {
                    return CommandOutcome::NotFound;
                };
                let Some(space) = self.space_mut(source) else {
                    return CommandOutcome::NotFound;
                };
                let Some(item) = space.item_mut(id) else {
                    return CommandOutcome::NotFound;
                };
                item.path = path;
                item.label = label;
                CommandOutcome::Applied
            }
            Command::SnapAll => {
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
            Command::SyncDesktop { items } => {
                let mut next = self.next_item_id().0;
                let mut reconciled = Vec::with_capacity(items.len());
                for mut entry in items {
                    match self.layer.items().iter().find(|old| old.path == entry.path) {
                        Some(old) => entry.id = old.id,
                        None => {
                            entry.id = ItemId(next);
                            next += 1;
                        }
                    }
                    reconciled.push(entry);
                }
                self.layer.replace_items(reconciled);
                CommandOutcome::Applied
            }
            Command::MoveToDesktop { item } => {
                let Some(source) = self.space_containing(item) else {
                    return CommandOutcome::NotFound;
                };
                let Some(mut moved) = self.remove_item(item) else {
                    return CommandOutcome::NotFound;
                };
                moved.position = Point::ZERO;
                self.layer.push_item(moved);
                // Close the gap the item left, without resizing the source
                // space under the user.
                if let Some(space) = self.space_mut(source) {
                    position_items(space, layout);
                }
                CommandOutcome::Applied
            }
            Command::SetDesktopItemPosition { item, position } => match self.layer.item_mut(item) {
                Some(entry) => {
                    entry.position = position;
                    CommandOutcome::Applied
                }
                None => CommandOutcome::NotFound,
            },
        }
    }

    fn rearrange(&mut self, id: SpaceId, layout: GridSpec) {
        if let Some(space) = self.space_mut(id) {
            arrange_space(space, layout);
        }
    }
}

/// Case-insensitive label order, comparing lowered characters without
/// allocating.
fn compare_names(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().flat_map(char::to_lowercase);
    let mut right = b.chars().flat_map(char::to_lowercase);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => continue,
                other => return other,
            },
        }
    }
}

/// Group order for `SortKey::Kind`: folders, files, shortcuts, drives, other.
fn kind_rank(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Folder => 0,
        ItemKind::File => 1,
        ItemKind::Shortcut => 2,
        ItemKind::Drive => 3,
        ItemKind::Other => 4,
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
                size: Size::new(240, 120),
                locked: false,
            },
            bounds(),
        );
        assert_eq!(desktop.spaces.len(), 1);
        assert_eq!(desktop.spaces[0].rect, Rect::new(40, 40, 240, 120));
        let id = desktop.spaces[0].id;
        assert_eq!(
            desktop.apply(Command::DeleteSpace { id }, bounds()),
            CommandOutcome::Applied
        );
        assert!(desktop.spaces.is_empty());
    }

    #[test]
    fn create_space_uses_the_size_and_lock() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Locked".into(),
                at: Point::new(10, 10),
                size: Size::new(320, 200),
                locked: true,
            },
            bounds(),
        );
        let space = &desktop.spaces[0];
        assert!(space.locked);
        assert_eq!(space.rect.size(), Size::new(320, 200));
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
        assert_eq!(desktop.space_item_count(), 1);
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

    fn space_with_items(items: Vec<Item>) -> (Desktop, SpaceId) {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 400, 400));
        for item in items {
            desktop.add_item(id, item);
        }
        (desktop, id)
    }

    fn item(id: u64, label: &str, kind: ItemKind, size: u64, modified: u64) -> Item {
        let mut item = Item::new(ItemId(id), format!("C:/missing/{label}"));
        item.label = label.to_string();
        item.kind = kind;
        item.size = Some(size);
        item.modified = Some(modified);
        item
    }

    fn labels(desktop: &Desktop, id: SpaceId) -> Vec<String> {
        desktop
            .space(id)
            .unwrap()
            .items
            .iter()
            .map(|item| item.label.clone())
            .collect()
    }

    #[test]
    fn sort_space_by_name_is_case_insensitive_both_ways() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "banana", ItemKind::File, 1, 1),
            item(2, "Apple", ItemKind::File, 1, 1),
            item(3, "cherry", ItemKind::File, 1, 1),
        ]);
        desktop.apply(
            Command::SortSpace {
                id,
                key: SortKey::Name,
                descending: false,
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["Apple", "banana", "cherry"]);
        desktop.apply(
            Command::SortSpace {
                id,
                key: SortKey::Name,
                descending: true,
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["cherry", "banana", "Apple"]);
    }

    #[test]
    fn sort_space_by_kind_groups_folders_first() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "zebra", ItemKind::File, 0, 0),
            item(2, "alpha", ItemKind::Folder, 0, 0),
        ]);
        desktop.apply(
            Command::SortSpace {
                id,
                key: SortKey::Kind,
                descending: false,
            },
            bounds(),
        );
        let first = &desktop.space(id).unwrap().items[0];
        assert_eq!(first.label, "alpha");
        assert_eq!(first.kind, ItemKind::Folder);
    }

    #[test]
    fn sort_space_by_size_and_modified() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "big", ItemKind::File, 300, 10),
            item(2, "small", ItemKind::File, 10, 30),
            item(3, "mid", ItemKind::File, 100, 20),
        ]);
        desktop.apply(
            Command::SortSpace {
                id,
                key: SortKey::Size,
                descending: false,
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["small", "mid", "big"]);
        desktop.apply(
            Command::SortSpace {
                id,
                key: SortKey::Modified,
                descending: true,
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["small", "mid", "big"]);
    }

    #[test]
    fn reorder_items_moves_a_block() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "a", ItemKind::File, 0, 0),
            item(2, "b", ItemKind::File, 0, 0),
            item(3, "c", ItemKind::File, 0, 0),
            item(4, "d", ItemKind::File, 0, 0),
        ]);
        let ids: Vec<ItemId> = desktop
            .space(id)
            .unwrap()
            .items
            .iter()
            .map(|i| i.id)
            .collect();
        // Move the first two items to the end as a block.
        assert_eq!(
            desktop.apply(
                Command::ReorderItems {
                    ids: vec![ids[0], ids[1]],
                    index: 4,
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["c", "d", "a", "b"]);
    }

    #[test]
    fn reorder_item_moves_within_its_space() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "a", ItemKind::File, 0, 0),
            item(2, "b", ItemKind::File, 0, 0),
            item(3, "c", ItemKind::File, 0, 0),
        ]);
        let first = desktop.space(id).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(
                Command::ReorderItem {
                    id: first,
                    index: 2,
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["b", "c", "a"]);
        assert_eq!(
            desktop.space(id).unwrap().items[2].position,
            desktop.layout.grid_position(2)
        );
        // An out-of-range slot is rejected without changing anything.
        assert_eq!(
            desktop.apply(
                Command::ReorderItem {
                    id: first,
                    index: 9,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(labels(&desktop, id), ["b", "c", "a"]);
    }

    #[test]
    fn reflow_locked_space_keeps_its_rect_and_takes_the_title_height() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 300, 300));
        desktop.apply(Command::ToggleLock { id }, bounds());
        let before = desktop.space(id).unwrap().rect;
        let mut layout = desktop.layout;
        layout.title_height = 40;
        assert_eq!(
            desktop.apply_with_layout(Command::Reflow { id }, layout, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.rect, before);
        assert_eq!(space.title_height, 40);
    }

    #[test]
    fn reflow_unlocked_auto_space_resizes_to_fit() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        for name in ["one.txt", "two.txt", "three.txt"] {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: name.into(),
                },
                bounds(),
            );
        }
        let before = desktop.space(id).unwrap().rect;
        let mut layout = desktop.layout;
        layout.cell_width *= 2;
        layout.cell_height *= 2;
        assert_eq!(
            desktop.apply_with_layout(Command::Reflow { id }, layout, bounds()),
            CommandOutcome::Applied
        );
        let after = desktop.space(id).unwrap().rect;
        assert!(after.width > before.width);
        assert!(after.height > before.height);
    }

    #[test]
    fn reflow_missing_space_is_not_found() {
        let mut desktop = Desktop::new();
        assert_eq!(
            desktop.apply(Command::Reflow { id: SpaceId(999) }, bounds()),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn customizing_seeds_once_and_keeps_the_look_when_off() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let mut seed = SpaceLook::default();
        seed.theme.icon_size = 150;
        let customize = |on, seed: &SpaceLook| Command::CustomizeSpace {
            id,
            on,
            seed: seed.clone(),
        };

        desktop.apply(customize(true, &seed), bounds());
        let space = desktop.space(id).unwrap();
        assert!(space.custom);
        assert_eq!(space.look.as_ref().unwrap().theme.icon_size, 150);

        desktop.apply(customize(false, &seed), bounds());
        let space = desktop.space(id).unwrap();
        assert!(!space.custom);
        assert!(space.look.is_some(), "kept while off");

        // Turning it back on restores the kept look, not a new seed.
        seed.theme.icon_size = 75;
        desktop.apply(customize(true, &seed), bounds());
        assert_eq!(
            desktop
                .space(id)
                .unwrap()
                .look
                .as_ref()
                .unwrap()
                .theme
                .icon_size,
            150
        );
        assert_eq!(
            desktop.apply(
                Command::CustomizeSpace {
                    id: SpaceId(999),
                    on: true,
                    seed,
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn set_space_look_sets_both_exactly_and_clamps() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let mut look = SpaceLook::default();
        look.theme.icon_size = u8::MAX;
        look.gap = 1000;
        let set = |custom, look| Command::SetSpaceLook { id, custom, look };
        assert_eq!(
            desktop.apply(set(true, Some(look)), bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert!(space.custom);
        let look = space.look.as_ref().unwrap();
        assert_eq!(look.theme.icon_size, crate::theme::ICON_SIZE_RANGE.1);
        assert_eq!(look.gap, crate::layout::GAP_RANGE.1);

        // Restoring a snapshot from before the space was customized.
        desktop.apply(set(false, None), bounds());
        let space = desktop.space(id).unwrap();
        assert!(!space.custom && space.look.is_none());
    }

    #[test]
    fn sync_desktop_reuses_ids_by_path_across_a_reorder() {
        let enter = |name: &str, x: i32| {
            let mut item = Item::new(ItemId(0), format!("C:/missing/{name}"));
            item.position = Point::new(x, 0);
            item
        };
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::SyncDesktop {
                items: vec![enter("a.txt", 10), enter("b.txt", 20)],
            },
            bounds(),
        );
        let a = desktop.layer().items()[0].id;
        let b = desktop.layer().items()[1].id;
        assert_ne!(a, b);

        // Reorder, drop b, and add c.
        desktop.apply(
            Command::SyncDesktop {
                items: vec![enter("a.txt", 30), enter("c.txt", 40)],
            },
            bounds(),
        );
        let items = desktop.layer().items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, a, "same path keeps its id");
        assert_eq!(items[0].position, Point::new(30, 0));
        assert!(
            items.iter().all(|item| item.id != b),
            "gone path is dropped"
        );
        assert_ne!(items[1].id, a, "new path gets a fresh id");
    }

    #[test]
    fn move_to_desktop_moves_a_space_item_to_the_layer() {
        let mut desktop = Desktop::new();
        let space = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let item = Item::new(ItemId(1), "C:/missing/a.txt");
        let item_id = item.id;
        desktop.add_item(space, item);
        assert_eq!(
            desktop.apply(Command::MoveToDesktop { item: item_id }, bounds()),
            CommandOutcome::Applied
        );
        assert!(desktop.space(space).unwrap().items.is_empty());
        assert!(desktop.layer().item(item_id).is_some());
        assert_eq!(desktop.layer().item_count(), 1);
        assert_eq!(
            desktop.apply(Command::MoveToDesktop { item: ItemId(999) }, bounds()),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn set_desktop_item_position_moves_and_reports_missing() {
        let mut desktop = Desktop::new();
        desktop
            .layer_mut()
            .push_item(Item::new(ItemId(1), "C:/missing/a.txt"));
        assert_eq!(
            desktop.apply(
                Command::SetDesktopItemPosition {
                    item: ItemId(1),
                    position: Point::new(50, 60),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(
            desktop.layer().item(ItemId(1)).unwrap().position,
            Point::new(50, 60)
        );
        assert_eq!(
            desktop.apply(
                Command::SetDesktopItemPosition {
                    item: ItemId(999),
                    position: Point::ZERO,
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
    }
}
