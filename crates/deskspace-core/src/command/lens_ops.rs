//! Commands that create and refresh a lens, a space that mirrors a folder.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::CommandOutcome;
use crate::desktop::Desktop;
use crate::geometry::Rect;
use crate::id::SpaceId;
use crate::item::Item;
use crate::layout::{GridSpec, arrange_space, clamp_to_bounds};
use crate::source::{FolderEntry, FolderView, LensTitle, SpaceSource};
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
        self.with_space(id, |space| {
            let SpaceSource::Folder(view) = &mut space.source else {
                return CommandOutcome::Rejected;
            };
            view.path = path;
            space.title = title;
            space.items.clear();
            CommandOutcome::Applied
        })
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
            *current = view;
            CommandOutcome::Applied
        })
    }

    /// Reconciles `entries` with the lens's current items by path, then sorts
    /// and lays them out. Include/exclude filters are F-22 and are not applied
    /// yet.
    pub(super) fn apply_sync_lens(
        &mut self,
        id: SpaceId,
        entries: Vec<FolderEntry>,
        layout: GridSpec,
    ) -> CommandOutcome {
        let Some(space) = self.space(id) else {
            return CommandOutcome::NotFound;
        };
        let Some(view) = space.folder_view().cloned() else {
            return CommandOutcome::Rejected;
        };
        let rule = space.view().sort;
        let mut existing: HashMap<PathBuf, Item> = space
            .items()
            .iter()
            .cloned()
            .map(|item| (item.path.clone(), item))
            .collect();

        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for entry in entries {
            if !seen.insert(entry.path.clone()) || (entry.hidden && !view.show_hidden) {
                continue;
            }
            let path = entry.path.clone();
            let label = entry.label.clone();
            let kind = entry.kind;
            let size = entry.size;
            let modified = entry.modified;
            let mut item = match existing.remove(&path) {
                Some(item) => item,
                None => entry.into_item(self.take_item_id()),
            };
            if !label.is_empty() {
                item.label = label;
            }
            item.kind = kind;
            item.size = size;
            item.modified = modified;
            items.push(item);
        }

        rule.sort_items(&mut items);
        let Some(space) = self.space_mut(id) else {
            return CommandOutcome::NotFound;
        };
        space.items = items;
        arrange_space(space, layout);
        CommandOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, CommandOutcome};
    use crate::geometry::{Point, Size};
    use crate::item::{ItemFacts, ItemKind, SortKey};
    use crate::sort::SortRule;

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
            hidden: false,
        }
    }

    fn sync(desktop: &mut Desktop, id: SpaceId, entries: Vec<FolderEntry>) -> CommandOutcome {
        desktop.apply(Command::SyncLens { id, entries }, bounds())
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
                        view,
                    },
                    bounds(),
                )
                .is_applied()
        );
        assert_eq!(desktop.space(id).unwrap().view(), view);
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
        sync(&mut desktop, id, vec![changed]);
        let item = &desktop.space(id).unwrap().items()[0];
        assert_eq!(item.label, "Renamed");
        assert_eq!(item.kind, ItemKind::Folder);
        assert_eq!((item.size, item.modified), (Some(99), Some(7)));
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
}
