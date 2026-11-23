//! `App` files: drops, storage, deletion, and desktop reconciliation.

mod delete;
mod shortcuts;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, Space, SpaceId};
use deskspace_platform::{
    DropAction, OnCollision, ShellItem, Ui, desktop_directory, is_inside, item_facts, notify_moved,
};

use super::undo::{FileChange, copy_items_label, move_items_label};
use super::{App, Surface};
use crate::storage::{path_key, same_path};

pub(super) use delete::stored_paths;
pub(super) use shortcuts::ShortcutSource;

/// The dropped `paths` worth handing to the shell for `folder`, in order. A
/// path already directly in `folder`, `folder` itself, and any folder that
/// contains `folder` are left out: the first would not change, and the others
/// cannot move into themselves.
fn paths_to_transfer(paths: &[PathBuf], folder: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| {
            let already_there = path
                .parent()
                .is_some_and(|parent| same_path(parent, folder));
            !already_there && !same_path(path, folder) && !is_inside(folder, path)
        })
        .cloned()
        .collect()
}

/// Where a deleted space's stored files go. Items listed in place are only
/// unlisted either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum StoredFiles {
    /// Back to the desktop, Delete space's default (ROADMAP D3).
    ToDesktop,
    /// To the Recycle Bin: Delete space's "Delete them", and the debug
    /// window's Delete all spaces (ROADMAP D14), so test files never pile up
    /// on the desktop.
    ToRecycleBin,
}

impl App {
    /// A file drop on `surface`: moves, keeps, or copies the files into the
    /// space, or opens a space at the desktop layer.
    pub(super) fn on_files_dropped(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        at: Point,
        paths: Vec<PathBuf>,
        action: DropAction,
    ) {
        self.drop_onto(ui, surface, at, |app, ui, space| {
            app.add_dropped(ui, space, at, paths, action);
        });
    }

    /// A dropped shell item (a Start-menu or taskbar app) becomes a shortcut
    /// in the space, or in a new space at the desktop layer.
    pub(super) fn on_shell_items_dropped(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        at: Point,
        items: Vec<ShellItem>,
    ) {
        let sources = items.into_iter().map(ShortcutSource::Shell).collect();
        self.drop_onto(ui, surface, at, |app, ui, space| {
            app.add_shortcuts(ui, space, at, sources);
        });
    }

    /// Runs `add` on the space a drop on `surface` at `at` goes into: that
    /// space, or, for the desktop layer, a new space opened at the drop point
    /// (ROADMAP D7). A new space the drop left empty, because nothing could
    /// be added, closes again.
    pub(super) fn drop_onto(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        at: Point,
        add: impl FnOnce(&mut Self, &mut Ui, SpaceId),
    ) {
        if surface == Surface::Desktop {
            self.undo.begin("New space");
        }
        let target = match surface {
            Surface::Space(id) => id,
            Surface::Desktop => match self.create_space_for_drop(ui, at) {
                Some(id) => id,
                None => {
                    self.undo.cancel();
                    return;
                }
            },
        };
        add(self, ui, target);
        let left_empty = surface == Surface::Desktop
            && self
                .config
                .desktop
                .space(target)
                .is_some_and(|space| space.items().is_empty());
        if left_empty {
            self.delete_space(ui, target);
            self.undo.cancel();
            return;
        }
        if surface == Surface::Desktop {
            self.undo.finish(&self.config.desktop);
        }
    }

    pub(super) fn add_dropped(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        paths: Vec<PathBuf>,
        action: DropAction,
    ) {
        // A lens shows a real folder (ROADMAP D3): the files go there through
        // the shell, never into the store or the lens's item list.
        if let Some(folder) = self.lens_folder(target) {
            self.drop_into_lens(ui, folder, paths, action);
            return;
        }
        let touched = match action {
            DropAction::Move => self.move_or_keep_dropped(target, at, paths, true),
            DropAction::KeepInPlace => self.move_or_keep_dropped(target, at, paths, false),
            DropAction::Copy => {
                self.undo.begin(copy_items_label(paths.len()));
                let mut touched = HashSet::from([target]);
                let mut count = 0usize;
                for path in paths {
                    if !path.exists() {
                        tracing::warn!(path = %path.display(), "dropped file does not exist; ignored");
                        continue;
                    }
                    let command = match self.find_item_by_file(&path) {
                        Some(item) => {
                            match self.config.desktop.space_containing(item) {
                                Some(source) if source == target => continue,
                                Some(source) => {
                                    touched.insert(source);
                                }
                                None => {}
                            }
                            Command::MoveItem { id: item, target }
                        }
                        None => {
                            let Some(placed) = self.copy_dropped(ui, path) else {
                                continue;
                            };
                            Command::AddItem {
                                space: target,
                                facts: item_facts(&placed),
                                path: placed,
                            }
                        }
                    };
                    if self.dispatch(command, at).is_applied() {
                        count += 1;
                    }
                }
                if count > 0 {
                    self.undo.set_label(copy_items_label(count));
                }
                self.undo.finish(&self.config.desktop);
                touched
            }
        };
        for id in &touched {
            self.refresh_space(ui, *id);
        }
        // A loose desktop item that was dropped in has moved off the desktop,
        // so reconcile the layer now instead of at the next shell refresh.
        if self.layer_active() {
            self.resync_desktop_layer(ui);
        }
        self.refresh_tray(ui);
        self.save(ui);
    }

