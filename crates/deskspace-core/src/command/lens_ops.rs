//! Commands that create and refresh a lens, a space that mirrors a folder.

use std::path::PathBuf;

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::geometry::Rect;
use crate::id::SpaceId;
use crate::item::Item;
use crate::layout::{GridSpec, arrange_space, clamp_to_bounds, position_items};
use crate::source::{FolderView, LensTitle, SpaceSource};
use crate::view::SpaceView;

impl Desktop {
    pub(super) fn apply_create_lens(
        &mut self,
        title: String,
        rect: Rect,
        path: PathBuf,
        locked: bool,
        view: SpaceView,
        bounds: Rect,
    ) -> CommandOutcome {
        let rect = clamp_to_bounds(rect, bounds);
        let id = self.add_space(title, rect);
        if let Some(space) = self.space_mut(id) {
            space.source = SpaceSource::Folder(FolderView {
                path,
                title: LensTitle::FolderName,
                ..FolderView::default()
            });
            space.locked = locked;
            space.view = view;
        }
        CommandOutcome::Applied
    }

    pub(super) fn apply_set_lens_folder(
        &mut self,
        id: SpaceId,
        path: PathBuf,
        title: String,
    ) -> CommandOutcome {
        let mut replaced = Vec::new();
        let outcome = self.with_space(id, |space| {
            let SpaceSource::Folder(view) = &mut space.source else {
                return CommandOutcome::Rejected;
            };
            // The items belong to the folder, so pointing a lens at the
            // folder it already shows keeps them. Their watch posts no new
            // listing for an unchanged path, so clearing would empty it for
            // good.
            if view.path != path {
                view.path = path;
                replaced = std::mem::take(&mut space.items);
            }
            space.title = title;
            CommandOutcome::Applied
        });
        self.retire_items(replaced);
        outcome
    }

    pub(super) fn apply_set_lens_title(
        &mut self,
        id: SpaceId,
        title: LensTitle,
        text: String,
    ) -> CommandOutcome {
        self.with_space(id, |space| {
            let SpaceSource::Folder(view) = &mut space.source else {
                return CommandOutcome::Rejected;
            };
            view.title = title;
            space.title = text;
            CommandOutcome::Applied
        })
    }

    pub(super) fn apply_set_lens_view(&mut self, id: SpaceId, view: FolderView) -> CommandOutcome {
        self.with_space(id, |space| {
            let SpaceSource::Folder(current) = &mut space.source else {
                return CommandOutcome::Rejected;
            };
            *current = view.clamped();
            CommandOutcome::Applied
        })
    }

    /// Swaps in `items`, a list a `LensBuilder` finished, and lays them out.
    /// This runs on the UI thread, so it does nothing that grows with the list
    /// but place it. A locked lens keeps its size, and its items still need
    /// places, as `Reflow` gives them. A lens that would need more than the
    /// work area stops growing there and scrolls (a big folder must never make
    /// a window millions of pixels tall).
    pub(super) fn apply_sync_lens(
        &mut self,
        id: SpaceId,
        items: Vec<Item>,
        layout: GridSpec,
        bounds: Rect,
    ) -> CommandOutcome {
        let Some(space) = self.space_mut(id) else {
            return CommandOutcome::NotFound;
        };
        if !space.is_lens() {
            return CommandOutcome::Rejected;
        }
        let replaced = std::mem::replace(&mut space.items, items);
        if space.locked {
            position_items(space, layout);
        } else {
            arrange_space(space, layout);
            if space.rect.width > bounds.width || space.rect.height > bounds.height {
                space.rect.width = space.rect.width.min(bounds.width);
                space.rect.height = space.rect.height.min(bounds.height);
                space.rect = clamp_to_bounds(space.rect, bounds);
                space.user_sized = true;
                position_items(space, layout);
            }
        }
        self.retire_items(replaced);
        CommandOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, CommandOutcome};
    use crate::geometry::{Point, Size};
    use crate::id::ItemId;
    use crate::item::{Item, ItemFacts, ItemKind, SortKey};
    use crate::lens_build::{LensBuilder, LensRules};
    use crate::sort::SortRule;
    use crate::source::FolderEntry;

    fn bounds() -> Rect {
        Rect::new(0, 0, 1920, 1080)
    }

