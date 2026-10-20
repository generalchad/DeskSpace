use std::path::{Path, PathBuf};
use std::time::SystemTime;

use deskspace_core::geometry::Point;
use deskspace_core::{Command, Desktop, Space, SpaceId};
use deskspace_platform::{Ui, notify_moved};

use crate::storage::StoreError;

use super::effect::Effect;
use super::{App, Surface};

/// Maximum number of undo entries kept in the session history (`ROADMAP.md` D11).
pub(super) const UNDO_DEPTH: usize = 50;

/// Label for moving one or several items.
pub(super) fn move_items_label(count: usize) -> String {
    if count == 1 {
        "Move item".to_string()
    } else {
        format!("Move {count} items")
    }
}

/// Label for copying one or several items.
pub(super) fn copy_items_label(count: usize) -> String {
    if count == 1 {
        "Copy item".to_string()
    } else {
        format!("Copy {count} items")
    }
}

/// Label for pasting one or several items.
pub(super) fn paste_items_label(count: usize) -> String {
    if count == 1 {
        "Paste item".to_string()
    } else {
        format!("Paste {count} items")
    }
}

/// Label for deleting one or several items.
pub(super) fn delete_items_label(count: usize) -> String {
    if count == 1 {
        "Delete item".to_string()
    } else {
        format!("Delete {count} items")
    }
}

/// Label for creating one or several shortcuts.
pub(super) fn new_shortcuts_label(count: usize) -> String {
    if count == 1 {
        "New shortcut".to_string()
    } else {
        format!("New {count} shortcuts")
    }
}

/// A filesystem change made by a user action, recorded in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FileChange {
    Moved { from: PathBuf, to: PathBuf },
    Renamed { from: PathBuf, to: PathBuf },
    Created { path: PathBuf },
    Recycled { original: PathBuf, at: SystemTime },
}

impl FileChange {
    fn can_undo(&self, exists: &impl Fn(&Path) -> bool) -> bool {
        match self {
            Self::Moved { from, to } | Self::Renamed { from, to } => exists(to) && !exists(from),
            Self::Created { path } => exists(path),
            Self::Recycled { original, .. } => !exists(original),
        }
    }

    fn can_redo(&self, exists: &impl Fn(&Path) -> bool) -> bool {
        match self {
            Self::Moved { from, to } | Self::Renamed { from, to } => exists(from) && !exists(to),
            Self::Created { path } => exists(path),
            Self::Recycled { original, .. } => !exists(original),
        }
    }
}

/// Before and after snapshots of a single space within an [`UndoEntry`].
///
/// `None` means the space did not exist in [`Desktop`] at that point.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SpaceChange {
    pub id: SpaceId,
    pub before: Option<(usize, Space)>,
    pub after: Option<(usize, Space)>,
}

/// One undoable user action (`ROADMAP.md` D11).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct UndoEntry {
    pub label: String,
    pub spaces: Vec<SpaceChange>,
    pub files: Vec<FileChange>,
}

impl UndoEntry {
    /// True while every listed space still equals its `after` state and every
    /// file change can be reversed.
    pub(super) fn can_undo(&self, desktop: &Desktop, exists: impl Fn(&Path) -> bool) -> bool {
        self.spaces
            .iter()
            .all(|change| space_state(desktop, change.id) == change.after)
            && self.files.iter().all(|file| file.can_undo(&exists))
    }

    /// True while every listed space still equals its `before` state and every
    /// file change can be re-applied.
    pub(super) fn can_redo(&self, desktop: &Desktop, exists: impl Fn(&Path) -> bool) -> bool {
        self.spaces
            .iter()
            .all(|change| space_state(desktop, change.id) == change.before)
            && self.files.iter().all(|file| file.can_redo(&exists))
    }
}

#[derive(Debug, Clone)]
struct OpenEntry {
    label: String,
    spaces: Vec<(SpaceId, Option<(usize, Space)>)>,
    files: Vec<FileChange>,
    depth: usize,
    cancelled: bool,
}

/// Session-only undo and redo stacks (`ROADMAP.md` D11).
#[derive(Debug, Default)]
pub(super) struct UndoStack {
    pub undo: Vec<UndoEntry>,
    pub redo: Vec<UndoEntry>,
    open: Option<OpenEntry>,
    suppressed: bool,
}

impl UndoStack {
    /// Starts recording a user action under `label`. Nested `begin` calls
    /// merge into the outer entry so a composite action (such as opening a
    /// new space on a desktop drop and moving items into it) records once.
    pub(super) fn begin(&mut self, label: impl Into<String>) {
        if self.suppressed {
            return;
        }
        match &mut self.open {
            Some(open) => open.depth += 1,
            None => {
                self.open = Some(OpenEntry {
                    label: label.into(),
                    spaces: Vec::new(),
                    files: Vec::new(),
                    depth: 1,
                    cancelled: false,
                });
            }
        }
    }

    /// Starts recording under `label` when no entry is open yet (used by
    /// multi-step pointer drags that finish on button release).
    pub(super) fn begin_if_none(&mut self, label: impl Into<String>) {
        if self.open.is_none() {
            self.begin(label);
        }
    }

    /// Updates the open entry's label when at the outer level.
    pub(super) fn set_label(&mut self, label: impl Into<String>) {
        if let Some(open) = &mut self.open
            && open.depth == 1
        {
            open.label = label.into();
        }
    }

    /// Discards the open entry (even if nested), recording nothing.
    pub(super) fn cancel(&mut self) {
        if let Some(open) = &mut self.open {
            open.cancelled = true;
            if open.depth <= 1 {
                self.open = None;
            }
        }
    }