    /// Moves or lists `paths` into `target`, recording one undo entry and
    /// returning the spaces that changed.
    pub(super) fn move_or_keep_dropped(
        &mut self,
        target: SpaceId,
        at: Point,
        paths: Vec<PathBuf>,
        move_into_store: bool,
    ) -> HashSet<SpaceId> {
        self.undo.begin(move_items_label(paths.len()));
        let index = self.item_index();
        let mut handled = HashSet::new();
        let mut touched = HashSet::from([target]);
        let mut count = 0usize;
        for path in paths {
            // A drop can name a file that no longer exists (for example a
            // stale desktop icon). Listing it would show a label without an
            // icon, so it is skipped.
            if !path.exists() {
                tracing::warn!(path = %path.display(), "dropped file does not exist; ignored");
                continue;
            }
            let key = path_key(&path);
            // The same file can arrive twice, in two spellings. The index does
            // not see an item added below, so handle each key once.
            if !handled.insert(key.clone()) {
                continue;
            }
            let command = match index.get(&key) {
                Some(&item) => {
                    match self.config.desktop.space_containing(item) {
                        Some(source) if source == target => continue,
                        Some(source) => {
                            touched.insert(source);
                        }
                        None => {}
                    }
                    Command::MoveItem { id: item, target }
                }
                None => {
                    let placed = if move_into_store {
                        self.place_dropped(path)
                    } else {
                        Some(path)
                    };
                    match placed {
                        Some(path) => Command::AddItem {
                            space: target,
                            facts: item_facts(&path),
                            path,
                        },
                        None => continue,
                    }
                }
            };
            // Not `commit`: one of several commands, refreshed once below.
            if self.dispatch(command, at).is_applied() {
                count += 1;
            }
        }
        if count > 0 {
            self.undo.set_label(move_items_label(count));
        }
        self.undo.finish(&self.config.desktop);
        touched
    }

    /// Moves `items` from their current spaces into `target`, recording one
    /// undo entry. Returns the spaces that changed.
    pub(super) fn move_items_to_space(
        &mut self,
        items: &[ItemId],
        target: SpaceId,
        at: Point,
    ) -> HashSet<SpaceId> {
        let mut touched = HashSet::from([target]);
        let mut moved = 0usize;
        self.undo.begin(move_items_label(items.len()));
        for &item in items {
            match self.config.desktop.space_containing(item) {
                Some(source) if source == target => continue,
                Some(source) => {
                    touched.insert(source);
                }
                None => {}
            }
            if self
                .dispatch(Command::MoveItem { id: item, target }, at)
                .is_applied()
            {
                moved += 1;
            }
        }
        if moved > 0 {
            self.undo.set_label(move_items_label(moved));
        }
        self.undo.finish(&self.config.desktop);
        touched
    }

    /// Moves or copies dropped `paths` into a lens's `folder` with the shell's
    /// file operation, which asks before replacing a file. The lens's watch
    /// lists the new files; the normal spaces that listed the moved ones are
    /// pruned.
    pub(super) fn drop_into_lens(
        &mut self,
        ui: &mut Ui,
        folder: PathBuf,
        paths: Vec<PathBuf>,
        action: DropAction,
    ) {
        if !folder.is_dir() {
            tracing::warn!(folder = %folder.display(), "lens folder is missing; drop ignored");
            return;
        }
        if !self.storage.allows_lens(&folder) {
            return;
        }
        let index = self.item_index();
        let listing_spaces: HashSet<SpaceId> = paths
            .iter()
            .filter_map(|path| index.get(&path_key(path)))
            .filter_map(|item| self.config.desktop.space_containing(*item))
            .collect();
        let paths = paths_to_transfer(&paths, &folder);
        if paths.is_empty() {
            return;
        }
        let move_items = match action {
            DropAction::Move => true,
            DropAction::Copy => false,
            // Keeping a file in place means nothing for a folder. The platform
            // never offers it over a lens; the arm keeps the match total.
            DropAction::KeepInPlace => {
                tracing::debug!("keep-in-place drop on a lens ignored");
                return;
            }
        };
        ui.transfer_files(&paths, &folder, move_items, OnCollision::Ask);
        for id in listing_spaces {
            self.prune_missing(ui, id);
        }
    }

