#![expect(dead_code, reason = "U3 adds the first caller")]

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use deskspace_core::{Desktop, Space, SpaceId};

/// Maximum number of undo entries kept in the session history (`ROADMAP.md` D11).
pub(super) const UNDO_DEPTH: usize = 50;

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
}

/// Session-only undo and redo stacks (`ROADMAP.md` D11).
#[derive(Debug, Default)]
pub(super) struct UndoStack {
    pub undo: Vec<UndoEntry>,
    pub redo: Vec<UndoEntry>,
    open: Option<OpenEntry>,
}

impl UndoStack {
    /// Starts recording a user action under `label`.
    pub(super) fn begin(&mut self, label: impl Into<String>) {
        self.open = Some(OpenEntry {
            label: label.into(),
            spaces: Vec::new(),
            files: Vec::new(),
        });
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
        let Some(open) = self.open.take() else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::UNIX_EPOCH;

    use deskspace_core::{Command, Rect};

    use crate::fixtures::{apply, create_space};

    use super::*;

    fn sample_rect(x: i32) -> Rect {
        Rect::new(x, 32, 320, 208)
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
        let created = PathBuf::from(r"C:\Store\copy.txt");
        let recycled = PathBuf::from(r"C:\Store\old.txt");

        stack.begin("Drop files");
        stack.note_space(&desktop, id);
        apply(&mut desktop, Command::ToggleLock { id });
        stack.note_file(FileChange::Moved {
            from: from.clone(),
            to: to.clone(),
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
        let mut disk: HashSet<PathBuf> = [to.clone(), created.clone()].into_iter().collect();
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
}