    /// Turns recording off or on (used while a debug-window tool runs).
    pub(super) fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
        if suppressed {
            self.open = None;
        }
    }

    /// Label of the next action [`App::undo`] will undo, if any.
    pub(super) fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }

    /// Label of the next action [`App::redo`] will redo, if any.
    pub(super) fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    /// Captures `id`'s current state in `desktop` the first time the open entry sees it.
    pub(super) fn note_space(&mut self, desktop: &Desktop, id: SpaceId) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.spaces.iter().any(|(noted, _)| *noted == id) {
            return;
        }
        open.spaces.push((id, space_state(desktop, id)));
    }

    /// Records a filesystem change in the open entry.
    pub(super) fn note_file(&mut self, change: FileChange) {
        if let Some(open) = &mut self.open {
            open.files.push(change);
        }
    }

    /// Closes the open entry, reads each noted space's `after` state from `desktop`,
    /// and pushes the entry if anything changed.
    pub(super) fn finish(&mut self, desktop: &Desktop) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.depth > 1 {
            open.depth -= 1;
            return;
        }
        let Some(open) = self.open.take() else {
            return;
        };
        if open.cancelled {
            return;
        }
        let spaces: Vec<SpaceChange> = open
            .spaces
            .into_iter()
            .filter_map(|(id, before)| {
                let after = space_state(desktop, id);
                (before != after).then_some(SpaceChange { id, before, after })
            })
            .collect();
        if spaces.is_empty() && open.files.is_empty() {
            return;
        }
        self.redo.clear();
        if self.undo.len() >= UNDO_DEPTH {
            self.undo.drain(..=self.undo.len() - UNDO_DEPTH);
        }
        self.undo.push(UndoEntry {
            label: open.label,
            spaces,
            files: open.files,
        });
    }
}

fn space_state(desktop: &Desktop, id: SpaceId) -> Option<(usize, Space)> {
    desktop
        .spaces()
        .iter()
        .enumerate()
        .find(|(_, space)| space.id == id)
        .map(|(index, space)| (index, space.clone()))
}

/// Outcome of an undo or redo step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StepOutcome {
    Empty,
    Stale(String),
    Applied {
        touched: Vec<SpaceId>,
        windows_changed: bool,
        folded: Vec<SpaceId>,
        files_changed: bool,
    },
}

fn stale_undo_text(label: &str) -> String {
    format!("DeskSpace can't undo \u{201C}{label}\u{201D}, because the space changed since.")
}

fn stale_redo_text(label: &str) -> String {
    format!("DeskSpace can't redo \u{201C}{label}\u{201D}, because the space changed since.")
}

impl App {
    pub(super) fn undo(&mut self, ui: &mut Ui) {
        match self.apply_undo_step(Some(ui)) {
            StepOutcome::Empty => {}
            StepOutcome::Stale(label) => {
                ui.alert("DeskSpace can't undo", &stale_undo_text(&label));
            }
            StepOutcome::Applied {
                touched,
                windows_changed,
                folded,
                files_changed,
            } => self.finish_restore(ui, touched, windows_changed, folded, files_changed),
        }
    }

    pub(super) fn redo(&mut self, ui: &mut Ui) {
        match self.apply_redo_step(Some(ui)) {
            StepOutcome::Empty => {}
            StepOutcome::Stale(label) => {
                ui.alert("DeskSpace can't redo", &stale_redo_text(&label));
            }
            StepOutcome::Applied {
                touched,
                windows_changed,
                folded,
                files_changed,
            } => self.finish_restore(ui, touched, windows_changed, folded, files_changed),
        }
    }

    #[cfg(test)]
    pub(super) fn apply_undo(&mut self) -> StepOutcome {
        self.apply_undo_step(None)
    }

    #[cfg(test)]
    pub(super) fn apply_redo(&mut self) -> StepOutcome {
        self.apply_redo_step(None)
    }

    fn apply_undo_step(&mut self, mut ui: Option<&mut Ui>) -> StepOutcome {
        let Some(mut entry) = self.undo.undo.pop() else {
            return StepOutcome::Empty;
        };
        if !entry.can_undo(&self.config.desktop, |path| path.exists()) {
            return StepOutcome::Stale(entry.label);
        }
        if !self.reverse_files(&mut ui, &mut entry.files, true) {
            return StepOutcome::Stale(entry.label);
        }
        let restore: Vec<(SpaceId, Option<(usize, Space)>)> = entry
            .spaces
            .iter()
            .map(|change| (change.id, change.before.clone()))
            .collect();
        let Some(outcome) = self.apply_restore_step(&entry, restore) else {
            return StepOutcome::Stale(entry.label);
        };
        self.undo.redo.push(entry);
        outcome
    }

    fn apply_redo_step(&mut self, mut ui: Option<&mut Ui>) -> StepOutcome {
        let Some(mut entry) = self.undo.redo.pop() else {
            return StepOutcome::Empty;
        };
        if !entry.can_redo(&self.config.desktop, |path| path.exists()) {
            return StepOutcome::Stale(entry.label);
        }
        if !self.reverse_files(&mut ui, &mut entry.files, false) {
            return StepOutcome::Stale(entry.label);
        }
        let restore: Vec<(SpaceId, Option<(usize, Space)>)> = entry
            .spaces
            .iter()
            .map(|change| (change.id, change.after.clone()))
            .collect();
        let Some(outcome) = self.apply_restore_step(&entry, restore) else {
            return StepOutcome::Stale(entry.label);
        };
        self.undo.undo.push(entry);
        outcome
    }