    /// Re-reads the shell's loose desktop items and reconciles the layer. Used
    /// after an operation that moved or renamed a loose file, so the layer
    /// follows without waiting for the next shell change.
    pub(super) fn resync_desktop_layer(&mut self, ui: &mut Ui) {
        if !self.layer_active() {
            return;
        }
        self.refresh_desktop_entries();
        self.sync_desktop_layer(ui);
    }

    /// Where a moved dropped file will live: in the storage folder. `None` if
    /// it vanished before it could be moved.
    fn place_dropped(&mut self, path: PathBuf) -> Option<PathBuf> {
        match self.storage.adopt(&path) {
            Ok(stored) => {
                if stored != path {
                    tracing::info!(from = %path.display(), to = %stored.display(), "moved into space storage");
                    notify_moved(&path, &stored);
                    self.undo.note_file(FileChange::Moved {
                        from: path,
                        to: stored.clone(),
                    });
                }
                Some(stored)
            }
            Err(error) if !path.exists() => {
                tracing::warn!(%error, "dropped file disappeared; ignored");
                None
            }
            Err(error) => {
                tracing::info!(%error, "keeping dropped file in place");
                Some(path)
            }
        }
    }

    /// Copies a dropped file into the store through the shell's file operation,
    /// leaving the original where it is, as a Copy drop does and a paste
    /// copies. Returns the new path, or `None` when the copy failed.
    fn copy_dropped(&mut self, ui: &mut Ui, path: PathBuf) -> Option<PathBuf> {
        let root = self.storage.root();
        if let Err(error) = std::fs::create_dir_all(root) {
            tracing::warn!(%error, root = %root.display(), "cannot create the space storage folder");
            return None;
        }
        let created = ui.transfer_files(
            std::slice::from_ref(&path),
            root,
            false,
            OnCollision::Rename,
        )?;
        let placed = created.into_iter().next()?;
        if placed.exists() {
            self.undo.note_file(FileChange::Created {
                path: placed.clone(),
            });
        }
        Some(placed)
    }

    /// Takes an item out of its space when the space is deleted. A file that
    /// was moved into the space storage goes back to the desktop first, so
    /// nothing is left behind in a folder the user never sees. Returns false
    /// if that move failed; the item then stays.
    pub(super) fn release_item(&mut self, id: ItemId, desktop_override: Option<&Path>) -> bool {
        let Some(path) = self.config.desktop.item(id).map(|item| item.path.clone()) else {
            return false;
        };
        let in_lens = self
            .config
            .desktop
            .space_containing(id)
            .and_then(|space| self.config.desktop.space(space))
            .is_some_and(Space::is_lens);
        if !in_lens && self.storage.contains(&path) && path.exists() {
            let resolved;
            let desktop = match desktop_override {
                Some(dir) => dir,
                None => match desktop_directory() {
                    Ok(dir) => {
                        resolved = dir;
                        &resolved
                    }
                    Err(error) => {
                        tracing::error!(%error, "cannot find the desktop folder");
                        return false;
                    }
                },
            };
            match self.storage.release(&path, desktop) {
                Ok(back) => {
                    tracing::info!(to = %back.display(), "moved back to the desktop");
                    self.icons.invalidate(&back);
                    notify_moved(&path, &back);
                    self.undo.note_file(FileChange::Moved {
                        from: path,
                        to: back,
                    });
                }
                Err(error) => {
                    tracing::error!(%error, "could not move the item back to the desktop");
                    return false;
                }
            }
        }
        let near = self
            .config
            .desktop
            .space_containing(id)
            .map_or(Point::ZERO, |space| self.space_center(space));
        // Not `commit`: the caller loops over items and refreshes once.
        self.dispatch(Command::RemoveItem { id }, near).is_applied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_file_already_in_the_folder_is_not_transferred() {
        let kept = paths_to_transfer(
            &paths(&["C:/Lens/a.txt", "c:/lens/b.txt"]),
            Path::new("C:/Lens"),
        );
        assert!(kept.is_empty());
    }

    #[test]
    fn the_folder_itself_is_not_transferred() {
        let kept = paths_to_transfer(&paths(&["C:/Lens"]), Path::new("C:/lens"));
        assert!(kept.is_empty());
    }

    #[test]
    fn a_folder_containing_the_target_is_not_transferred() {
        let kept = paths_to_transfer(
            &paths(&["C:/Users", "C:/Users/me"]),
            Path::new("C:/Users/me/Lens"),
        );
        assert!(kept.is_empty());
    }

    #[test]
    fn a_mixed_drop_keeps_only_the_valid_paths_in_order() {
        let kept = paths_to_transfer(
            &paths(&[
                "C:/Other/z.txt",
                "C:/Lens/a.txt",
                "C:/Users",
                "C:/Other/b.txt",
                "C:/Users/me/Lens",
            ]),
            Path::new("C:/Users/me/Lens"),
        );
        assert_eq!(
            kept,
            paths(&["C:/Other/z.txt", "C:/Lens/a.txt", "C:/Other/b.txt"])
        );
    }
}
