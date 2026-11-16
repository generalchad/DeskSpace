//! Keeping a normal space's items in step with their files. An item's size,
//! dates, and attributes are read when it is added and go stale as its file
//! changes, and a file can be deleted while DeskSpace is closed. So at
//! startup the icon worker reads every item's file, off this thread, and
//! what it hands back is applied here: fresh facts, and the items whose
//! files are gone. A lens needs none of this: every listing brings its items.

use std::path::PathBuf;

use deskspace_core::{Command, Desktop, ItemId, SpaceId, Touched};
use deskspace_platform::FactBatch;

use super::App;
use super::effect::Effect;

impl App {
    /// Asks the worker to read every normal space's items again.
    /// `Event::ItemFactsRead` brings each space's batch back.
    pub(super) fn request_item_facts(&mut self) {
        for (space, items) in fact_requests(&self.config.desktop) {
            self.icons.read_item_facts(space, items);
        }
    }

    /// Takes the batches the worker has finished. `apply_item_facts`, which
    /// runs after every event, applies them.
    pub(super) fn collect_item_facts(&mut self) {
        self.held_facts.extend(self.icons.take_item_facts());
    }

    /// Applies the batches collected so far and returns what the spaces they
    /// changed owe. While an undo entry is open they wait for it to finish:
    /// a change dispatched now would join the user's action, and undoing
    /// that would put the old facts, or a pruned item, back.
    pub(super) fn apply_item_facts(&mut self) -> Vec<Effect> {
        if self.held_facts.is_empty() || self.undo.is_recording() {
            return Vec::new();
        }
        std::mem::take(&mut self.held_facts)
            .into_iter()
            .flat_map(|batch| self.apply_fact_batch(batch))
            .collect()
    }

    /// Drops the items of one batch whose files are gone, then writes the
    /// facts read for the rest as one `RefreshItemFacts`.
    fn apply_fact_batch(&mut self, batch: FactBatch) -> Vec<Effect> {
        let FactBatch { space, facts, gone } = batch;
        let pruned = self.remove_gone(space, &gone);
        let read = facts.len();
        let at = self.space_center(space);
        let refreshed = self
            .dispatch(Command::RefreshItemFacts { space, facts }, at)
            .is_applied();
        tracing::debug!(%space, read, pruned, refreshed, "read a space's items again");
        let mut effects = Vec::new();
        if pruned > 0 || refreshed {
            effects.push(Effect::Refresh(Touched {
                spaces: vec![space],
                layer: false,
            }));
        }
        if pruned > 0 {
            effects.push(Effect::RefreshTray);
        }
        if !effects.is_empty() {
            effects.push(Effect::ScheduleSave);
        }
        effects
    }
}