    /// Reverses `files` in reverse order on undo, or forward order on redo.
    /// Caller has already checked every change with `can_undo` or `can_redo`
    /// before touching the first file.
    fn reverse_files(
        &mut self,
        ui: &mut Option<&mut Ui>,
        files: &mut [FileChange],
        undo: bool,
    ) -> bool {
        let len = files.len();
        for step in 0..len {
            let index = if undo { len - 1 - step } else { step };
            let ok = match &files[index] {
                FileChange::Moved { from, to } => {
                    let (src, dst) = if undo {
                        (to.clone(), from.clone())
                    } else {
                        (from.clone(), to.clone())
                    };
                    self.reverse_move(ui, &src, &dst)
                }
                FileChange::Renamed { from, to } => {
                    let (src, dst) = if undo {
                        (to.clone(), from.clone())
                    } else {
                        (from.clone(), to.clone())
                    };
                    self.reverse_rename(ui, &src, &dst)
                }
                FileChange::Created { path } => {
                    let target = path.clone();
                    match self.reverse_created(ui, &target) {
                        Some(at) => {
                            files[index] = FileChange::Recycled {
                                original: target,
                                at,
                            };
                            true
                        }
                        None => false,
                    }
                }
                FileChange::Recycled { original, at } => {
                    let target = original.clone();
                    let since = *at;
                    match self.reverse_recycled(ui, &target, since) {
                        Some(restored) => {
                            files[index] = FileChange::Created { path: restored };
                            true
                        }
                        None => false,
                    }
                }
            };
            if !ok {
                return false;
            }
        }
        true
    }

    fn reverse_created(&self, ui: &mut Option<&mut Ui>, path: &Path) -> Option<SystemTime> {
        let at = SystemTime::now();
        let recycled = match ui.as_deref_mut() {
            Some(ui) => {
                ui.delete_files(std::slice::from_ref(&path.to_path_buf()), false) && !path.exists()
            }
            None => self.test_recycle(path),
        };
        recycled.then_some(at)
    }

    fn reverse_recycled(
        &self,
        ui: &mut Option<&mut Ui>,
        original: &Path,
        since: SystemTime,
    ) -> Option<PathBuf> {
        match ui.as_deref_mut() {
            Some(ui) => ui.restore_recycled(original, since),
            None => self.test_restore_recycled(original),
        }
    }

    fn test_recycle(&self, path: &Path) -> bool {
        let bin = self.storage.root().join(".test-recycle");
        let Some(name) = path.file_name() else {
            return false;
        };
        std::fs::create_dir_all(&bin).is_ok() && std::fs::rename(path, bin.join(name)).is_ok()
    }

    fn test_restore_recycled(&self, original: &Path) -> Option<PathBuf> {
        if original.exists() {
            return None;
        }
        let bin = self.storage.root().join(".test-recycle");
        let recycled = bin.join(original.file_name()?);
        std::fs::rename(&recycled, original)
            .ok()
            .map(|()| original.to_path_buf())
    }

    fn reverse_move(&self, ui: &mut Option<&mut Ui>, from: &Path, to: &Path) -> bool {
        match self.storage.move_between(from, to) {
            Ok(()) => {
                notify_moved(from, to);
                true
            }
            Err(StoreError::OtherVolume(_)) => {
                let (Some(ui), Some(parent), Some(target_name)) = (
                    ui.as_deref_mut(),
                    to.parent(),
                    to.file_name().and_then(|name| name.to_str()),
                ) else {
                    return false;
                };
                let Some(moved) =
                    ui.transfer_files(std::slice::from_ref(&from.to_path_buf()), parent, true)
                else {
                    return false;
                };
                let Some(landed) = moved.into_iter().next() else {
                    return false;
                };
                if landed != to && !ui.rename_path(&landed, target_name) {
                    return false;
                }
                to.exists() && !from.exists()
            }
            Err(error) => {
                tracing::warn!(%error, from = %from.display(), to = %to.display(), "could not reverse file move");
                false
            }
        }
    }

    fn reverse_rename(&mut self, ui: &mut Option<&mut Ui>, from: &Path, to: &Path) -> bool {
        let Some(new_name) = to.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let ok = match ui.as_deref_mut() {
            Some(ui) => ui.rename_path(from, new_name) && to.exists(),
            None => std::fs::rename(from, to).is_ok(),
        };
        if ok {
            self.icons.invalidate(from);
            self.icons.invalidate(to);
        }
        ok
    }

    fn apply_restore_step(
        &mut self,
        entry: &UndoEntry,
        spaces: Vec<(SpaceId, Option<(usize, Space)>)>,
    ) -> Option<StepOutcome> {
        let windows_changed = entry
            .spaces
            .iter()
            .any(|change| change.before.is_none() != change.after.is_none());
        let folded: Vec<SpaceId> = entry
            .spaces
            .iter()
            .filter_map(|change| {
                let (_, before) = change.before.as_ref()?;
                let (_, after) = change.after.as_ref()?;
                (before.folded() != after.folded()).then_some(change.id)
            })
            .collect();
        let touched: Vec<SpaceId> = entry.spaces.iter().map(|change| change.id).collect();
        if !self
            .dispatch(Command::RestoreSpaces { spaces }, Point::default())
            .is_applied()
        {
            return None;
        }
        for &id in &touched {
            match self.config.desktop.space(id) {
                None => {
                    if self.selection.surface == Some(Surface::Space(id)) {
                        self.selection.clear();
                    }
                    self.pointer.clear_hover_on(Surface::Space(id));
                    self.view_cache.forget(id);
                    if let Some(session) = self.settings_ui.session_mut() {
                        session.forget_space(id);
                    }
                }
                Some(space) => {
                    if self.selection.surface == Some(Surface::Space(id)) {
                        if space.folded() {
                            self.selection.clear();
                        } else {
                            self.selection
                                .items
                                .retain(|item| space.item(*item).is_some());
                        }
                    }
                    if let (Some(session), Some(space)) = (
                        self.settings_ui.session_mut(),
                        self.config.desktop.space(id),
                    ) {
                        session.resnapshot(space);
                    }
                }
            }
        }
        Some(StepOutcome::Applied {
            touched,
            windows_changed,
            folded,
            files_changed: !entry.files.is_empty(),
        })
    }

