//! The debug window's Spaces section: making, clearing, and resetting test
//! spaces through the paths the menus already use.

use deskspace_core::{Command, Desktop, Space, SpaceId};
use deskspace_platform::{DebugControlKind, Ui, cursor_pos};

use super::test_spaces::{STRESS_SIZES, TEST_SPACES, stress_shortcuts};
use super::windows::rect_text;
use crate::app::App;
use crate::app::files::{ShortcutSource, StoredFiles, stored_paths};

impl App {
    /// Delete all spaces (ROADMAP D14): every space goes as Delete space
    /// would, but stored files go to the Recycle Bin instead of the desktop.
    pub(super) fn delete_all_spaces(&mut self, ui: &mut Ui) {
        let spaces = self.config.desktop.spaces();
        let items = spaces.iter().map(|space| space.items().len()).sum();
        let stored = spaces
            .iter()
            .map(|space| stored_paths(space, &self.storage).len())
            .sum();
        let ids: Vec<_> = spaces.iter().map(|space| space.id).collect();
        let question = delete_all_question(ids.len(), items, stored);
        if !self.debug_confirm(ui, &question) {
            return;
        }
        for id in &ids {
            self.delete_space_now(ui, *id, StoredFiles::ToRecycleBin);
        }
        let kept = ids
            .iter()
            .filter(|id| self.config.desktop.space(**id).is_some())
            .count();
        tracing::info!(
            deleted = ids.len() - kept,
            kept,
            "debug: deleted all spaces"
        );
    }

    /// Create test spaces: one ordinary space per `TEST_SPACES` row, made
    /// through New space and filled through the one shortcut writer.
    pub(super) fn create_test_spaces(&mut self, ui: &mut Ui) {
        let mut made = 0;
        for space in TEST_SPACES {
            let sources = space
                .shortcuts
                .iter()
                .filter_map(|shortcut| shortcut.source())
                .collect();
            if self.create_test_space(ui, space.title, sources).is_some() {
                made += 1;
            }
        }
        tracing::info!(made, "debug: created test spaces");
    }

    /// Create a stress space: one space holding as many shortcuts as the
    /// chosen stress size, in one `add_shortcuts` call.
    pub(super) fn create_stress_space(&mut self, ui: &mut Ui) {
        let index = self.debug.as_ref().map_or(0, |tools| tools.stress_size);
        let size = STRESS_SIZES[index.min(STRESS_SIZES.len() - 1)];
        let sources = stress_shortcuts(size);
        let made = sources.len();
        self.create_test_space(ui, &format!("Test: Stress {size}"), sources);
        tracing::info!(size, made, "debug: created a stress space");
    }

    /// A space where New space puts one, titled `title`, holding a shortcut
    /// for each source. Returns its id.
    fn create_test_space(
        &mut self,
        ui: &mut Ui,
        title: &str,
        sources: Vec<ShortcutSource>,
    ) -> Option<SpaceId> {
        let id = self.create_space(ui, cursor_pos())?;
        let near = self.space_center(id);
        let title = title.to_string();
        self.commit(ui, Command::RenameSpace { id, title }, near);
        self.add_shortcuts(ui, id, near, sources);
        Some(id)
    }

    /// Every space in model order, as the table shows it.
    pub(super) fn space_table(&self) -> DebugControlKind {
        let columns = [
            "Id", "Title", "Rect", "Items", "Folded", "Locked", "Own look",
        ];
        DebugControlKind::Table {
            columns: columns.iter().map(|column| column.to_string()).collect(),
            rows: self.config.desktop.spaces().iter().map(space_row).collect(),
        }
    }

    /// Forgets every cached icon and redraws, so each icon is read from the
    /// shell again.
    pub(super) fn clear_icon_cache(&mut self, ui: &mut Ui) {
        let cleared = self.icons.len();
        self.icons.clear();
        self.refresh_spaces(ui);
        tracing::info!(cleared, "debug: cleared the icon cache");
    }

    /// Unfolds every folded space as the space menu's Fold does.
    pub(super) fn unfold_all(&mut self, ui: &mut Ui) {
        let folded = folded_spaces(&self.config.desktop);
        for id in &folded {
            self.toggle_fold(ui, *id);
        }
        tracing::info!(count = folded.len(), "debug: unfolded spaces");
    }