    fn lens(desktop: &mut Desktop, path: &str) -> SpaceId {
        assert!(
            desktop
                .apply(
                    Command::CreateLens {
                        title: "Lens".into(),
                        at: Point::new(10, 10),
                        size: Size::new(240, 120),
                        path: path.into(),
                        locked: false,
                        view: SpaceView::default(),
                    },
                    bounds(),
                )
                .is_applied()
        );
        desktop.spaces()[0].id
    }

    fn entry(name: &str, kind: ItemKind) -> FolderEntry {
        FolderEntry {
            path: PathBuf::from(format!("C:/folder/{name}")),
            label: name.trim_end_matches(".txt").into(),
            kind,
            size: Some(1),
            modified: Some(2),
            created: Some(1),
            hidden: false,
        }
    }

    /// What the worker does for a listing, then what the UI thread does with
    /// the result.
    fn sync(desktop: &mut Desktop, id: SpaceId, entries: Vec<FolderEntry>) -> CommandOutcome {
        let ids = desktop.reserve_item_ids();
        let (rules, items) = desktop.space(id).map_or_else(
            || (LensRules::default(), Vec::new()),
            |space| {
                (
                    space.lens_rules().unwrap_or_default(),
                    space.items().to_vec(),
                )
            },
        );
        let items = LensBuilder::new(rules, ids, &items).listed(entries).items;
        desktop.apply(Command::SyncLens { id, items }, bounds())
    }

    fn labels(desktop: &Desktop, id: SpaceId) -> Vec<String> {
        desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .map(|item| item.label.clone())
            .collect()
    }