    fn finish_restore(
        &mut self,
        ui: &mut Ui,
        touched: Vec<SpaceId>,
        windows_changed: bool,
        folded: Vec<SpaceId>,
        files_changed: bool,
    ) {
        if windows_changed {
            self.sync_windows(ui);
        }
        for id in folded {
            self.run(ui, vec![Effect::FoldSpace(id)]);
        }
        for id in touched {
            if self.config.desktop.space(id).is_some() {
                self.restyle_space(ui, id);
            }
        }
        if files_changed && self.layer_active() {
            self.resync_desktop_layer(ui);
        }
        if self.settings_ui.is_open() {
            self.refresh_settings(ui);
        }
        self.refresh_tray(ui);
        self.save(ui);
    }

    /// Arranges `id`'s items onto the grid, recording an undo entry.
    #[cfg(test)]
    pub(super) fn arrange_space(&mut self, id: SpaceId, at: Point) -> Vec<Effect> {
        self.record_effects("Arrange space", Command::AutoArrange { id }, at, Vec::new())
    }

    /// Snaps every unlocked space to the grid, recording an undo entry.
    #[cfg(test)]
    pub(super) fn snap_all_spaces(&mut self, at: Point) -> Vec<Effect> {
        self.record_effects("Snap all", Command::SnapAll, at, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::UNIX_EPOCH;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::geometry::Size;
    use deskspace_core::{Command, Item, ItemId, Rect, ResizeEdges, SortKey};
    use deskspace_platform::FieldValue;

    use crate::fixtures::{add_item, apply, create_space};
    use crate::settings::{Session, field};

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("deskspace-undo-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample_rect(x: i32) -> Rect {
        Rect::new(x, 32, 320, 208)
    }

    fn test_app() -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        App::new(Config::new(), store, false, false)
    }

    fn test_app_in(root: &Path) -> App {
        let store = ConfigStore::with_path(root.join("deskspace.toml"));
        App::new(Config::new(), store, false, false)
    }

    #[test]
    fn entry_round_trip_captures_first_before_and_final_after() {
        let mut desktop = Desktop::default();
        let id = create_space(&mut desktop, "Work", sample_rect(16));
        let mut stack = UndoStack::default();

        stack.begin("Move space");
        stack.note_space(&desktop, id);
        apply(
            &mut desktop,
            Command::SetSpaceRect {
                id,
                rect: sample_rect(160),
            },
        );
        // Noting the same space a second time keeps the original `before`.
        stack.note_space(&desktop, id);
        apply(
            &mut desktop,
            Command::SetSpaceRect {
                id,
                rect: sample_rect(320),
            },
        );
        stack.finish(&desktop);

        assert_eq!(stack.undo.len(), 1);
        let entry = &stack.undo[0];
        assert_eq!(entry.label, "Move space");
        assert_eq!(entry.spaces.len(), 1);
        assert_eq!(entry.spaces[0].before.as_ref().unwrap().1.rect().x, 16);
        assert_eq!(entry.spaces[0].after.as_ref().unwrap().1.rect().x, 320);
        assert!(entry.can_undo(&desktop, |_| false));
        assert!(!entry.can_redo(&desktop, |_| false));

        // Restore the `before` state: now `can_redo` is true and `can_undo` is false.
        apply(
            &mut desktop,
            Command::RestoreSpaces {
                spaces: vec![(id, entry.spaces[0].before.clone())],
            },
        );
        assert!(!entry.can_undo(&desktop, |_| false));
        assert!(entry.can_redo(&desktop, |_| false));
    }

    #[test]
    fn finish_caps_depth_at_undo_depth() {
        let mut desktop = Desktop::default();
        let id = create_space(&mut desktop, "Work", sample_rect(0));
        let mut stack = UndoStack::default();

        for step in 1..=(UNDO_DEPTH + 5) {
            stack.begin(format!("Move {step}"));
            stack.note_space(&desktop, id);
            apply(
                &mut desktop,
                Command::SetSpaceRect {
                    id,
                    rect: sample_rect(step as i32 * 16),
                },
            );
            stack.finish(&desktop);
        }

        assert_eq!(stack.undo.len(), UNDO_DEPTH);
        assert_eq!(stack.undo.first().unwrap().label, "Move 6");
        assert_eq!(
            stack.undo.last().unwrap().label,
            format!("Move {}", UNDO_DEPTH + 5)
        );
    }

    #[test]
    fn new_entry_clears_redo_but_empty_action_does_not() {
        let mut desktop = Desktop::default();
        let id = create_space(&mut desktop, "Work", sample_rect(10));
        let mut stack = UndoStack::default();

        stack.begin("Rename space");
        stack.note_space(&desktop, id);
        apply(
            &mut desktop,
            Command::RenameSpace {
                id,
                title: "Play".into(),
            },
        );
        stack.finish(&desktop);
        stack.redo.push(stack.undo.pop().unwrap());

        // An empty action records nothing and leaves `redo` intact.
        stack.begin("No-op");
        stack.note_space(&desktop, id);
        stack.finish(&desktop);
        assert!(stack.undo.is_empty());
        assert_eq!(stack.redo.len(), 1);

        // A real change records a new undo entry and clears `redo`.
        stack.begin("Toggle fold");
        stack.note_space(&desktop, id);
        apply(&mut desktop, Command::ToggleFold { id });
        stack.finish(&desktop);
        assert_eq!(stack.undo.len(), 1);
        assert!(stack.redo.is_empty());
    }

    #[test]
    fn changed_space_or_moved_file_makes_entry_stale() {
        let mut desktop = Desktop::default();
        let id = create_space(&mut desktop, "Work", sample_rect(10));
        let mut stack = UndoStack::default();

        let from = PathBuf::from(r"C:\Users\alice\Desktop\a.txt");
        let to = PathBuf::from(r"C:\Store\a.txt");
        let renamed_from = PathBuf::from(r"C:\Store\old_name.txt");
        let renamed_to = PathBuf::from(r"C:\Store\new_name.txt");
        let created = PathBuf::from(r"C:\Store\copy.txt");
        let recycled = PathBuf::from(r"C:\Store\old.txt");

        stack.begin("Drop files");
        stack.note_space(&desktop, id);
        apply(&mut desktop, Command::ToggleLock { id });
        stack.note_file(FileChange::Moved {
            from: from.clone(),
            to: to.clone(),
        });
        stack.note_file(FileChange::Renamed {
            from: renamed_from,
            to: renamed_to.clone(),
        });
        stack.note_file(FileChange::Created {
            path: created.clone(),
        });
        stack.note_file(FileChange::Recycled {
            original: recycled.clone(),
            at: UNIX_EPOCH,
        });
        stack.finish(&desktop);

        let entry = &stack.undo[0];
        let mut disk: HashSet<PathBuf> = [to.clone(), renamed_to.clone(), created.clone()]
            .into_iter()
            .collect();
        assert!(entry.can_undo(&desktop, |p| disk.contains(p)));

        // Moving `to` away outside DeskSpace makes the entry stale.
        disk.remove(&to);
        assert!(!entry.can_undo(&desktop, |p| disk.contains(p)));
        disk.insert(to.clone());

        // Putting a new file at `from` or `recycled` makes the entry stale (would clobber).
        disk.insert(from.clone());
        assert!(!entry.can_undo(&desktop, |p| disk.contains(p)));
        disk.remove(&from);

        disk.insert(recycled);
        assert!(!entry.can_undo(&desktop, |p| disk.contains(p)));
        disk.clear();
        disk.insert(to);
        disk.insert(renamed_to);
        disk.insert(created);

        // Changing the space outside the entry makes it stale.
        apply(
            &mut desktop,
            Command::RenameSpace {
                id,
                title: "Edited since".into(),
            },
        );
        assert!(!entry.can_undo(&desktop, |p| disk.contains(p)));
    }

    #[test]
    fn move_and_resize_drags_record_once_on_release_and_round_trip() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", sample_rect(16));
        let initial_rect = app.config.desktop.space(id).unwrap().rect();

        // Multi-step space drag records one entry on release.
        app.drag_space_to(id, Point::new(0, 0), Point::new(160, 32));
        app.drag_space_to(id, Point::new(0, 0), Point::new(320, 32));
        assert!(app.undo.undo.is_empty());
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo.len(), 1);
        let moved_rect = app.config.desktop.space(id).unwrap().rect();
        assert_ne!(moved_rect, initial_rect);

        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), initial_rect);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), moved_rect);

        // Multi-step resize drag records one entry on release, and FitToContents undoes it.
        let right_edge = ResizeEdges {
            right: true,
            ..ResizeEdges::NONE
        };
        app.resize_space_to(
            id,
            right_edge,
            Point::new(moved_rect.right(), 100),
            (true, false),
            Point::new(moved_rect.right() + 200, 100),
        );
        app.resize_space_to(
            id,
            right_edge,
            Point::new(moved_rect.right(), 100),
            (true, false),
            Point::new(moved_rect.right() + 300, 100),
        );
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo.len(), 2);
        let resized_rect = app.config.desktop.space(id).unwrap().rect();
        assert!(resized_rect.width > moved_rect.width);

        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), moved_rect);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), resized_rect);

        // Fit to contents records one entry and undoes/redoes.
        app.fit_space_to_contents(id, resized_rect.center());
        assert_eq!(app.undo.undo_label(), Some("Fit to contents"));
        let fitted_rect = app.config.desktop.space(id).unwrap().rect();
        assert!(!app.config.desktop.space(id).unwrap().user_sized());
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), resized_rect);
        assert!(app.config.desktop.space(id).unwrap().user_sized());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), fitted_rect);
    }

    #[test]
    fn reorder_sort_arrange_and_snap_all_round_trip() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(23, 37, 480, 320));
        let first = add_item(&mut app.config.desktop, id, "C:/missing/b.txt");
        let second = add_item(&mut app.config.desktop, id, "C:/missing/a.txt");

        let order = |app: &App| -> Vec<ItemId> {
            app.config
                .desktop
                .space(id)
                .unwrap()
                .items()
                .iter()
                .map(|i| i.id)
                .collect()
        };
        assert_eq!(order(&app), vec![first, second]);

        // Reorder by dragging `first` onto `second`'s slot.
        let second_pos = {
            let space = app.config.desktop.space(id).unwrap();
            let content = space.content_rect();
            let pos = space.item(second).unwrap().position;
            Point::new(content.x + pos.x + 10, content.y + pos.y + 10)
        };
        assert!(app.reorder_item_step(id, first, second_pos));
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo_label(), Some("Move item"));
        assert_eq!(order(&app), vec![second, first]);
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(order(&app), vec![first, second]);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(order(&app), vec![second, first]);

        // Sort space descending by name (`b.txt` before `a.txt`).
        app.sort_space_items(id, SortKey::Name, true, Point::new(100, 100));
        assert_eq!(app.undo.undo_label(), Some("Sort space"));
        assert_eq!(order(&app), vec![first, second]);
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(order(&app), vec![second, first]);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(order(&app), vec![first, second]);

        // Arrange space (`AutoArrange`) after displacing an item off-grid.
        let mut displaced = Space::new(id, "Work", Rect::new(23, 37, 480, 320));
        let mut off_grid = Item::new(first, "C:/missing/b.txt", Default::default());
        off_grid.position = Point::new(333, 444);
        displaced.push_item(off_grid);
        apply(
            &mut app.config.desktop,
            Command::RestoreSpaces {
                spaces: vec![(id, Some((0, displaced)))],
            },
        );
        app.arrange_space(id, Point::new(100, 100));
        assert_eq!(app.undo.undo_label(), Some("Arrange space"));
        let arranged_pos = app
            .config
            .desktop
            .space(id)
            .unwrap()
            .item(first)
            .unwrap()
            .position;
        assert_ne!(arranged_pos, Point::new(333, 444));
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(
            app.config
                .desktop
                .space(id)
                .unwrap()
                .item(first)
                .unwrap()
                .position,
            Point::new(333, 444)
        );
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(
            app.config
                .desktop
                .space(id)
                .unwrap()
                .item(first)
                .unwrap()
                .position,
            arranged_pos
        );

        // Snap all (`SnapAll`) after placing a space off the 16 px grid.
        let unsnapped = app.config.desktop.space(id).unwrap().rect();
        assert_eq!(unsnapped.origin(), Point::new(23, 37));
        app.snap_all_spaces(Point::new(100, 100));
        assert_eq!(app.undo.undo_label(), Some("Snap all"));
        let snapped = app.config.desktop.space(id).unwrap().rect();
        assert_ne!(snapped.origin(), unsnapped.origin());
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), unsnapped);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(id).unwrap().rect(), snapped);
    }

    #[test]
    fn lock_fold_rename_create_delete_and_move_between_spaces_round_trip() {
        let mut app = test_app();
        let a = app
            .create_new_space(Point::new(32, 32), Size::new(320, 208))
            .unwrap();
        assert_eq!(app.undo.undo_label(), Some("New space"));
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(a).is_none());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(a).is_some());

        // Lock space.
        app.toggle_space_lock(a, Point::new(50, 50));
        assert!(app.config.desktop.space(a).unwrap().locked());
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(!app.config.desktop.space(a).unwrap().locked());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(a).unwrap().locked());
        app.toggle_space_lock(a, Point::new(50, 50));

        // Fold space.
        app.toggle_space_fold(a);
        assert!(app.config.desktop.space(a).unwrap().folded());
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(!app.config.desktop.space(a).unwrap().folded());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(a).unwrap().folded());
        app.toggle_space_fold(a);

        // Rename space.
        app.rename_space(a, "Docs".to_string());
        assert_eq!(app.config.desktop.space(a).unwrap().title(), "Docs");
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(a).unwrap().title(), "New space");
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(a).unwrap().title(), "Docs");

        // Move items between spaces.
        let b = app
            .create_new_space(Point::new(400, 32), Size::new(320, 208))
            .unwrap();
        let item1 = add_item(&mut app.config.desktop, a, "C:/missing/1.txt");
        let item2 = add_item(&mut app.config.desktop, a, "C:/missing/2.txt");
        app.move_items_to_space(&[item1, item2], b, Point::new(450, 50));
        assert_eq!(app.undo.undo_label(), Some("Move 2 items"));
        assert!(app.config.desktop.space(a).unwrap().items().is_empty());
        assert_eq!(app.config.desktop.space(b).unwrap().items().len(), 2);
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(a).unwrap().items().len(), 2);
        assert!(app.config.desktop.space(b).unwrap().items().is_empty());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(a).unwrap().items().is_empty());
        assert_eq!(app.config.desktop.space(b).unwrap().items().len(), 2);

        // Delete space with in-place items.
        app.undo.begin("Delete space");
        assert!(app.release_and_delete_space(b));
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo_label(), Some("Delete space"));
        assert!(app.config.desktop.space(b).is_none());
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(b).unwrap().items().len(), 2);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(b).is_none());
    }

    #[test]
    fn drop_delete_space_with_stored_files_and_rename_item_round_trip() {
        let scratch = Scratch::new("file-moves");
        let desktop_dir = scratch.0.join("Desktop");
        std::fs::create_dir_all(&desktop_dir).unwrap();
        let mut app = test_app_in(&scratch.0);
        let space = create_space(&mut app.config.desktop, "Work", sample_rect(16));

        let original_a = desktop_dir.join("alpha.txt");
        let original_b = desktop_dir.join("beta.txt");
        std::fs::write(&original_a, "alpha").unwrap();
        std::fs::write(&original_b, "beta").unwrap();

        // Drop two files into the space: they move into the store under one entry.
        app.move_or_keep_dropped(
            space,
            Point::new(50, 50),
            vec![original_a.clone(), original_b.clone()],
            true,
        );
        assert_eq!(app.undo.undo_label(), Some("Move 2 items"));
        let stored_a = app.storage.root().join("alpha.txt");
        let stored_b = app.storage.root().join("beta.txt");
        assert!(stored_a.exists() && stored_b.exists());
        assert!(!original_a.exists() && !original_b.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);

        // Undo the drop: both files return to `desktop_dir` and the space is empty again.
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(original_a.exists() && original_b.exists());
        assert!(!stored_a.exists() && !stored_b.exists());
        assert!(app.config.desktop.space(space).unwrap().items().is_empty());

        // Redo the drop: both files move back into the store and the items return.
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(stored_a.exists() && stored_b.exists());
        assert!(!original_a.exists() && !original_b.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);

        // Rename the first item in place: records `Renamed` and updates path + label.
        let item_a = app.config.desktop.space(space).unwrap().items()[0].id;
        assert!(
            app.rename_space_item(item_a, false, "gamma", |from, new_name| {
                std::fs::rename(from, from.with_file_name(new_name)).is_ok()
            })
        );
        assert_eq!(app.undo.undo_label(), Some("Rename item"));
        let stored_gamma = app.storage.root().join("gamma.txt");
        assert!(stored_gamma.exists() && !stored_a.exists());
        assert_eq!(app.config.desktop.item(item_a).unwrap().label, "gamma");

        // Undo and redo the item rename.
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(stored_a.exists() && !stored_gamma.exists());
        assert_eq!(app.config.desktop.item(item_a).unwrap().label, "alpha");
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(stored_gamma.exists() && !stored_a.exists());
        assert_eq!(app.config.desktop.item(item_a).unwrap().label, "gamma");

        // Delete the space with stored files: files release to `desktop_dir`.
        app.undo.begin("Delete space");
        assert!(app.release_and_delete_space_to(space, Some(&desktop_dir)));
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo_label(), Some("Delete space"));
        let released_gamma = desktop_dir.join("gamma.txt");
        assert!(app.config.desktop.space(space).is_none());
        assert!(released_gamma.exists() && original_b.exists());
        assert!(!stored_gamma.exists() && !stored_b.exists());

        // Undo the space delete: files return to the store and the space comes back.
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(space).is_some());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);
        assert!(stored_gamma.exists() && stored_b.exists());
        assert!(!released_gamma.exists() && !original_b.exists());

        // Redo the space delete: files release to `desktop_dir` and the space is gone again.
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(app.config.desktop.space(space).is_none());
        assert!(released_gamma.exists() && original_b.exists());
        assert!(!stored_gamma.exists() && !stored_b.exists());
    }

    #[test]
    fn stale_file_entry_touches_no_file() {
        let scratch = Scratch::new("stale-files");
        let desktop_dir = scratch.0.join("Desktop");
        std::fs::create_dir_all(&desktop_dir).unwrap();
        let mut app = test_app_in(&scratch.0);
        let space = create_space(&mut app.config.desktop, "Work", sample_rect(16));

        let original_a = desktop_dir.join("alpha.txt");
        let original_b = desktop_dir.join("beta.txt");
        std::fs::write(&original_a, "alpha").unwrap();
        std::fs::write(&original_b, "beta").unwrap();

        app.move_or_keep_dropped(
            space,
            Point::new(50, 50),
            vec![original_a.clone(), original_b.clone()],
            true,
        );
        let stored_a = app.storage.root().join("alpha.txt");
        let stored_b = app.storage.root().join("beta.txt");

        // Put a new file at `original_a` so reversing the first move would clobber it.
        std::fs::write(&original_a, "new file").unwrap();

        // Undo checks every file first, so `stored_b` is NOT moved back either.
        assert_eq!(
            app.apply_undo(),
            StepOutcome::Stale("Move 2 items".to_string())
        );
        assert!(stored_a.exists() && stored_b.exists());
        assert!(!original_b.exists());
        assert_eq!(std::fs::read_to_string(&original_a).unwrap(), "new file");
    }

    #[test]
    fn settings_edit_records_no_entry_and_makes_prior_space_entry_stale() {
        let mut app = test_app();
        let id = create_space(&mut app.config.desktop, "Work", sample_rect(16));
        app.rename_space(id, "Renamed".to_string());
        assert_eq!(app.undo.undo.len(), 1);

        // A settings-window edit on the space dispatches without an open undo
        // entry, so it records nothing and makes the earlier entry stale.
        let mut session = Session::new(&app.config);
        session.select_scope(Some(id));
        let customize = crate::settings::customize_space(&app.config, &mut session, true).unwrap();
        assert!(app.dispatch(customize, Point::new(50, 50)).is_applied());
        let (_, command, _) = crate::settings::space_edit(
            &app.config,
            &mut session,
            field::TINT_OPACITY,
            FieldValue::Slider(60),
            false,
        )
        .unwrap();
        assert!(app.dispatch(command, Point::new(50, 50)).is_applied());
        assert_eq!(app.undo.undo.len(), 1);

        assert_eq!(
            app.apply_undo(),
            StepOutcome::Stale("Rename space".to_string())
        );
        assert!(app.undo.undo.is_empty());
        assert_eq!(
            stale_undo_text("Move 3 items"),
            "DeskSpace can't undo \u{201C}Move 3 items\u{201D}, because the space changed since."
        );
    }

    #[test]
    fn deletes_copies_and_shortcuts_round_trip_and_shift_delete_records_nothing() {
        use crate::app::files::ShortcutSource;
        use deskspace_platform::{CustomShortcut, ShortcutTarget, item_facts};

        let scratch = Scratch::new("deletes-copies-shortcuts");
        let desktop_dir = scratch.0.join("Desktop");
        std::fs::create_dir_all(&desktop_dir).unwrap();
        let mut app = test_app_in(&scratch.0);
        let space = create_space(&mut app.config.desktop, "Work", sample_rect(16));

        // 1. Create a custom URL shortcut through `create_shortcuts`.
        assert!(app.create_shortcuts(
            space,
            Point::new(50, 50),
            vec![ShortcutSource::Custom(CustomShortcut {
                name: "Docs".to_string(),
                target: ShortcutTarget::Url("https://example.com".to_string()),
                arguments: String::new(),
            })],
        ));
        assert_eq!(app.undo.undo_label(), Some("New shortcut"));
        let shortcut_path = app.storage.root().join("Docs.url");
        assert!(shortcut_path.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 1);

        // Undo recycles the shortcut and turns the record into `Recycled`; redo restores it.
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(!shortcut_path.exists());
        assert!(app.config.desktop.space(space).unwrap().items().is_empty());
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(shortcut_path.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 1);

        // 2. Record a pasted copy (`FileChange::Created`) and undo/redo it.
        let copy_path = app.storage.root().join("copied.txt");
        std::fs::write(&copy_path, "copy").unwrap();
        app.undo.begin(paste_items_label(1));
        app.undo.note_file(FileChange::Created {
            path: copy_path.clone(),
        });
        assert!(
            app.dispatch(
                Command::AddItem {
                    space,
                    path: copy_path.clone(),
                    facts: item_facts(&copy_path),
                },
                Point::new(50, 50),
            )
            .is_applied()
        );
        app.undo.finish(&app.config.desktop);
        assert_eq!(app.undo.undo_label(), Some("Paste item"));
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(!copy_path.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 1);
        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(copy_path.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);

        // 3. Delete two items to the Recycle Bin, undo, and redo.
        let items: Vec<_> = app
            .config
            .desktop
            .space(space)
            .unwrap()
            .items()
            .iter()
            .map(|item| item.id)
            .collect();
        let bin = app.storage.root().join(".test-recycle");
        std::fs::create_dir_all(&bin).unwrap();
        app.remove_space_items(space, items, false, |paths, _| {
            for path in paths {
                std::fs::rename(path, bin.join(path.file_name().unwrap())).unwrap();
            }
        });
        assert_eq!(app.undo.undo_label(), Some("Delete 2 items"));
        assert!(!shortcut_path.exists() && !copy_path.exists());
        assert!(app.config.desktop.space(space).unwrap().items().is_empty());

        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert!(shortcut_path.exists() && copy_path.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);

        assert!(matches!(app.apply_redo(), StepOutcome::Applied { .. }));
        assert!(!shortcut_path.exists() && !copy_path.exists());
        assert!(app.config.desktop.space(space).unwrap().items().is_empty());

        // Restore them again so we can test `remove_space_item_at` and Shift+Delete.
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));

        // 4. `remove_space_item_at` on an in-place item records only a model change.
        let in_place = desktop_dir.join("keep.txt");
        std::fs::write(&in_place, "keep").unwrap();
        app.move_or_keep_dropped(space, Point::new(50, 50), vec![in_place.clone()], false);
        assert!(app.remove_space_item_at(space, 2, |_| unreachable!("not stored")));
        assert_eq!(app.undo.undo_label(), Some("Delete item"));
        assert!(in_place.exists());
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 2);
        assert!(matches!(app.apply_undo(), StepOutcome::Applied { .. }));
        assert_eq!(app.config.desktop.space(space).unwrap().items().len(), 3);

        // 5. Shift+Delete (`permanent = true`) records nothing.
        let undo_count = app.undo.undo.len();
        let first_id = app.config.desktop.space(space).unwrap().items()[0].id;
        app.remove_space_items(space, vec![first_id], true, |paths, _| {
            for path in paths {
                std::fs::remove_file(path).unwrap();
            }
        });
        assert_eq!(app.undo.undo.len(), undo_count);
    }

    #[test]
    fn emptying_recycle_bin_before_undo_is_stale_and_changes_nothing() {
        let scratch = Scratch::new("empty-recycle-bin");
        let mut app = test_app_in(&scratch.0);
        let space = create_space(&mut app.config.desktop, "Work", sample_rect(16));

        let stored = app.storage.root().join("gone.txt");
        std::fs::create_dir_all(app.storage.root()).unwrap();
        std::fs::write(&stored, "gone").unwrap();
        let item = add_item(&mut app.config.desktop, space, &stored.to_string_lossy());

        let bin = app.storage.root().join(".test-recycle");
        std::fs::create_dir_all(&bin).unwrap();
        app.remove_space_items(space, vec![item], false, |paths, _| {
            for path in paths {
                std::fs::rename(path, bin.join(path.file_name().unwrap())).unwrap();
            }
        });
        assert_eq!(app.undo.undo_label(), Some("Delete item"));

        // Empty the Recycle Bin before undoing.
        std::fs::remove_dir_all(&bin).unwrap();

        assert_eq!(
            app.apply_undo(),
            StepOutcome::Stale("Delete item".to_string())
        );
        assert!(app.config.desktop.space(space).unwrap().items().is_empty());
        assert!(!stored.exists());
    }
}
