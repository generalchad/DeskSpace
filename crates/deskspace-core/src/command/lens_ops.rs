//! Commands that create and refresh a lens, a space that mirrors a folder.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::CommandOutcome;
use super::sort::compare_lens_items;
use crate::desktop::Desktop;
use crate::geometry::{Point, Rect, Size};
use crate::id::SpaceId;
use crate::item::Item;
use crate::layout::{GridSpec, arrange_space, clamp_to_bounds};
use crate::source::{FolderEntry, FolderView, SpaceSource};

impl Desktop {
    pub(super) fn apply_create_lens(
        &mut self,
        title: String,
        at: Point,
        size: Size,
        path: PathBuf,
        locked: bool,
        bounds: Rect,
    ) -> CommandOutcome {
        let rect = clamp_to_bounds(Rect::new(at.x, at.y, size.width, size.height), bounds);
        let id = self.add_space(title, rect);
        if let Some(space) = self.space_mut(id) {
            space.source = SpaceSource::Folder(FolderView {
                path,
                ..FolderView::default()
            });
            space.locked = locked;
        }
        CommandOutcome::Applied
    }

    pub(super) fn apply_set_lens_folder(&mut self, id: SpaceId, path: PathBuf) -> CommandOutcome {
        self.with_space(id, |space| {
            let SpaceSource::Folder(view) = &mut space.source else {
                return CommandOutcome::Rejected;
            };
            view.path = path;
            space.items.clear();
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

        items.sort_by(|a, b| compare_lens_items(&view, a, b));
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
    use crate::item::{ItemFacts, ItemKind, SortKey};

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
        assert!(space.items().is_empty());
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
            },
            bounds(),
        );
        let rect = desktop.space(desktop.spaces()[0].id).unwrap().rect();
        assert!(rect.x + rect.width <= bounds().width);
        assert!(rect.y + rect.height <= bounds().height);
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
                },
                bounds()
            ),
            CommandOutcome::Applied
        );
        let space = desktop.space(id).unwrap();
        assert_eq!(space.folder_view().unwrap().path, PathBuf::from("C:/other"));
        assert!(space.items().is_empty());
    }

    #[test]
    fn setting_a_lens_view_replaces_it() {
        let mut desktop = Desktop::new();
        let id = lens(&mut desktop, "C:/folder");
        let view = FolderView {
            path: "C:/folder".into(),
            sort: SortKey::Modified,
            descending: true,
            folders_first: true,
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
    fn lens_commands_on_a_normal_space_are_rejected() {
        let mut desktop = Desktop::new();
        desktop.apply(
            Command::CreateSpace {
                title: "Space".into(),
                at: Point::ZERO,
                size: Size::new(240, 120),
                locked: false,
            },
            bounds(),
        );
        let id = desktop.spaces()[0].id;
        assert_eq!(
            desktop.apply(
                Command::SetLensFolder {
                    id,
                    path: "C:/folder".into(),
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