    #[test]
    fn creating_a_lens_stores_its_folder_and_lock() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/Users/me/Documents");
        let space = desktop.space(id).unwrap();
        assert!(space.is_lens());
        assert_eq!(
            space.folder_view().unwrap().path,
            PathBuf::from("C:/Users/me/Documents")
        );
        assert_eq!(
            space.folder_view().unwrap().title,
            LensTitle::FolderName,
            "a new lens follows its folder's name"
        );
        assert!(space.items().is_empty());
    }

    #[test]
    fn changing_a_lens_rule_leaves_its_order_to_the_worker() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(
            &mut desktop,
            id,
            vec![
                entry("b.txt", ItemKind::File),
                entry("a.txt", ItemKind::File),
            ],
        );
        assert_eq!(labels(&desktop, id), ["a", "b"]);
        assert_eq!(
            desktop.apply(
                Command::SetSpaceView {
                    id,
                    view: SpaceView {
                        keep_sorted: true,
                        sort: SortRule {
                            key: SortKey::Name,
                            descending: true,
                            folders_first: false,
                        },
                        ..SpaceView::default()
                    },
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(
            labels(&desktop, id),
            ["a", "b"],
            "the worker rebuilds the list under the new rule"
        );
    }

    #[test]
    fn a_lens_too_large_for_the_work_area_stops_growing_and_scrolls() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let items: Vec<Item> = (0..2_000)
            .map(|n| {
                Item::new(
                    ItemId(n),
                    format!("C:/folder/file{n}.txt"),
                    ItemFacts::default(),
                )
            })
            .collect();
        assert_eq!(
            desktop.apply(Command::SyncLens { id, items }, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.items().len(), 2_000);
        assert!(space.rect.width <= bounds().width);
        assert!(
            space.rect.height <= bounds().height,
            "a big lens stays inside the work area"
        );
        assert!(
            space.user_sized(),
            "the overflow pins the lens so later listings scroll"
        );
        assert!(
            space.items().last().unwrap().position.y > space.content_rect().height,
            "the rest of the list is below the visible content"
        );
    }

    #[test]
    fn a_lens_that_fits_still_sizes_to_its_items() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let items = vec![Item::new(
            ItemId(1),
            "C:/folder/a.txt",
            ItemFacts::default(),
        )];
        assert_eq!(
            desktop.apply(Command::SyncLens { id, items }, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert!(!space.user_sized(), "a lens that fits stays auto-sized");
        assert!(space.rect.height < bounds().height);
    }

    #[test]
    fn a_lens_always_keeps_sorted() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        assert!(desktop.space(id).unwrap().keeps_sorted());

        sync(
            &mut desktop,
            id,
            vec![
                entry("b.txt", ItemKind::File),
                entry("a.txt", ItemKind::File),
            ],
        );
        let first = desktop.space(id).unwrap().items()[0].id;
        assert_eq!(labels(&desktop, id), ["a", "b"], "the lens is in order");
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
        assert_eq!(labels(&desktop, id), ["a", "b"], "the order held");
    }

    #[test]
    fn a_normal_space_does_not_keep_sorted_until_asked() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::ZERO,
                size: Size::new(240, 120),
                locked: false,
                icon: crate::icon::SpaceIcon::None,
                view: crate::view::SpaceView::default(),
            },
            bounds(),
        );
        let id = desktop.spaces()[0].id;
        assert!(!desktop.space(id).unwrap().keeps_sorted());
    }

    #[test]
    fn creating_a_lens_clamps_it_to_the_bounds() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateLens {
                title: "Lens".into(),
                at: Point::new(5000, 5000),
                size: Size::new(240, 120),
                path: "C:/folder".into(),
                locked: true,
                view: SpaceView::default(),
            },
            bounds(),
        );
        let rect = desktop.space(desktop.spaces()[0].id).unwrap().rect();
        assert!(rect.x + rect.width <= bounds().width);
        assert!(rect.y + rect.height <= bounds().height);
    }

    #[test]
    fn creating_a_lens_keeps_its_view() {
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        let view = SpaceView {
            status_bar: true,
            ..SpaceView::default()
        };
        assert!(
            desktop
                .apply(
                    Command::CreateLens {
                        title: "Lens".into(),
                        at: Point::new(10, 10),
                        size: Size::new(240, 120),
                        path: "C:/folder".into(),
                        locked: false,
                        view: view.clone(),
                    },
                    bounds(),
                )
                .is_applied()
        );
        assert_eq!(*desktop.space(id).unwrap().view(), view);
    }

    #[test]
    fn sync_lens_keeps_ids_for_paths_that_survive() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(
            &mut desktop,
            id,
            vec![
                entry("b.txt", ItemKind::File),
                entry("a.txt", ItemKind::File),
            ],
        );
        let a = desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .find(|item| item.label == "a")
            .unwrap()
            .id;
        let b = desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .find(|item| item.label == "b")
            .unwrap()
            .id;

        sync(
            &mut desktop,
            id,
            vec![
                entry("a.txt", ItemKind::File),
                entry("c.txt", ItemKind::File),
            ],
        );
        let items = desktop.space(id).unwrap().items();
        assert_eq!(labels(&desktop, id), ["a", "c"]);
        assert_eq!(items[0].id, a);
        assert!(items.iter().all(|item| item.id != b), "b is gone");
        assert!(items[1].id > b, "c gets a fresh id above the old ones");
    }

    #[test]
    fn sync_lens_sorts_names_naturally() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(
            &mut desktop,
            id,
            vec![
                entry("file10.txt", ItemKind::File),
                entry("file2.txt", ItemKind::File),
            ],
        );
        assert_eq!(labels(&desktop, id), ["file2", "file10"]);
    }

    #[test]
    fn sync_lens_refreshes_an_existing_items_facts() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        let mut changed = entry("a.txt", ItemKind::Folder);
        changed.label = "Renamed".into();
        changed.size = Some(99);
        changed.modified = Some(7);
        changed.created = Some(6);
        sync(&mut desktop, id, vec![changed]);
        let item = &desktop.space(id).unwrap().items()[0];
        assert_eq!(item.label, "Renamed");
        assert_eq!(item.kind, ItemKind::Folder);
        assert_eq!((item.size, item.modified), (Some(99), Some(7)));
        assert_eq!(item.created, Some(6));
    }

    #[test]
    fn sync_lens_grows_an_auto_space_to_fit() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let entries = (0..6)
            .map(|index| entry(&format!("file{index}.txt"), ItemKind::File))
            .collect();
        sync(&mut desktop, id, entries);
        let space = desktop.space(id).unwrap();
        assert!(space.rect().height > 120, "the lens grew to fit");
        assert_eq!(space.items()[0].position, desktop.layout.grid_position(0));
    }

    #[test]
    fn sync_lens_filters_hidden_entries_unless_asked() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let mut hidden = entry("hidden.txt", ItemKind::File);
        hidden.hidden = true;
        sync(
            &mut desktop,
            id,
            vec![entry("shown.txt", ItemKind::File), hidden.clone()],
        );
        assert_eq!(labels(&desktop, id), ["shown"]);

        let mut view = desktop.space(id).unwrap().folder_view().unwrap().clone();
        view.show_hidden = true;
        desktop.apply(Command::SetLensView { id, view }, bounds());
        sync(
            &mut desktop,
            id,
            vec![entry("shown.txt", ItemKind::File), hidden],
        );
        assert_eq!(labels(&desktop, id), ["hidden", "shown"]);
    }

    #[test]
    fn sync_lens_ignores_duplicate_paths() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(
            &mut desktop,
            id,
            vec![
                entry("a.txt", ItemKind::File),
                entry("a.txt", ItemKind::File),
            ],
        );
        assert_eq!(labels(&desktop, id).len(), 1);
    }

    #[test]
    fn sync_lens_places_the_items_of_a_locked_lens_and_keeps_its_size() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        desktop.apply(Command::ToggleLock { id }, bounds());
        let rect = desktop.space(id).unwrap().rect();
        let entries = (0..6)
            .map(|index| entry(&format!("file{index}.txt"), ItemKind::File))
            .collect();
        sync(&mut desktop, id, entries);
        let space = desktop.space(id).unwrap();
        assert_eq!(space.rect(), rect);
        assert_eq!(space.items()[5].position, desktop.layout.grid_position(5));
    }

    #[test]
    fn replaced_lists_wait_to_be_freed_off_the_ui_thread() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        assert!(desktop.take_retired_items().is_empty(), "nothing replaced");
        sync(&mut desktop, id, vec![entry("b.txt", ItemKind::File)]);
        let retired = desktop.take_retired_items();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0][0].label, "a");
        desktop.apply(
            Command::SetLensFolder {
                id,
                path: "C:/other".into(),
                title: "Other".into(),
            },
            bounds(),
        );
        assert_eq!(desktop.take_retired_items()[0][0].label, "b");
    }

    #[test]
    fn a_reserved_block_is_never_handed_out_again() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        let high = desktop.space(id).unwrap().items()[0].id;
        let block = desktop.reserve_item_ids();
        assert!(block.next > high.0, "above every id present");
        let above = desktop.next_item_id();
        assert!(above.0 >= block.end, "above the whole block");
        assert!(desktop.reserve_item_ids().next >= block.end);
    }

    /// The state undo stores for lens `id`, as `App` records it.
    fn undo_state(
        desktop: &Desktop,
        id: SpaceId,
    ) -> Vec<(SpaceId, Option<(usize, crate::space::Space)>)> {
        vec![(id, Some((0, desktop.space(id).unwrap().undo_snapshot())))]
    }

    #[test]
    fn a_normal_spaces_snapshot_keeps_its_items_and_a_lenss_does_not() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        let space = desktop.space(id).unwrap();
        let snapshot = space.undo_snapshot();
        assert!(snapshot.items().is_empty());
        assert_eq!(snapshot.title(), space.title());
        assert_eq!(snapshot.folder_view(), space.folder_view());

        let normal = desktop.next_space_id();
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::new(400, 0),
                size: Size::new(240, 120),
                locked: false,
                icon: crate::icon::SpaceIcon::None,
                view: SpaceView::default(),
            },
            bounds(),
        );
        desktop.apply(
            Command::AddItem {
                space: normal,
                path: "C:/x/a.txt".into(),
                facts: ItemFacts::default(),
            },
            bounds(),
        );
        let space = desktop.space(normal).unwrap();
        assert_eq!(&space.undo_snapshot(), space);
    }

    #[test]
    fn restoring_a_lens_on_the_same_folder_keeps_the_items_it_holds() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        let spaces = undo_state(&desktop, id);
        sync(&mut desktop, id, vec![entry("new.txt", ItemKind::File)]);
        desktop.take_retired_items();

        assert_eq!(
            desktop.apply(Command::RestoreSpaces { spaces }, bounds()),
            CommandOutcome::Applied
        );
        assert_eq!(labels(&desktop, id), ["new"], "what the folder holds now");
        assert!(desktop.take_retired_items().is_empty());
    }

    #[test]
    fn restoring_a_lens_on_another_folder_empties_it_until_its_list_comes() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let spaces = undo_state(&desktop, id);
        desktop.apply(
            Command::SetLensFolder {
                id,
                path: "C:/other".into(),
                title: "Other".into(),
            },
            bounds(),
        );
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);

        assert_eq!(
            desktop.apply(Command::RestoreSpaces { spaces }, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(
            space.folder_view().unwrap().path,
            PathBuf::from("C:/folder")
        );
        assert!(space.items().is_empty());
        assert_eq!(
            desktop.take_retired_items()[0][0].label,
            "a",
            "freed off the UI thread"
        );
    }

    #[test]
    fn restoring_a_lens_at_another_size_places_its_items_again() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let entries = (0..6)
            .map(|index| entry(&format!("file{index}.txt"), ItemKind::File))
            .collect();
        sync(&mut desktop, id, entries);
        let spaces = undo_state(&desktop, id);
        desktop.apply(
            Command::SetSpaceRect {
                id,
                rect: Rect::new(10, 10, 900, 400),
            },
            bounds(),
        );
        assert_eq!(
            desktop.apply(Command::RestoreSpaces { spaces }, bounds()),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.items().len(), 6);
        assert_eq!(space.items()[5].position, desktop.layout.grid_position(5));
    }

    #[test]
    fn deleting_a_restored_lens_frees_its_items_off_the_ui_thread() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        desktop.apply(
            Command::RestoreSpaces {
                spaces: vec![(id, None)],
            },
            bounds(),
        );
        assert!(desktop.space(id).is_none());
        assert_eq!(desktop.take_retired_items()[0][0].label, "a");
    }

    #[test]
    fn sync_lens_with_no_entries_empties_it() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        sync(&mut desktop, id, Vec::new());
        assert!(desktop.space(id).unwrap().items().is_empty());
    }

    #[test]
    fn sync_lens_on_a_normal_space_is_rejected() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::ZERO,
                size: Size::new(240, 120),
                locked: false,
                icon: crate::icon::SpaceIcon::None,
                view: crate::view::SpaceView::default(),
            },
            bounds(),
        );
        let id = desktop.spaces()[0].id;
        assert_eq!(
            sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]),
            CommandOutcome::Rejected
        );
        assert!(desktop.space(id).unwrap().items().is_empty());
    }

    #[test]
    fn sync_lens_missing_space_is_not_found() {
        let mut desktop = Desktop::new();
        assert_eq!(
            sync(&mut desktop, SpaceId(9), Vec::new()),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn rebinding_a_lens_clears_its_items() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        assert_eq!(
            desktop.apply(
                Command::SetLensFolder {
                    id,
                    path: "C:/other".into(),
                    title: "Other".into(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.folder_view().unwrap().path, PathBuf::from("C:/other"));
        assert_eq!(space.title(), "Other");
        assert!(space.items().is_empty());
    }

    #[test]
    fn rebinding_a_lens_to_the_folder_it_shows_keeps_its_items() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        sync(&mut desktop, id, vec![entry("a.txt", ItemKind::File)]);
        assert_eq!(
            desktop.apply(
                Command::SetLensFolder {
                    id,
                    path: "C:/folder".into(),
                    title: "Folder".into(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.title(), "Folder", "the title still updates");
        assert_eq!(labels(&desktop, id), ["a"], "the folder's items stay");
    }

    #[test]
    fn setting_a_lens_title_stores_its_mode_and_text() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        assert_eq!(
            desktop.apply(
                Command::SetLensTitle {
                    id,
                    title: LensTitle::ProfilePath,
                    text: "Documents".into(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.folder_view().unwrap().title, LensTitle::ProfilePath);
        assert_eq!(space.title(), "Documents");
    }

    #[test]
    fn renaming_a_lens_makes_its_title_custom() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        assert_eq!(
            desktop.apply(
                Command::RenameSpace {
                    id,
                    title: "Mine".into()
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.title(), "Mine");
        assert_eq!(space.folder_view().unwrap().title, LensTitle::Custom);
    }

    #[test]
    fn setting_a_lens_title_on_a_missing_space_is_not_found() {
        let mut desktop = Desktop::new();
        assert_eq!(
            desktop.apply(
                Command::SetLensTitle {
                    id: SpaceId(9),
                    title: LensTitle::Custom,
                    text: "x".into(),
                },
                bounds()
            ),
            CommandOutcome::NotFound
        );
    }

    #[test]
    fn setting_a_lens_view_replaces_it() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let view = FolderView {
            path: "C:/folder".into(),
            show_hidden: true,
            ..FolderView::default()
        };
        assert_eq!(
            desktop.apply(
                Command::SetLensView {
                    id,
                    view: view.clone(),
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        assert_eq!(desktop.space(id).unwrap().folder_view(), Some(&view));
    }

    #[test]
    fn sync_lens_sorts_by_the_spaces_rule() {
        use crate::view::SpaceView;

        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        desktop.apply(
            Command::SetSpaceView {
                id,
                view: SpaceView {
                    sort: SortRule {
                        key: SortKey::Size,
                        descending: true,
                        folders_first: false,
                    },
                    ..SpaceView::default()
                },
            },
            bounds(),
        );
        let sized = |name: &str, size: u64| FolderEntry {
            size: Some(size),
            ..entry(name, ItemKind::File)
        };
        sync(
            &mut desktop,
            id,
            vec![sized("small.txt", 10), sized("big.txt", 300)],
        );
        assert_eq!(labels(&desktop, id), ["big", "small"]);
    }

    #[test]
    fn lens_commands_on_a_normal_space_are_rejected() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::ZERO,
                size: Size::new(240, 120),
                locked: false,
                icon: crate::icon::SpaceIcon::None,
                view: crate::view::SpaceView::default(),
            },
            bounds(),
        );
        let id = desktop.spaces()[0].id;
        assert_eq!(
            desktop.apply(
                Command::SetLensFolder {
                    id,
                    path: "C:/folder".into(),
                    title: "Folder".into(),
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(
            desktop.apply(
                Command::SetLensTitle {
                    id,
                    title: LensTitle::Custom,
                    text: "x".into(),
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(
            desktop.apply(
                Command::SetLensView {
                    id,
                    view: FolderView::default(),
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
    }

    #[test]
    fn adding_an_item_to_a_lens_is_rejected() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        assert_eq!(
            desktop.apply(
                Command::AddItem {
                    space: id,
                    path: "C:/elsewhere/a.txt".into(),
                    facts: ItemFacts::default(),
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert!(desktop.space(id).unwrap().items().is_empty());
    }

    #[test]
    fn moving_an_item_into_or_out_of_a_lens_is_rejected() {
        let mut desktop = Desktop::new();
        let lens_id = lens(&mut desktop, "C:/folder");
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::new(400, 0),
                size: Size::new(240, 120),
                locked: false,
                icon: crate::icon::SpaceIcon::None,
                view: crate::view::SpaceView::default(),
            },
            bounds(),
        );
        let space_id = desktop.spaces()[1].id;
        desktop.apply(
            Command::AddItem {
                space: space_id,
                path: "C:/folder/a.txt".into(),
                facts: ItemFacts::default(),
            },
            bounds(),
        );
        let item = desktop.space(space_id).unwrap().items()[0].id;

        assert_eq!(
            desktop.apply(
                Command::MoveItem {
                    id: item,
                    target: lens_id,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        sync(&mut desktop, lens_id, vec![entry("b.txt", ItemKind::File)]);
        let lens_item = desktop.space(lens_id).unwrap().items()[0].id;
        assert_eq!(
            desktop.apply(
                Command::MoveItem {
                    id: lens_item,
                    target: space_id,
                },
                bounds()
            ),
            CommandOutcome::Rejected
        );
        assert_eq!(
            desktop.space_containing(item),
            Some(space_id),
            "the rejected move changed nothing"
        );
        assert_eq!(desktop.space_containing(lens_item), Some(lens_id));
    }

    fn filter(desktop: &mut Desktop, id: SpaceId, include: &[&str], exclude: &[&str]) {
        let mut view = desktop.space(id).unwrap().folder_view().unwrap().clone();
        view.include = include.iter().map(|p| p.to_string()).collect();
        view.exclude = exclude.iter().map(|p| p.to_string()).collect();
        assert!(
            desktop
                .apply(Command::SetLensView { id, view }, bounds())
                .is_applied()
        );
    }

    fn filter_entries() -> Vec<FolderEntry> {
        vec![
            entry("a.pdf", ItemKind::File),
            entry("b.txt", ItemKind::File),
            entry("c.tmp", ItemKind::File),
            entry("Sub", ItemKind::Folder),
            entry("Old", ItemKind::Folder),
        ]
    }

    #[test]
    fn show_only_keeps_matching_files_and_every_folder() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        filter(&mut desktop, id, &["*.PDF"], &[]);
        sync(&mut desktop, id, filter_entries());
        assert_eq!(labels(&desktop, id), ["a.pdf", "Old", "Sub"]);
    }

    #[test]
    fn hide_drops_matching_files_and_folders() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        filter(&mut desktop, id, &[], &["*.tmp", "old"]);
        sync(&mut desktop, id, filter_entries());
        assert_eq!(labels(&desktop, id), ["a.pdf", "b", "Sub"]);
    }

    #[test]
    fn hide_wins_over_show_only_and_names_not_paths_are_matched() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        // "folder" is in every entry's path but no entry's name.
        filter(&mut desktop, id, &["*.pdf", "*folder*"], &["a.*"]);
        sync(&mut desktop, id, filter_entries());
        assert_eq!(labels(&desktop, id), ["Old", "Sub"]);
    }

    #[test]
    fn filter_lists_are_capped_trimmed_and_deduplicated() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let mut many: Vec<String> = (0..40).map(|n| format!("*.{n}")).collect();
        many.insert(0, "  *.pdf  ".into());
        many.insert(1, "*.pdf".into());
        many.insert(2, "   ".into());
        many.insert(3, "x".repeat(300));
        let mut view = desktop.space(id).unwrap().folder_view().unwrap().clone();
        view.include = many;
        desktop.apply(Command::SetLensView { id, view }, bounds());
        let include = &desktop.space(id).unwrap().folder_view().unwrap().include;
        assert_eq!(include.len(), crate::source::MAX_PATTERNS);
        assert_eq!(include[0], "*.pdf", "trimmed, and its repeat dropped");
        assert_eq!(include[1].chars().count(), crate::source::MAX_PATTERN_LEN);
        assert_eq!(include[2], "*.0", "the blank pattern is dropped");
    }

    /// Run by hand in release for the Performance rule's dated timing:
    /// `cargo test --release -p deskspace-core -- --ignored --nocapture`.
    #[test]
    #[ignore = "a timing, for release builds; run by hand"]
    fn time_a_filtered_sync_of_fifty_thousand_entries() {
        // In the order NTFS lists them: by code unit, so `10` comes before
        // `9`, which natural sort has to untangle.
        let entries = || -> Vec<FolderEntry> {
            let mut entries: Vec<FolderEntry> = (0..50_000)
                .map(|n| {
                    entry(
                        &format!("quarterly report {n} final draft.pdf"),
                        ItemKind::File,
                    )
                })
                .collect();
            entries.sort_by(|a, b| a.path.cmp(&b.path));
            entries
        };
        // 32 "Hide" patterns, each tried on every entry and matching none, so
        // every entry is still listed: one set the end check rejects at
        // once, as most real patterns are, and one whose end matches so the
        // whole pattern is tried.
        let hide = |pattern: fn(usize) -> String| (0..32).map(pattern).collect::<Vec<_>>();
        let cases = [
            ("no filters", Vec::new()),
            (
                "32 patterns, end differs",
                hide(|n| format!("*report*draft{n}?.tmp")),
            ),
            (
                "32 patterns, end matches",
                hide(|n| format!("*quarterly*report*{n}x*.pdf")),
            ),
        ];
        for (label, exclude) in cases {
            let mut desktop = Desktop::new();
            let id = lens(&mut desktop, "C:/folder");
            let mut view = desktop.space(id).unwrap().folder_view().unwrap().clone();
            view.exclude = exclude;
            desktop.apply(Command::SetLensView { id, view }, bounds());
            let rules = desktop.space(id).unwrap().lens_rules().unwrap();
            let mut builder = LensBuilder::new(rules, desktop.reserve_item_ids(), &[]);
            for pass in ["first listing", "refresh"] {
                let entries = entries();
                let start = std::time::Instant::now();
                let list = builder.listed(entries);
                let built = start.elapsed();
                let kept = list.items.len();
                let start = std::time::Instant::now();
                desktop.apply(
                    Command::SyncLens {
                        id,
                        items: list.items,
                    },
                    bounds(),
                );
                let swapped = start.elapsed();
                let start = std::time::Instant::now();
                drop(desktop.take_retired_items());
                println!(
                    "{label}, {pass}, {kept} of 50,000 kept: worker {built:?}, \
                     UI thread swap {swapped:?}, freeing the old list {:?}",
                    start.elapsed()
                );
            }
        }
    }
}
