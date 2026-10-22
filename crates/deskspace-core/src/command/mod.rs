//! The command layer: every change to the domain model is a `Command` applied
//! with `Desktop::apply`.
//!
//! `apply_with_layout` only dispatches; each command is a private `Desktop::apply_<variant>`
//! method in the submodule for what it changes. To add a command, add the
//! variant here, one dispatch arm, and the method beside its neighbours.

mod item_ops;
mod layer_ops;
mod lens_ops;
mod sort;
mod space_ops;
mod touched;

use std::path::PathBuf;

use crate::desktop::Desktop;
use crate::geometry::{Point, Rect, Size};
use crate::id::{ItemId, SpaceId};
use crate::item::{Item, ItemFacts};
use crate::layout::{GridSpec, arrange_space, position_items};
use crate::source::{FolderEntry, FolderView, LensTitle};
use crate::space::{ResizeEdges, Space, SpaceLook};
use crate::view::SpaceView;

pub use crate::item::SortKey;
pub use touched::Touched;

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
    /// Creates a lens: a space that mirrors `path` live (ROADMAP D3, M4).
    /// `path` is only stored; `SyncLens` fills the items.
    CreateLens {
        title: String,
        at: Point,
        size: Size,
        path: PathBuf,
        locked: bool,
    },
    /// Rebinds a lens to `path`, sets its title text, and clears its listed
    /// items until the next `SyncLens`.
    SetLensFolder {
        id: SpaceId,
        path: PathBuf,
        title: String,
    },
    /// Sets how a lens's title follows its folder and the text to show now.
    SetLensTitle {
        id: SpaceId,
        title: LensTitle,
        text: String,
    },
    /// Replaces a lens's view settings. The items are left as they are; the
    /// app syncs the folder again to apply a changed filter or order.
    SetLensView {
        id: SpaceId,
        view: FolderView,
    },
    /// Replaces a lens's items from the folder's current `entries`. Hidden
    /// entries are dropped unless the view shows them; the result is sorted
    /// with natural name order. A path that survives keeps its `ItemId`, a new
    /// path gets a fresh id, and a path that is gone is dropped.
    SyncLens {
        id: SpaceId,
        entries: Vec<FolderEntry>,
    },
    DeleteSpace {
        id: SpaceId,
    },
    RenameSpace {
        id: SpaceId,
        title: String,
    },
    /// Sets how a space lays its items out (D15). Works on a normal space and
    /// a lens alike; the view is separate from the look.
    SetSpaceView {
        id: SpaceId,
        view: SpaceView,
    },
    /// Moves an unlocked space by `delta`, snapped to the grid and kept
    /// inside the bounds.
    MoveSpace {
        id: SpaceId,
        delta: Point,
    },
    /// Sets an unlocked space's rect, snapped to the grid and kept inside the
    /// bounds.
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
    /// Keeps a space at its current size from now on, as if the user had sized
    /// it, so later re-flows change its columns rather than its rect. An
    /// icon-size change uses it, so spaces keep their size as an Explorer
    /// window does (`TASKS.md` F15).
    KeepSize {
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
    /// Puts spaces back exactly as they were, for undo and redo (ROADMAP
    /// D11). For each id, `Some((index, space))` puts that space at `index`
    /// in the space order, replacing a space with the same id, and `None`
    /// removes it. The layout is not applied: a restore is an exact copy.
    /// `Rejected`, changing nothing, if an id is listed twice, a space does
    /// not carry its listed id, or a restored item's id is held by a space
    /// not listed, by another restored space, or by the desktop layer.
    RestoreSpaces {
        spaces: Vec<(SpaceId, Option<(usize, Space)>)>,
    },
    ToggleLock {
        id: SpaceId,
    },
    ToggleFold {
        id: SpaceId,
    },
    /// Lists `path` in `space`. The caller reads `facts` from the file
    /// system; core does no IO. A lens is `Rejected`: its items come from its
    /// folder through `SyncLens`.
    AddItem {
        space: SpaceId,
        path: PathBuf,
        facts: ItemFacts,
    },
    RemoveItem {
        id: ItemId,
    },
    /// Moves an item into `target`. Either side being a lens is `Rejected`
    /// (ROADMAP D3): a lens is a view, so its moves are file moves through the
    /// shell.
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
    /// Updates an item's path and label: after its file was renamed on disk,
    /// or to give a new item the shell's display name.
    RenameItem {
        id: ItemId,
        path: PathBuf,
        label: String,
    },
    /// Lays a space's items out on the grid again, fitting the space to them
    /// unless it is user-sized.
    AutoArrange {
        id: SpaceId,
    },
    /// Snaps every unlocked space to the grid, then pushes overlapping spaces
    /// apart.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    /// The command changed (or deliberately kept) the model.
    Applied,
    /// The space or item it names does not exist.
    NotFound,
    /// The target exists but refuses the change, for example a locked space.
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
            } => self.apply_create_space(title, at, size, locked, bounds),
            Command::CreateLens {
                title,
                at,
                size,
                path,
                locked,
            } => self.apply_create_lens(title, at, size, path, locked, bounds),
            Command::SetLensFolder { id, path, title } => {
                self.apply_set_lens_folder(id, path, title)
            }
            Command::SetLensTitle { id, title, text } => self.apply_set_lens_title(id, title, text),
            Command::SetLensView { id, view } => self.apply_set_lens_view(id, view),
            Command::SyncLens { id, entries } => self.apply_sync_lens(id, entries, layout),
            Command::DeleteSpace { id } => self.apply_delete_space(id),
            Command::RenameSpace { id, title } => self.apply_rename_space(id, title),
            Command::SetSpaceView { id, view } => self.apply_set_space_view(id, view),
            Command::MoveSpace { id, delta } => self.apply_move_space(id, delta, layout, bounds),
            Command::SetSpaceRect { id, rect } => {
                self.apply_set_space_rect(id, rect, layout, bounds)
            }
            Command::ResizeSpace { id, edges, to } => {
                self.apply_resize_space(id, edges, to, layout, bounds)
            }
            Command::FitToContents { id } => self.apply_fit_to_contents(id, layout, bounds),
            Command::KeepSize { id } => self.apply_keep_size(id),
            Command::Reflow { id } => self.apply_reflow(id, layout),
            Command::SortSpace {
                id,
                key,
                descending,
            } => self.apply_sort_space(id, key, descending, layout),
            Command::CustomizeSpace { id, on, seed } => self.apply_customize_space(id, on, seed),
            Command::SetSpaceLook { id, custom, look } => {
                self.apply_set_space_look(id, custom, look)
            }
            Command::RestoreSpaces { spaces } => self.apply_restore_spaces(spaces),
            Command::ToggleLock { id } => self.apply_toggle_lock(id),
            Command::ToggleFold { id } => self.apply_toggle_fold(id),
            Command::AutoArrange { id } => self.apply_auto_arrange(id, layout),
            Command::SnapAll => self.apply_snap_all(layout, bounds),
            Command::AddItem { space, path, facts } => {
                self.apply_add_item(space, path, facts, layout)
            }
            Command::RemoveItem { id } => self.apply_remove_item(id, layout),
            Command::MoveItem { id, target } => self.apply_move_item(id, target, layout),
            Command::ReorderItem { id, index } => self.apply_reorder_item(id, index, layout),
            Command::ReorderItems { ids, index } => self.apply_reorder_items(&ids, index, layout),
            Command::RenameItem { id, path, label } => self.apply_rename_item(id, path, label),
            Command::SyncDesktop { items } => self.apply_sync_desktop(items),
            Command::MoveToDesktop { item } => self.apply_move_to_desktop(item, layout),
            Command::SetDesktopItemPosition { item, position } => {
                self.apply_set_desktop_item_position(item, position)
            }
        }
    }

    /// Runs `change` on space `id`, or reports it missing.
    fn with_space(
        &mut self,
        id: SpaceId,
        change: impl FnOnce(&mut crate::space::Space) -> CommandOutcome,
    ) -> CommandOutcome {
        self.space_mut(id).map_or(CommandOutcome::NotFound, change)
    }

    /// Arranges space `id` to fit its items (a no-op when it is gone).
    fn rearrange(&mut self, id: SpaceId, layout: GridSpec) {
        if let Some(space) = self.space_mut(id) {
            arrange_space(space, layout);
        }
    }

    /// Closes the gap an item left in space `id`, without resizing the space
    /// under the user.
    fn close_gap(&mut self, id: SpaceId, layout: GridSpec) {
        if let Some(space) = self.space_mut(id) {
            position_items(space, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemKind;

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
                facts: ItemFacts::default(),
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
                    facts: ItemFacts::default(),
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
        // Width 480 fits four columns of 106 and rounds down to 438; height
        // 320 fits two rows and rounds down to 270, so the padding is even.
        assert_eq!(sized, Rect::new(0, 0, 438, 270));
        for index in 0..20 {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: format!("file{index}.txt").into(),
                    facts: ItemFacts::default(),
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
    fn a_kept_space_reflows_into_new_columns_instead_of_resizing() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 10, 10));
        for index in 0..6 {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: format!("file{index}.txt").into(),
                    facts: ItemFacts::default(),
                },
                bounds(),
            );
        }
        let auto = desktop.space(id).unwrap().rect;
        assert_eq!(
            desktop.apply(Command::KeepSize { id }, bounds()),
            CommandOutcome::Applied
        );
        assert!(desktop.space(id).unwrap().user_sized);

        let zoomed = desktop.layout.scaled(1.5);
        desktop.apply_with_layout(Command::Reflow { id }, zoomed, bounds());
        let space = desktop.space(id).unwrap();
        assert_eq!(space.rect, auto);
        assert_eq!(
            space.items[1].position.y,
            zoomed.grid_position_in(1, zoomed.columns_for(auto.width)).y
        );
    }

    #[test]
    fn keeping_the_size_of_a_missing_space_is_not_found() {
        let mut desktop = Desktop::new();
        assert_eq!(
            desktop.apply(Command::KeepSize { id: SpaceId(9) }, bounds()),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn a_resized_space_has_equal_side_padding() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(100, 0, 300, 300));
        let layout = desktop.layout;
        for name in ["one", "two", "three", "four", "five", "six"] {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: format!("{name}.txt").into(),
                    facts: ItemFacts::default(),
                },
                bounds(),
            );
        }
        let edges = ResizeEdges {
            right: true,
            ..ResizeEdges::NONE
        };
        assert_eq!(
            desktop.apply(
                Command::ResizeSpace {
                    id,
                    edges,
                    to: Point::new(100 + 560, 60),
                },
                bounds(),
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        // The width is exactly a whole number of columns, so the padding is
        // equal on both sides instead of the remainder landing on the right.
        assert_eq!(space.rect.width, layout.fit_width(space.rect.width));
        let columns = layout.columns_for(space.rect.width);
        let last_column_x = layout.padding + (columns - 1) * (layout.cell_width + layout.gap);
        assert_eq!(space.items.first().unwrap().position.x, layout.padding);
        assert_eq!(
            space.rect.width - (last_column_x + layout.cell_width),
            layout.padding,
            "left and right padding match"
        );
    }

    #[test]
    fn a_resized_space_has_equal_top_and_bottom_padding() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 300, 300));
        let layout = desktop.layout;
        let edges = ResizeEdges {
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert_eq!(
            desktop.apply(
                Command::ResizeSpace {
                    id,
                    edges,
                    to: Point::new(300, 320),
                },
                bounds(),
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.rect.height, layout.fit_height(space.rect.height));
        let rows = layout.rows_for(space.rect.height);
        let last_row_y =
            layout.title_height + layout.padding + (rows - 1) * (layout.cell_height + layout.gap);
        assert_eq!(
            space.rect.height - (last_row_y + layout.cell_height),
            layout.padding,
            "top and bottom padding match"
        );
    }

    #[test]
    fn fit_to_grid_can_be_turned_off() {
        let mut desktop = Desktop::new();
        desktop.layout.fit_to_grid = false;
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let edges = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        desktop.apply(
            Command::ResizeSpace {
                id,
                edges,
                to: Point::new(480, 320),
            },
            bounds(),
        );
        // Without the fit the size keeps the dragged dimensions (snapped to
        // `snap`), not whole columns and rows.
        let rect = desktop.space(id).unwrap().rect;
        assert_eq!(rect, Rect::new(0, 0, 480, 320));
        assert_ne!(rect.width, desktop.layout.fit_width(rect.width));
        assert_ne!(rect.height, desktop.layout.fit_height(rect.height));
    }

    #[test]
    fn locked_and_folded_spaces_reject_resize() {
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
        desktop.space_mut(id).unwrap().folded = true;
        assert_eq!(
            desktop.apply(resize.clone(), bounds()),
            CommandOutcome::Rejected
        );
        desktop.space_mut(id).unwrap().folded = false;
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
                    facts: ItemFacts::default(),
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

    #[test]
    fn delete_and_rename_missing_space_are_not_found() {
        let mut desktop = Desktop::new();
        let missing = SpaceId(999);
        assert_eq!(
            desktop.apply(Command::DeleteSpace { id: missing }, bounds()),
            CommandOutcome::NotFound
        );
        assert_eq!(
            desktop.apply(
                Command::RenameSpace {
                    id: missing,
                    title: "B".into(),
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn rename_space_sets_the_title() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        desktop.apply(
            Command::RenameSpace {
                id,
                title: "Projects".into(),
            },
            bounds(),
        );
        assert_eq!(desktop.space(id).unwrap().title, "Projects");
    }

    #[test]
    fn move_space_snaps_and_stays_inside_the_bounds() {
        let mut desktop = Desktop::new();
        let snap = desktop.layout.snap;
        let id = desktop.add_space("A", Rect::new(0, 0, 200, 100));
        desktop.apply(
            Command::MoveSpace {
                id,
                delta: Point::new(snap + 1, 5000),
            },
            bounds(),
        );
        let rect = desktop.space(id).unwrap().rect;
        assert_eq!(rect.x % snap, 0);
        assert_eq!(
            rect.y + rect.height,
            bounds().height,
            "clamped to the bottom"
        );
    }

    #[test]
    fn set_space_rect_snaps_and_respects_the_lock() {
        let mut desktop = Desktop::new();
        let snap = desktop.layout.snap;
        let id = desktop.add_space("A", Rect::new(0, 0, 200, 100));
        let set = |x| Command::SetSpaceRect {
            id,
            rect: Rect::new(x, 0, 200, 100),
        };
        assert_eq!(
            desktop.apply(set(snap * 3 + 1), bounds()),
            CommandOutcome::Applied
        );
        assert_eq!(desktop.space(id).unwrap().rect.x % snap, 0);
        desktop.apply(Command::ToggleLock { id }, bounds());
        assert_eq!(desktop.apply(set(0), bounds()), CommandOutcome::Rejected);
    }

    #[test]
    fn toggle_fold_flips_both_ways() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        desktop.apply(Command::ToggleFold { id }, bounds());
        assert!(desktop.space(id).unwrap().folded);
        desktop.apply(Command::ToggleFold { id }, bounds());
        assert!(!desktop.space(id).unwrap().folded);
    }

    #[test]
    fn auto_arrange_lays_items_on_the_grid() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "a", ItemKind::File, 0, 0),
            item(2, "b", ItemKind::File, 0, 0),
        ]);
        desktop.space_mut(id).unwrap().items[1].position = Point::new(999, 999);
        assert_eq!(
            desktop.apply(Command::AutoArrange { id }, bounds()),
            CommandOutcome::Applied
        );
        assert_eq!(
            desktop.space(id).unwrap().items[1].position,
            desktop.layout.grid_position(1)
        );
        assert_eq!(
            desktop.apply(Command::AutoArrange { id: SpaceId(999) }, bounds()),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn rename_item_updates_path_and_label() {
        let (mut desktop, id) = space_with_items(vec![item(1, "a", ItemKind::File, 0, 0)]);
        let item_id = desktop.space(id).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(
                Command::RenameItem {
                    id: item_id,
                    path: "C:/missing/b".into(),
                    label: "b".into(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let renamed = desktop.item(item_id).unwrap();
        assert_eq!(renamed.label, "b");
        assert_eq!(renamed.path, PathBuf::from("C:/missing/b"));
    }

    #[test]
    fn reorder_items_rejects_an_empty_selection() {
        let (mut desktop, _) = space_with_items(vec![item(1, "a", ItemKind::File, 0, 0)]);
        assert_eq!(
            desktop.apply(
                Command::ReorderItems {
                    ids: Vec::new(),
                    index: 0,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
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
        let mut item = Item::new(
            ItemId(id),
            format!("C:/missing/{label}"),
            ItemFacts::default(),
        );
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
                    facts: ItemFacts::default(),
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
    fn set_space_view_works_on_spaces_and_lenses() {
        use crate::view::ViewMode;

        let mut desktop = Desktop::new();
        let normal = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        let lens = desktop.next_space_id();
        assert!(
            desktop
                .apply(
                    Command::CreateLens {
                        title: "L".into(),
                        at: Point::new(200, 0),
                        size: Size::new(100, 100),
                        path: "C:/missing".into(),
                        locked: false,
                    },
                    bounds()
                )
                .is_applied()
        );
        for id in [normal, lens] {
            assert_eq!(
                desktop.apply(
                    Command::SetSpaceView {
                        id,
                        view: SpaceView {
                            mode: ViewMode::LargeIcons
                        },
                    },
                    bounds()
                ),
                CommandOutcome::Applied
            );
            assert_eq!(desktop.space(id).unwrap().view().mode, ViewMode::LargeIcons);
        }
        assert_eq!(
            desktop.apply(
                Command::SetSpaceView {
                    id: SpaceId(999),
                    view: SpaceView::default(),
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
            let mut item = Item::new(
                ItemId(0),
                format!("C:/missing/{name}"),
                ItemFacts::default(),
            );
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
        let item = Item::new(ItemId(1), "C:/missing/a.txt", ItemFacts::default());
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
        desktop.layer.push_item(Item::new(
            ItemId(1),
            "C:/missing/a.txt",
            ItemFacts::default(),
        ));
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

    /// Three spaces, A, B, and C in that order, with one item in B.
    fn three_spaces() -> (Desktop, [SpaceId; 3], ItemId) {
        let mut desktop = Desktop::new();
        let ids = ["A", "B", "C"].map(|title| {
            let id = desktop.next_space_id();
            let create = Command::CreateSpace {
                title: title.into(),
                at: Point::new(40, 40),
                size: Size::new(240, 120),
                locked: false,
            };
            assert!(desktop.apply(create, bounds()).is_applied());
            id
        });
        let item = desktop.next_item_id();
        let add = Command::AddItem {
            space: ids[1],
            path: "C:/missing/b.txt".into(),
            facts: ItemFacts::default(),
        };
        assert!(desktop.apply(add, bounds()).is_applied());
        (desktop, ids, item)
    }

    /// `id`'s state as an undo entry records it: its index and a copy.
    fn recorded(desktop: &Desktop, id: SpaceId) -> (SpaceId, Option<(usize, Space)>) {
        let index = desktop.spaces.iter().position(|space| space.id == id);
        (
            id,
            index.map(|index| (index, desktop.spaces[index].clone())),
        )
    }

    fn order(desktop: &Desktop) -> Vec<SpaceId> {
        desktop.spaces.iter().map(|space| space.id).collect()
    }

    #[test]
    fn a_deleted_space_id_is_not_given_to_a_new_space() {
        let (mut desktop, [_, _, c], _) = three_spaces();
        desktop.apply(Command::DeleteSpace { id: c }, bounds());
        assert_ne!(desktop.next_space_id(), c);
        assert!(desktop.next_space_id() > c);
    }

    #[test]
    fn a_removed_item_id_is_not_given_to_a_new_item() {
        let (mut desktop, [a, ..], item) = three_spaces();
        desktop.apply(Command::RemoveItem { id: item }, bounds());
        let next = desktop.next_item_id();
        assert!(next > item);
        let add = Command::AddItem {
            space: a,
            path: "C:/missing/new.txt".into(),
            facts: ItemFacts::default(),
        };
        desktop.apply(add, bounds());
        assert_eq!(desktop.spaces[0].items()[0].id, next);
    }

    #[test]
    fn a_new_loose_file_does_not_take_a_removed_item_id() {
        let (mut desktop, _, item) = three_spaces();
        desktop.apply(Command::RemoveItem { id: item }, bounds());
        let sync = Command::SyncDesktop {
            items: vec![Item::new(
                ItemId(0),
                "C:/missing/loose.txt",
                ItemFacts::default(),
            )],
        };
        desktop.apply(sync, bounds());
        assert!(desktop.layer().items()[0].id > item);
    }

    #[test]
    fn restoring_a_deleted_space_brings_it_back_in_its_place() {
        let (mut desktop, [a, b, c], item) = three_spaces();
        desktop.apply(Command::ToggleLock { id: b }, bounds());
        let before = recorded(&desktop, b);
        let original = before.1.clone().unwrap().1;
        desktop.apply(Command::DeleteSpace { id: b }, bounds());

        let outcome = desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![before],
            },
            bounds(),
        );
        assert_eq!(outcome, CommandOutcome::Applied);
        assert_eq!(order(&desktop), [a, b, c]);
        assert_eq!(desktop.space(b), Some(&original));
        assert!(desktop.space(b).unwrap().locked());
        assert_eq!(desktop.space_containing(item), Some(b));
    }

    #[test]
    fn restoring_over_an_existing_space_replaces_it_in_place() {
        let (mut desktop, [a, b, c], _) = three_spaces();
        let before = recorded(&desktop, b);
        desktop.apply(
            Command::RenameSpace {
                id: b,
                title: "Renamed".into(),
            },
            bounds(),
        );
        desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![before],
            },
            bounds(),
        );
        assert_eq!(order(&desktop), [a, b, c]);
        assert_eq!(desktop.space(b).unwrap().title(), "B");
    }

    #[test]
    fn restoring_several_spaces_keeps_each_index() {
        let (mut desktop, [a, b, c], _) = three_spaces();
        let before = vec![recorded(&desktop, c), recorded(&desktop, a)];
        desktop.apply(Command::DeleteSpace { id: a }, bounds());
        desktop.apply(Command::DeleteSpace { id: c }, bounds());
        desktop.apply(Command::RestoreSpaces { spaces: before }, bounds());
        assert_eq!(order(&desktop), [a, b, c]);
    }

    #[test]
    fn restoring_absence_removes_a_space_and_is_harmless_when_gone() {
        let (mut desktop, [a, b, c], _) = three_spaces();
        let remove = Command::RestoreSpaces {
            spaces: vec![(b, None)],
        };
        assert!(desktop.apply(remove.clone(), bounds()).is_applied());
        assert_eq!(order(&desktop), [a, c]);
        assert!(desktop.apply(remove, bounds()).is_applied());
        assert_eq!(order(&desktop), [a, c]);
    }

    #[test]
    fn a_restored_space_keeps_its_ids_from_being_reused() {
        let (mut desktop, [_, b, _], item) = three_spaces();
        let before = recorded(&desktop, b);
        desktop.apply(Command::DeleteSpace { id: b }, bounds());
        desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![before],
            },
            bounds(),
        );
        desktop.apply(Command::DeleteSpace { id: b }, bounds());
        assert!(desktop.next_item_id() > item);
    }

    #[test]
    fn a_restore_whose_item_is_held_elsewhere_changes_nothing() {
        let (mut desktop, [a, b, _], item) = three_spaces();
        let before = recorded(&desktop, b);
        desktop.apply(
            Command::MoveItem {
                id: item,
                target: a,
            },
            bounds(),
        );
        desktop.apply(Command::DeleteSpace { id: b }, bounds());
        let unchanged = desktop.clone();
        let outcome = desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![before],
            },
            bounds(),
        );
        assert_eq!(outcome, CommandOutcome::Rejected);
        assert_eq!(desktop, unchanged);
    }

    #[test]
    fn a_restore_whose_item_is_on_the_layer_changes_nothing() {
        let (mut desktop, [_, b, _], item) = three_spaces();
        let before = recorded(&desktop, b);
        desktop.apply(Command::MoveToDesktop { item }, bounds());
        desktop.apply(Command::DeleteSpace { id: b }, bounds());
        let outcome = desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![before],
            },
            bounds(),
        );
        assert_eq!(outcome, CommandOutcome::Rejected);
        assert!(desktop.space(b).is_none());
    }

    #[test]
    fn a_restore_listing_an_id_twice_or_the_wrong_space_changes_nothing() {
        let (mut desktop, [a, b, _], _) = three_spaces();
        let unchanged = desktop.clone();
        let twice = vec![recorded(&desktop, b), (b, None)];
        let (_, wrong) = recorded(&desktop, b);
        for spaces in [twice, vec![(a, wrong)]] {
            let outcome = desktop.apply(Command::RestoreSpaces { spaces }, bounds());
            assert_eq!(outcome, CommandOutcome::Rejected);
            assert_eq!(desktop, unchanged);
        }
    }
}
