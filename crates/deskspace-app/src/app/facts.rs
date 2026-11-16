//! Keeping a normal space's item facts fresh. An item's size, dates, and
//! attributes are read when it is added and go stale as its file changes, so
//! at startup the icon worker reads them again, off this thread, and the
//! batches it hands back are applied here. A lens needs none of this: every
//! listing brings its items' facts.

use std::path::PathBuf;

use deskspace_core::{Command, Desktop, ItemId, SpaceId};

use super::App;
use super::effect::Effect;
use super::persist::commit_effects;

impl App {
    /// Asks the worker to read every normal space's item facts again.
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

    /// Applies the batches collected so far, each as one `RefreshItemFacts`,
    /// and returns what the spaces it changed owe. While an undo entry is
    /// open they wait for it to finish: a refresh dispatched now would join
    /// the user's action, and undoing that would put the old facts back.
    pub(super) fn apply_item_facts(&mut self) -> Vec<Effect> {
        if self.held_facts.is_empty() || self.undo.is_recording() {
            return Vec::new();
        }
        let mut effects = Vec::new();
        for (space, facts) in std::mem::take(&mut self.held_facts) {
            let items = facts.len();
            let at = self.space_center(space);
            let command = Command::RefreshItemFacts { space, facts };
            let touched = self.config.desktop.touched_by(&command);
            let outcome = self.dispatch(command, at);
            tracing::debug!(%space, items, ?outcome, "read a space's item facts again");
            effects.extend(commit_effects(outcome, touched, Vec::new()));
        }
        effects
    }
}

/// The files to read: every item of every normal space that has any, with
/// its path. A lens is left out, because its listing owns its facts.
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
    use deskspace_core::{ItemFacts, Rect, Touched};

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

    #[test]
    fn a_batch_updates_its_space_and_owes_a_repaint_and_a_save() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(16, 32, 320, 208));
        let item = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        app.held_facts.push((id, vec![(item, sized(42))]));

        let effects = app.apply_item_facts();

        assert_eq!(size_of(&app, item), Some(42));
        assert_eq!(
            effects,
            vec![
                Effect::Refresh(Touched {
                    spaces: vec![id],
                    layer: false
                }),
                Effect::ScheduleSave
            ]
        );
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

        app.held_facts.push((id, vec![(item, sized(42))]));
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
        app.held_facts.push((id, vec![(item, sized(42))]));
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
            (id, vec![(item, ItemFacts::default())]),
            (lens, vec![(ItemId(900), sized(1))]),
            (SpaceId(900), vec![(item, sized(1))]),
        ];

        assert_eq!(app.apply_item_facts(), vec![]);
        assert!(app.held_facts.is_empty());
        assert_eq!(size_of(&app, item), None);
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