/// The files to read: every item of every normal space that has any, with
/// its path. A lens is left out, because its listing owns its items.
fn fact_requests(desktop: &Desktop) -> Vec<(SpaceId, Vec<(ItemId, PathBuf)>)> {
    desktop
        .spaces()
        .iter()
        .filter(|space| !space.is_lens() && !space.items().is_empty())
        .map(|space| {
            let items = space
                .items()
                .iter()
                .map(|item| (item.id, item.path.clone()))
                .collect();
            (space.id, items)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::geometry::Point;
    use deskspace_core::{ItemFacts, Rect};

    use crate::app::undo::StepOutcome;
    use crate::fixtures::{add_item, create_lens, create_space};

    use super::*;

    fn test_app() -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        App::new(Config::new(), store, false, false)
    }

    fn sized(size: u64) -> ItemFacts {
        ItemFacts {
            size: Some(size),
            ..ItemFacts::default()
        }
    }

    fn size_of(app: &App, item: ItemId) -> Option<u64> {
        app.config.desktop.item(item).and_then(|item| item.size)
    }

    /// A batch that read `facts` and found no file gone.
    fn read(space: SpaceId, facts: Vec<(ItemId, ItemFacts)>) -> FactBatch {
        FactBatch {
            space,
            facts,
            gone: Vec::new(),
        }
    }

    /// A batch that read nothing and found `gone`'s files gone.
    fn gone(space: SpaceId, gone: Vec<ItemId>) -> FactBatch {
        FactBatch {
            space,
            facts: Vec::new(),
            gone,
        }
    }

    fn repaint(space: SpaceId) -> Effect {
        Effect::Refresh(Touched {
            spaces: vec![space],
            layer: false,
        })
    }

    /// A file that exists for a test, removed on drop.
    struct RealFile(PathBuf);

    impl RealFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "deskspace-facts-test-{name}-{}",
                std::process::id()
            ));
            std::fs::write(&path, "x").unwrap();
            Self(path)
        }
    }

    impl Drop for RealFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn a_batch_updates_its_space_and_owes_a_repaint_and_a_save() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let item = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        app.held_facts.push(read(id, vec![(item, sized(42))]));

        let effects = app.apply_item_facts();

        assert_eq!(size_of(&app, item), Some(42));
        assert_eq!(effects, vec![repaint(id), Effect::ScheduleSave]);
        assert!(app.held_facts.is_empty());
        assert_eq!(app.undo.undo_label(), None, "a refresh is not an action");
    }

    #[test]
    fn a_refresh_leaves_undo_available() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let item = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        app.record_effects(
            "Rename space",
            Command::RenameSpace {
                id,
                title: "Play".into(),
            },
            Point::default(),
            Vec::new(),
        );
        assert_eq!(app.undo.undo_label(), Some("Rename space"));

        app.held_facts.push(read(id, vec![(item, sized(42))]));
        assert!(!app.apply_item_facts().is_empty());

        assert_eq!(app.undo.undo_label(), Some("Rename space"));
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().title(), "Work");
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().title(), "Play");
    }

    #[test]
    fn a_batch_waits_for_an_open_undo_entry_and_stays_out_of_it() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let item = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");

        // A drag in progress holds an entry open between events.
        app.undo.begin("Move space");
        app.held_facts.push(read(id, vec![(item, sized(42))]));
        assert_eq!(app.apply_item_facts(), vec![]);
        assert_eq!(size_of(&app, item), None, "nothing is applied yet");
        assert_eq!(app.held_facts.len(), 1, "and nothing is lost");

        app.undo.finish(&app.config.desktop);
        assert!(!app.apply_item_facts().is_empty());
        assert_eq!(size_of(&app, item), Some(42));
        assert_eq!(app.undo.undo_label(), None, "the drag's entry never saw it");
    }

    #[test]
    fn a_batch_that_changes_nothing_owes_nothing() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let item = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        let lens = create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(400, 32, 320, 208),
            "C:/missing/folder",
        );
        app.held_facts = vec![
            read(id, vec![(item, ItemFacts::default())]),
            read(lens, vec![(ItemId(900), sized(1))]),
            read(SpaceId(900), vec![(item, sized(1))]),
            gone(id, vec![ItemId(900)]),
            gone(SpaceId(900), vec![item]),
        ];

        assert_eq!(app.apply_item_facts(), vec![]);
        assert!(app.held_facts.is_empty());
        assert_eq!(size_of(&app, item), None);
        assert!(app.config.desktop.item(item).is_some());
    }

    #[test]
    fn an_item_whose_file_is_gone_is_dropped_and_the_rest_refreshed() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let kept_file = RealFile::new("kept");
        let kept = add_item(&mut app.config.desktop, id, &kept_file.0.to_string_lossy());
        // `C:\` answers and holds no such folder, so this file is gone.
        let deleted = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        app.held_facts.push(FactBatch {
            space: id,
            facts: vec![(kept, sized(42))],
            gone: vec![deleted],
        });

        let effects = app.apply_item_facts();

        assert!(app.config.desktop.item(deleted).is_none());
        assert_eq!(size_of(&app, kept), Some(42));
        assert_eq!(
            effects,
            vec![repaint(id), Effect::RefreshTray, Effect::ScheduleSave]
        );
    }

    #[test]
    fn an_item_is_kept_when_its_file_is_there_or_cannot_be_reached_now() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        // Renamed since the worker read its old path, or back since.
        let there_file = RealFile::new("there");
        let there = add_item(&mut app.config.desktop, id, &there_file.0.to_string_lossy());
        // No drive or share answers for a path without one.
        let out_of_reach = add_item(&mut app.config.desktop, id, "no-root/a.txt");
        app.held_facts.push(gone(id, vec![there, out_of_reach]));

        assert_eq!(app.apply_item_facts(), vec![]);
        assert!(app.config.desktop.item(there).is_some());
        assert!(app.config.desktop.item(out_of_reach).is_some());
    }

    #[test]
    fn every_normal_space_with_items_is_asked_for_and_no_lens() {
        let mut desktop = Desktop::default();
        let work = create_space(&mut desktop, "Work", Rect::new(16, 32, 320, 208));
        let first = add_item(&mut desktop, work, "C:/missing/a.txt");
        let second = add_item(&mut desktop, work, "C:/missing/b.txt");
        create_space(&mut desktop, "Empty", Rect::new(400, 32, 320, 208));
        create_lens(
            &mut desktop,
            "Lens",
            Rect::new(800, 32, 320, 208),
            "C:/missing/folder",
        );

        assert_eq!(
            fact_requests(&desktop),
            [(
                work,
                vec![
                    (first, PathBuf::from("C:/missing/a.txt")),
                    (second, PathBuf::from("C:/missing/b.txt"))
                ]
            )]
        );
    }
}
