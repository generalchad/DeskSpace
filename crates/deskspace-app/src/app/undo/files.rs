//! Reversing file changes on undo and redo, and rolling back a partial step.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use deskspace_platform::{OnCollision, Ui, WatchKey, notify_moved};

use crate::storage::StoreError;

use super::{App, FileChange, Reversed};

pub(super) fn rollback_failed_text(action: &str, label: &str, paths: &[PathBuf]) -> String {
    let paths: Vec<String> = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    format!(
        "DeskSpace could not put these files back after {action} \u{201C}{label}\u{201D}. \
         They are not where they were:\n{}",
        paths.join("\n")
    )
}

/// The path a change's file is known by from the entry's state before it was
/// reversed: where an undo or redo is trying to put it back.
fn original_path(change: &FileChange, undo: bool) -> PathBuf {
    match change {
        FileChange::Moved { from, to } | FileChange::Renamed { from, to } => {
            if undo {
                to.clone()
            } else {
                from.clone()
            }
        }
        FileChange::Created { path } => path.clone(),
        FileChange::Recycled { original, .. } => original.clone(),
    }
}

impl App {
    /// Reverses `files` in reverse order on undo, or forward order on redo.
    /// Caller has already checked every change with `can_undo` or `can_redo`
    /// before touching the first file. A step that fails after earlier ones
    /// were applied puts those back, so the undo or redo either applies the
    /// whole entry or leaves it as it was.
    pub(super) fn reverse_files(
        &mut self,
        ui: &mut Option<&mut Ui>,
        files: &mut [FileChange],
        undo: bool,
    ) -> Reversed {
        let before = files.to_vec();
        let len = files.len();
        for step in 0..len {
            let index = if undo { len - 1 - step } else { step };
            if !self.reverse_step(ui, files, index, undo) {
                return self.roll_back(ui, files, &before, step, undo);
            }
        }
        Reversed::Done
    }

    /// Puts back the steps `reverse_files` applied before one failed, newest
    /// first, and reports the files left out of place when that fails too. A
    /// `Created` change has become `Recycled` by then, and the other way
    /// round, so the same step with the direction flipped is its own reverse.
    pub(super) fn roll_back(
        &mut self,
        ui: &mut Option<&mut Ui>,
        files: &mut [FileChange],
        before: &[FileChange],
        applied: usize,
        undo: bool,
    ) -> Reversed {
        let len = files.len();
        for step in (0..applied).rev() {
            let index = if undo { len - 1 - step } else { step };
            if !self.reverse_step(ui, files, index, !undo) {
                let mut paths = Vec::new();
                for remaining in (0..=step).rev() {
                    let index = if undo { len - 1 - remaining } else { remaining };
                    let path = original_path(&before[index], undo);
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
                for path in &paths {
                    tracing::warn!(
                        path = %path.display(),
                        "undo rollback could not put a file back"
                    );
                }
                return Reversed::Failed(paths);
            }
        }
        Reversed::Stale
    }

    /// Reverses one change in `files` and returns whether it could be applied.
    /// A `Created` change becomes `Recycled` and the other way round, so
    /// applying the same step again reverses it.
    fn reverse_step(
        &mut self,
        ui: &mut Option<&mut Ui>,
        files: &mut [FileChange],
        index: usize,
        undo: bool,
    ) -> bool {
        match &files[index] {
            FileChange::Moved { from, to } => {
                let (src, dst) = if undo {
                    (to.clone(), from.clone())
                } else {
                    (from.clone(), to.clone())
                };
                let ok = self.reverse_move(ui, &src, &dst);
                if ok {
                    self.icons.invalidate(&src);
                    self.icons.invalidate(&dst);
                }
                ok
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
                        self.icons.invalidate(&target);
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
                        self.icons.invalidate(&target);
                        self.icons.invalidate(&restored);
                        files[index] = FileChange::Created { path: restored };
                        true
                    }
                    None => false,
                }
            }
        }
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
                    to.file_name().and_then(OsStr::to_str),
                ) else {
                    return false;
                };
                let Some(moved) = ui.transfer_files(
                    std::slice::from_ref(&from.to_path_buf()),
                    parent,
                    true,
                    OnCollision::Rename,
                ) else {
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
        let Some(new_name) = to.file_name().and_then(OsStr::to_str) else {
            return false;
        };
        let ok = match ui.as_deref_mut() {
            Some(ui) => ui.rename_path(from, new_name) && to.exists(),
            None => std::fs::rename(from, to).is_ok(),
        };
        if ok {
            self.icons.invalidate(from);
            self.icons.invalidate(to);
            // A lens keeps its items out of undo, so the model still names
            // `from`. Tell its worker about `to`, so the next listing keeps the
            // item's id and with it the selection.
            if let (Some(ui), Some((space, item, modified))) =
                (ui.as_deref_mut(), self.lens_item_at(from))
            {
                ui.note_lens_item(WatchKey::Space(space), to.to_path_buf(), item, modified);
            }
        }
        ok
    }
}