    /// Unlocks every locked space as the space menu's Lock position does.
    pub(super) fn unlock_all(&mut self, ui: &mut Ui) {
        let locked = locked_spaces(&self.config.desktop);
        for id in &locked {
            let near = self.space_center(*id);
            self.commit(ui, Command::ToggleLock { id: *id }, near);
        }
        tracing::info!(count = locked.len(), "debug: unlocked spaces");
    }
}

/// One space's row in the Spaces table.
fn space_row(space: &Space) -> Vec<String> {
    let yes_no = |value: bool| if value { "yes" } else { "no" }.to_string();
    vec![
        space.id.0.to_string(),
        space.title().to_string(),
        rect_text(space.rect()),
        space.items().len().to_string(),
        yes_no(space.folded()),
        yes_no(space.locked()),
        yes_no(space.custom()),
    ]
}

/// The spaces that are folded now.
fn folded_spaces(desktop: &Desktop) -> Vec<SpaceId> {
    desktop
        .spaces()
        .iter()
        .filter(|space| space.folded())
        .map(|space| space.id)
        .collect()
}

/// The spaces that are locked now.
fn locked_spaces(desktop: &Desktop) -> Vec<SpaceId> {
    desktop
        .spaces()
        .iter()
        .filter(|space| space.locked())
        .map(|space| space.id)
        .collect()
}

/// `count` and `noun`, plural unless `count` is 1.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Delete all spaces' question: how many spaces and items go, and where the
/// files go.
fn delete_all_question(spaces: usize, items: usize, stored: usize) -> String {
    let (ask, hold) = if spaces == 1 {
        ("Delete the only space?".to_string(), "It holds")
    } else {
        (format!("Delete all {spaces} spaces?"), "They hold")
    };
    let go = if stored == 1 { "goes" } else { "go" };
    format!(
        "{ask}\n\n{hold} {}. {} {go} to the Recycle Bin; items shown in place \
         are only removed from their spaces.",
        counted(items, "item"),
        counted(stored, "stored file"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use deskspace_core::Rect;

    #[test]
    fn unfolding_and_unlocking_leave_every_space_open_and_free() {
        let mut desktop = Desktop::default();
        let a = fixtures::create_space(&mut desktop, "A", Rect::new(0, 0, 300, 200));
        let b = fixtures::create_space(&mut desktop, "B", Rect::new(400, 0, 300, 200));
        fixtures::create_space(&mut desktop, "C", Rect::new(800, 0, 300, 200));
        fixtures::apply(&mut desktop, Command::ToggleFold { id: a });
        fixtures::apply(&mut desktop, Command::ToggleLock { id: a });
        fixtures::apply(&mut desktop, Command::ToggleLock { id: b });
        for id in folded_spaces(&desktop) {
            fixtures::apply(&mut desktop, Command::ToggleFold { id });
        }
        for id in locked_spaces(&desktop) {
            fixtures::apply(&mut desktop, Command::ToggleLock { id });
        }
        let spaces = desktop.spaces();
        assert!(
            spaces
                .iter()
                .all(|space| !space.folded() && !space.locked())
        );
    }

    #[test]
    fn a_space_row_names_its_state() {
        let mut desktop = Desktop::default();
        let id = fixtures::create_space(&mut desktop, "Work", Rect::new(10, 20, 300, 200));
        fixtures::add_item(&mut desktop, id, "C:/missing/a.txt");
        fixtures::apply(&mut desktop, Command::ToggleLock { id });
        let row = space_row(desktop.space(id).unwrap());
        assert_eq!(row[1], "Work");
        assert_eq!(row[3], "1");
        assert_eq!(&row[4..6], ["no", "yes"]);
    }

    #[test]
    fn delete_all_says_how_many_and_where_files_go() {
        assert_eq!(
            delete_all_question(3, 12, 5),
            "Delete all 3 spaces?\n\nThey hold 12 items. 5 stored files go to the \
             Recycle Bin; items shown in place are only removed from their spaces."
        );
        assert_eq!(
            delete_all_question(1, 1, 0),
            "Delete the only space?\n\nIt holds 1 item. 0 stored files go to the \
             Recycle Bin; items shown in place are only removed from their spaces."
        );
        assert!(delete_all_question(2, 2, 1).contains("1 stored file goes"));
    }
}
