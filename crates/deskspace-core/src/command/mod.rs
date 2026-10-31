//! The command layer: every change to the domain model is a `Command` applied
//! with `Desktop::apply`.
//!
//! `apply_with_layout` only dispatches; each command is a private `Desktop::apply_<variant>`
//! method in the submodule for what it changes. To add a command, add the
//! variant here, one dispatch arm, and the method beside its neighbours.

mod item_ops;
mod layer_ops;
mod lens_ops;
mod space_ops;
mod touched;

use std::path::PathBuf;

use crate::desktop::Desktop;
use crate::geometry::{Point, Rect, Size};
use crate::icon::SpaceIcon;
use crate::id::{ItemId, SpaceId};
use crate::item::{Item, ItemFacts};
use crate::layout::{GridSpec, arrange_space, position_items};
use crate::sort::SortRule;
use crate::source::{FolderEntry, FolderView, LensTitle};
use crate::space::{ResizeEdges, Space, SpaceLook};
use crate::view::SpaceView;
use crate::view_layout::ViewMetrics;

pub use crate::item::SortKey;
pub use touched::Touched;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Creates a space at `at` (its top-left in desktop coordinates) with the
    /// configured `size`, optionally starting locked. `view` is the new
    /// space's view settings (W40).
    CreateSpace {
        title: String,
        at: Point,
        size: Size,
        locked: bool,
        icon: SpaceIcon,
        view: SpaceView,
    },
    /// Creates a lens: a space that mirrors `path` live (ROADMAP D3, M4).
    /// `path` is only stored; `SyncLens` fills the items. `view` is the new
    /// lens's view settings (W19, W23).
    CreateLens {
        title: String,
        at: Point,
        size: Size,
        path: PathBuf,
        locked: bool,
        view: SpaceView,
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
    /// Sets how a space lays its items out (D17). Works on a normal space and
    /// a lens alike; the view is separate from the look.
    SetSpaceView {
        id: SpaceId,
        view: SpaceView,
    },
    /// Sets a space's icon. A locked space still takes it, as it takes a
    /// rename.
    SetSpaceIcon {
        id: SpaceId,
        icon: SpaceIcon,
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
    /// `metrics` are the cells its view draws, which only the app can work
    /// out; with `fit_to_grid`, they decide the rows and columns it fits.
    ResizeSpace {
        id: SpaceId,
        edges: ResizeEdges,
        to: Point,
        metrics: ViewMetrics,
    },
    /// Drops a user size so the space grows to fit its items again. A view
    /// that places items by order sizes the space once, from `metrics`: the
    /// cells the space is drawn with, which only the app can work out.
    FitToContents {
        id: SpaceId,
        metrics: ViewMetrics,
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
    /// Reorders a space's items by `rule`, keeps the rule for the space's
    /// next sort, and re-lays them out.
    SortSpace {
        id: SpaceId,
        rule: SortRule,
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
    /// Rejected on a space that keeps sorted (D17).
    ReorderItem {
        id: ItemId,
        index: usize,
    },
    /// Moves several items (a drag of a multi-selection) to `index`, keeping
    /// their relative order. Rejected on a space that keeps sorted (D17).
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
            Command::SetSpaceView { id, view } => self.apply_set_space_view(id, view, layout),
            Command::SetSpaceIcon { id, icon } => self.apply_set_space_icon(id, icon),
            Command::CreateSpace {
                title,
                at,
                size,
                locked,
                icon,
                view,
            } => self.apply_create_space(
                title,
                Rect::new(at.x, at.y, size.width, size.height),
                locked,
                icon,
                view,
                bounds,
            ),
            Command::DeleteSpace { id } => self.apply_delete_space(id),
            Command::RenameSpace { id, title } => self.apply_rename_space(id, title),
            Command::CreateLens {
                title,
                at,
                size,
                path,
                locked,
                view,
            } => self.apply_create_lens(
                title,
                Rect::new(at.x, at.y, size.width, size.height),
                path,
                locked,
                view,
                bounds,
            ),
            Command::SetLensFolder { id, path, title } => {
                self.apply_set_lens_folder(id, path, title)
            }
            Command::SetLensTitle { id, title, text } => self.apply_set_lens_title(id, title, text),
            Command::SetLensView { id, view } => self.apply_set_lens_view(id, view),
            Command::SyncLens { id, entries } => self.apply_sync_lens(id, entries, layout),
            Command::MoveSpace { id, delta } => self.apply_move_space(id, delta, layout, bounds),
            Command::SetSpaceRect { id, rect } => {
                self.apply_set_space_rect(id, rect, layout, bounds)
            }
            Command::ResizeSpace {
                id,
                edges,
                to,
                metrics,
            } => self.apply_resize_space(id, edges, to, metrics, layout, bounds),
            Command::FitToContents { id, metrics } => {
                self.apply_fit_to_contents(id, metrics, layout, bounds)
            }
            Command::KeepSize { id } => self.apply_keep_size(id),
            Command::Reflow { id } => self.apply_reflow(id, layout),
            Command::SortSpace { id, rule } => self.apply_sort_space(id, rule, layout),
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
            Command::RenameItem { id, path, label } => {
                self.apply_rename_item(id, path, label, layout)
            }
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

    /// Arranges space `id` to fit its items (a no-op when it is gone), holding
    /// a kept sort first.
    fn rearrange(&mut self, id: SpaceId, layout: GridSpec) {
        if let Some(space) = self.space_mut(id) {
            let sorted = hold_kept_order(space);
            arrange_space(space, layout);
            // `arrange_space` leaves a locked space alone, so a held order
            // still needs its items placed.
            if sorted && space.locked {
                position_items(space, layout);
            }
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

/// Puts a space's items in its kept order after one changed, and reports
/// whether it sorted. A lens always keeps sorted; a normal space when its
/// view asks (D17).
fn hold_kept_order(space: &mut Space) -> bool {
    if !space.keeps_sorted() {
        return false;
    }
    space.view.sort.sort_items(&mut space.items);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemKind;
    use crate::view::ViewMode;
    use crate::view_layout::ViewLayout;

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
                icon: SpaceIcon::None,
                view: SpaceView::default(),
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
                icon: SpaceIcon::None,
                view: SpaceView::default(),
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
                metrics: ViewMetrics::for_grid(desktop.layout),
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
        let layout = crate::view_layout::ViewLayout::for_space(
            space,
            crate::view_layout::ViewMetrics::for_grid(desktop.layout),
        );
        assert!(layout.max_scroll(space.content_rect().size()) > 0);

        desktop.apply(
            Command::FitToContents {
                id,
                metrics: ViewMetrics::for_grid(desktop.layout),
            },
            bounds(),
        );
        assert!(!desktop.space(id).unwrap().user_sized);
        assert_ne!(desktop.space(id).unwrap().rect.size(), sized.size());
    }

    /// A space of `items` items shown as `mode`, at `rect`.
    fn space_in_view(mode: ViewMode, rect: Rect, items: usize) -> (Desktop, SpaceId) {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", rect);
        for index in 0..items {
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: format!("file{index}.txt").into(),
                    facts: ItemFacts::default(),
                },
                bounds(),
            );
        }
        desktop.apply(
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(mode),
            },
            bounds(),
        );
        // Adding items sized the space to them, so size it as a user would.
        let space = desktop.space_mut(id).unwrap();
        space.rect = rect;
        space.user_sized = true;
        (desktop, id)
    }

    /// Fits the space and returns its rect and how far it can still scroll.
    fn fit(desktop: &mut Desktop, id: SpaceId, mode: ViewMode) -> (Rect, i32) {
        let metrics =
            ViewMetrics::for_view(&SpaceView::default().with_mode(mode), desktop.layout, 1.0);
        assert!(
            desktop
                .apply(Command::FitToContents { id, metrics }, bounds())
                .is_applied()
        );
        let space = desktop.space(id).unwrap();
        let layout = ViewLayout::for_space(space, metrics);
        (space.rect, layout.max_scroll(space.content_rect().size()))
    }

    #[test]
    fn fitting_a_row_view_keeps_the_width_and_hugs_the_rows() {
        for mode in [
            ViewMode::SmallIcons,
            ViewMode::Details,
            ViewMode::Tiles,
            ViewMode::Content,
        ] {
            let tall = Rect::new(10, 10, 500, 700);
            let (mut desktop, id) = space_in_view(mode, tall, 3);
            let (rect, scroll) = fit(&mut desktop, id, mode);
            assert_eq!(rect.width, tall.width, "{mode:?}");
            assert!(rect.height < tall.height, "{mode:?}: {rect:?}");
            assert_eq!(scroll, 0, "{mode:?}: every row shows");
            assert_eq!(rect.origin(), tall.origin(), "{mode:?}");
        }
    }

    #[test]
    fn fitting_a_row_view_stops_at_the_work_area() {
        let (mut desktop, id) =
            space_in_view(ViewMode::SmallIcons, Rect::new(10, 300, 300, 200), 500);
        let (rect, scroll) = fit(&mut desktop, id, ViewMode::SmallIcons);
        assert_eq!(rect.height, bounds().height);
        assert_eq!(rect.y, bounds().y, "moved up to stay on the work area");
        assert!(scroll > 0, "the rest scrolls");
    }

    #[test]
    fn fitting_list_keeps_the_height_and_hugs_the_columns() {
        let wide = Rect::new(10, 10, 900, 200);
        let (mut desktop, id) = space_in_view(ViewMode::List, wide, 3);
        let (rect, scroll) = fit(&mut desktop, id, ViewMode::List);
        assert_eq!(rect.height, wide.height);
        assert!(rect.width < wide.width, "a few items shrink it: {rect:?}");
        assert_eq!(scroll, 0);

        let narrow = Rect::new(10, 10, 210, 200);
        let (mut desktop, id) = space_in_view(ViewMode::List, narrow, 20);
        let (rect, scroll) = fit(&mut desktop, id, ViewMode::List);
        assert_eq!(rect.height, narrow.height);
        assert!(rect.width > narrow.width, "more columns grow it: {rect:?}");
        assert_eq!(scroll, 0);
    }

    #[test]
    fn fitting_counts_the_status_bar_and_an_empty_space_keeps_one_row() {
        let tall = Rect::new(10, 10, 500, 700);
        let (mut desktop, id) = space_in_view(ViewMode::SmallIcons, tall, 0);
        let (empty, _) = fit(&mut desktop, id, ViewMode::SmallIcons);
        assert!(empty.height > desktop.space(id).unwrap().title_height);

        desktop.apply(
            Command::SetSpaceView {
                id,
                view: SpaceView {
                    status_bar: true,
                    ..SpaceView::default().with_mode(ViewMode::SmallIcons)
                },
            },
            bounds(),
        );
        let (with_bar, scroll) = fit(&mut desktop, id, ViewMode::SmallIcons);
        assert_eq!(
            with_bar.height - empty.height,
            desktop.space(id).unwrap().status_bar_height()
        );
        assert_eq!(scroll, 0);
    }

    #[test]
    fn a_grouped_space_refuses_a_rearrange_and_leaves_positions_alone() {
        let (mut desktop, id) = space_in_view(ViewMode::Icons, Rect::new(10, 10, 500, 400), 3);
        let before: Vec<Point> = desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .map(|item| item.position)
            .collect();
        let item = desktop.space(id).unwrap().items()[0].id;
        desktop.apply(
            Command::SetSpaceView {
                id,
                view: SpaceView {
                    group: Some(crate::view::GroupBy {
                        key: crate::item::SortKey::Name,
                        descending: false,
                    }),
                    ..SpaceView::default()
                },
            },
            bounds(),
        );
        assert_eq!(
            desktop.apply(Command::ReorderItem { id: item, index: 2 }, bounds()),
            CommandOutcome::Rejected
        );
        desktop.apply(Command::Reflow { id }, bounds());
        let after: Vec<Point> = desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .map(|item| item.position)
            .collect();
        assert_eq!(after, before, "a hand arrangement waits for ungrouping");
    }

    #[test]
    fn a_folded_space_fits_as_it_would_show() {
        let tall = Rect::new(10, 10, 500, 700);
        let (mut open, open_id) = space_in_view(ViewMode::Details, tall, 3);
        let (open_rect, _) = fit(&mut open, open_id, ViewMode::Details);

        let (mut folded, id) = space_in_view(ViewMode::Details, tall, 3);
        folded.apply(Command::ToggleFold { id }, bounds());
        let (rect, _) = fit(&mut folded, id, ViewMode::Details);
        assert_eq!(rect, open_rect);
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
                    metrics: ViewMetrics::for_grid(layout),
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
                    metrics: ViewMetrics::for_grid(layout),
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
    fn a_details_resize_fits_whole_rows_and_leaves_the_width_at_the_snap() {
        let (mut desktop, id) = space_in_view(ViewMode::Details, Rect::new(0, 0, 300, 300), 12);
        let layout = desktop.layout;
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Details),
            layout,
            1.0,
        );
        let edges = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert!(
            desktop
                .apply(
                    Command::ResizeSpace {
                        id,
                        edges,
                        to: Point::new(500, 400),
                        metrics,
                    },
                    bounds()
                )
                .is_applied()
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(
            space.rect,
            Rect::new(0, 0, 496, 385),
            "the width keeps the 16 px snap; the height holds whole Details rows"
        );
        assert_ne!(
            space.rect.height,
            layout.fit_height(space.rect.height),
            "the height is not rounded to the icon grid"
        );
    }

    #[test]
    fn a_list_resize_fits_its_own_cells() {
        let (mut desktop, id) = space_in_view(ViewMode::List, Rect::new(0, 0, 300, 300), 20);
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::List),
            desktop.layout,
            1.0,
        );
        let edges = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert!(
            desktop
                .apply(
                    Command::ResizeSpace {
                        id,
                        edges,
                        to: Point::new(500, 400),
                        metrics,
                    },
                    bounds()
                )
                .is_applied()
        );
        assert_eq!(
            desktop.space(id).unwrap().rect,
            Rect::new(0, 0, 426, 386),
            "whole 200 x 22 List cells, not 96 px icon cells"
        );
    }

    #[test]
    fn a_fitted_resize_stays_inside_the_work_area() {
        let (mut desktop, id) = space_in_view(ViewMode::List, Rect::new(0, 0, 200, 300), 20);
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::List),
            desktop.layout,
            1.0,
        );
        let edges = ResizeEdges {
            left: true,
            ..ResizeEdges::NONE
        };
        assert!(
            desktop
                .apply(
                    Command::ResizeSpace {
                        id,
                        edges,
                        to: Point::new(100, 0),
                        metrics,
                    },
                    bounds()
                )
                .is_applied()
        );
        let rect = desktop.space(id).unwrap().rect;
        assert_eq!(
            rect,
            Rect::new(0, 0, 224, 300),
            "the fit does not leave the screen"
        );
    }

    #[test]
    fn a_status_bar_is_not_clipped_by_the_height_fit() {
        let (mut desktop, id) = space_in_view(ViewMode::Details, Rect::new(0, 0, 300, 300), 12);
        let view = SpaceView {
            status_bar: true,
            ..SpaceView::default().with_mode(ViewMode::Details)
        };
        assert!(
            desktop
                .apply(
                    Command::SetSpaceView {
                        id,
                        view: view.clone()
                    },
                    bounds()
                )
                .is_applied()
        );
        {
            let space = desktop.space_mut(id).unwrap();
            space.rect = Rect::new(0, 0, 300, 300);
            space.user_sized = true;
        }
        let metrics = ViewMetrics::for_view(&view, desktop.layout, 1.0);
        let edges = ResizeEdges {
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert!(
            desktop
                .apply(
                    Command::ResizeSpace {
                        id,
                        edges,
                        to: Point::new(300, 400),
                        metrics,
                    },
                    bounds()
                )
                .is_applied()
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.status_bar_height(), 24);
        assert_eq!(
            space.rect.height, 387,
            "the bar and the header come off before the rows are fitted"
        );
        assert_ne!(
            space.rect.height,
            metrics.fit_height(400, space.title_height + metrics.header),
            "the bar is chrome, not a row"
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
                metrics: ViewMetrics::for_grid(desktop.layout),
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
            metrics: ViewMetrics::for_grid(desktop.layout),
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
    fn set_space_icon_sets_it_even_on_a_locked_space() {
        let mut desktop = Desktop::new();
        let id = desktop.add_space("A", Rect::new(0, 0, 100, 100));
        desktop.apply(Command::ToggleLock { id }, bounds());
        let icon = SpaceIcon::Chosen(crate::icon::IconRef::parse("fluent:games").unwrap());
        assert_eq!(
            desktop.apply(
                Command::SetSpaceIcon {
                    id,
                    icon: icon.clone()
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(desktop.space(id).unwrap().icon(), &icon);
    }

    #[test]
    fn set_space_icon_on_a_missing_space_is_not_found() {
        let mut desktop = Desktop::new();
        assert_eq!(
            desktop.apply(
                Command::SetSpaceIcon {
                    id: SpaceId(999),
                    icon: SpaceIcon::Automatic
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn create_space_sets_the_icon() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Games".into(),
                at: Point::new(0, 0),
                size: Size::new(240, 120),
                locked: false,
                icon: SpaceIcon::Automatic,
                view: SpaceView::default(),
            },
            bounds(),
        );
        assert_eq!(desktop.spaces[0].icon(), &SpaceIcon::Automatic);
    }

    #[test]
    fn create_space_takes_the_view() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Files".into(),
                at: Point::new(0, 0),
                size: Size::new(240, 120),
                locked: false,
                icon: SpaceIcon::None,
                view: SpaceView {
                    status_bar: true,
                    ..SpaceView::default().with_mode(crate::view::ViewMode::Details)
                },
            },
            bounds(),
        );
        assert_eq!(
            desktop.spaces[0].view().mode,
            crate::view::ViewMode::Details
        );
        assert!(desktop.spaces[0].view().status_bar);
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
                rule: SortRule {
                    key: SortKey::Name,
                    ..SortRule::default()
                },
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["Apple", "banana", "cherry"]);
        desktop.apply(
            Command::SortSpace {
                id,
                rule: SortRule {
                    key: SortKey::Name,
                    descending: true,
                    folders_first: false,
                },
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["cherry", "banana", "Apple"]);
    }

    #[test]
    fn sort_space_keeps_its_rule_and_puts_folders_first() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "big", ItemKind::File, 300, 1),
            item(2, "z-folder", ItemKind::Folder, 0, 2),
            item(3, "small", ItemKind::File, 10, 3),
        ]);
        let rule = SortRule {
            key: SortKey::Size,
            descending: true,
            folders_first: true,
        };
        desktop.apply(Command::SortSpace { id, rule }, bounds());
        assert_eq!(labels(&desktop, id), ["z-folder", "big", "small"]);
        assert_eq!(
            desktop.space(id).unwrap().view().sort,
            rule,
            "the space keeps the rule it sorted by"
        );
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
                rule: SortRule {
                    key: SortKey::Kind,
                    ..SortRule::default()
                },
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
                rule: SortRule {
                    key: SortKey::Size,
                    ..SortRule::default()
                },
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["small", "mid", "big"]);
        desktop.apply(
            Command::SortSpace {
                id,
                rule: SortRule {
                    key: SortKey::Modified,
                    descending: true,
                    folders_first: false,
                },
            },
            bounds(),
        );
        assert_eq!(labels(&desktop, id), ["small", "mid", "big"]);
    }

    #[test]
    fn an_order_view_keeps_the_grid_positions() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "a", ItemKind::File, 1, 1),
            item(2, "b", ItemKind::File, 1, 1),
        ]);
        assert_eq!(
            desktop.apply(Command::AutoArrange { id }, bounds()),
            CommandOutcome::Applied
        );
        let before: Vec<(ItemId, Point)> = desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .map(|item| (item.id, item.position))
            .collect();
        assert_eq!(
            desktop.apply(
                Command::SetSpaceView {
                    id,
                    view: SpaceView::default().with_mode(crate::view::ViewMode::SmallIcons),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        for (id, position) in &before {
            assert_eq!(
                desktop.item(*id).unwrap().position,
                *position,
                "switching views"
            );
        }
        assert_eq!(
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: "C:/missing/c".into(),
                    facts: ItemFacts::default(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        for (id, position) in &before {
            assert_eq!(
                desktop.item(*id).unwrap().position,
                *position,
                "an added item leaves the grid positions alone"
            );
        }
        let first = desktop.space(id).unwrap().items()[0].id;
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
        for (id, position) in &before {
            assert_eq!(
                desktop.item(*id).unwrap().position,
                *position,
                "rearranging by order"
            );
        }
    }

    fn keep_sorted(desktop: &mut Desktop, id: SpaceId, rule: SortRule) {
        assert_eq!(
            desktop.apply(
                Command::SetSpaceView {
                    id,
                    view: SpaceView {
                        keep_sorted: true,
                        sort: rule,
                        ..SpaceView::default()
                    },
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert!(desktop.space(id).unwrap().keeps_sorted());
    }

    #[test]
    fn turning_keep_sorted_on_sorts_the_space_at_once() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "c", ItemKind::File, 1, 1),
            item(2, "a", ItemKind::File, 1, 1),
            item(3, "b", ItemKind::File, 1, 1),
        ]);
        assert_eq!(labels(&desktop, id), ["c", "a", "b"]);
        keep_sorted(&mut desktop, id, SortRule::default());
        assert_eq!(labels(&desktop, id), ["a", "b", "c"]);
        assert_eq!(
            desktop.space(id).unwrap().items[0].position,
            desktop.layout.grid_position(0)
        );
    }

    #[test]
    fn a_kept_sorted_space_puts_a_new_item_in_order() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "b", ItemKind::File, 1, 1),
            item(2, "c", ItemKind::File, 1, 1),
        ]);
        keep_sorted(&mut desktop, id, SortRule::default());
        assert_eq!(
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: "C:/missing/a".into(),
                    facts: ItemFacts::default(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["a", "b", "c"]);
        assert_eq!(
            desktop.space(id).unwrap().items[0].position,
            desktop.layout.grid_position(0)
        );
    }

    #[test]
    fn a_kept_sorted_space_puts_a_moved_item_in_order() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "b", ItemKind::File, 1, 1),
            item(2, "c", ItemKind::File, 1, 1),
        ]);
        let source = desktop.add_space("B", Rect::new(500, 0, 400, 400));
        desktop.add_item(source, item(3, "a", ItemKind::File, 1, 1));
        keep_sorted(&mut desktop, id, SortRule::default());
        let moved = desktop.space(source).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(
                Command::MoveItem {
                    id: moved,
                    target: id
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["a", "b", "c"]);
    }

    #[test]
    fn a_kept_sorted_space_sorts_after_a_rename() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "b", ItemKind::File, 1, 1),
            item(2, "c", ItemKind::File, 1, 1),
        ]);
        keep_sorted(&mut desktop, id, SortRule::default());
        let first = desktop.space(id).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(
                Command::RenameItem {
                    id: first,
                    path: "C:/missing/z".into(),
                    label: "z".into(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["c", "z"]);
        assert_eq!(
            desktop.space(id).unwrap().items[0].position,
            desktop.layout.grid_position(0)
        );
    }

    #[test]
    fn a_kept_sorted_space_rejects_rearranging() {
        let (mut desktop, id) = space_with_items(vec![
            item(1, "a", ItemKind::File, 1, 1),
            item(2, "b", ItemKind::File, 1, 1),
        ]);
        keep_sorted(&mut desktop, id, SortRule::default());
        let first = desktop.space(id).unwrap().items[0].id;
        assert_eq!(
            desktop.apply(
                Command::ReorderItem {
                    id: first,
                    index: 1,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(
            desktop.apply(
                Command::ReorderItems {
                    ids: vec![first],
                    index: 1,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(labels(&desktop, id), ["a", "b"]);
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
                        view: crate::SpaceView::default(),
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
                        view: SpaceView::default().with_mode(ViewMode::LargeIcons),
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
                icon: SpaceIcon::None,
                view: SpaceView::default(),
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
