//! Deleting spaces and items, pruning missing files, and returning stored files.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use deskspace_core::{Command, ItemId, Space, SpaceId};
use deskspace_platform::{IconUser, Ui, file_is_gone};

use super::super::undo::{FileChange, delete_items_label};
use super::super::{App, Surface, delete_consequence, delete_needs_confirmation};
use super::StoredFiles;
use crate::storage::ItemStore;

/// The answers to deleting a space that holds stored files, in button order,
/// with where each sends them; `None` is Cancel. The first is the default, so
/// Enter moves them to the desktop as Delete space always has. Cancel stays
/// last: `Ui::choose` reports Esc as `IDCANCEL`, which is 2.
const STORED_FILES_CHOICES: [(&str, Option<StoredFiles>); 3] = [
    ("Move to the desktop", Some(StoredFiles::ToDesktop)),
    ("Delete them", Some(StoredFiles::ToRecycleBin)),
    ("Cancel", None),
];

/// Where the button at `answer` of `STORED_FILES_CHOICES` sends the stored
/// files, or `None` to keep the space (Cancel, Esc, or a closed dialog).
fn stored_files_for(answer: Option<usize>) -> Option<StoredFiles> {
    answer
        .and_then(|index| STORED_FILES_CHOICES.get(index))
        .and_then(|(_, stored)| *stored)
}

impl App {
    /// Deletes every selected item's file with the shell, then drops the items
    /// whose files are gone. Unless `permanent` is set they go to the Recycle
    /// Bin, as Explorer's Delete key does; `permanent` is Shift+Delete. A file
    /// the shell could not delete (for example one in use) stays listed. An item
    /// whose file is already missing is dropped without a shell operation. This
    /// is distinct from deleting a space, which returns its stored files to
    /// the desktop unless the user chooses to delete them.
    pub(in crate::app) fn delete_selected_items(&mut self, ui: &mut Ui, permanent: bool) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        let items = self.active_items();
        if items.is_empty() {
            return;
        }
        self.selection.clear_items();
        self.pointer.clear_hover();
        match surface {
            Surface::Space(space) => {
                self.remove_space_items(space, items, permanent, |paths, permanent| {
                    ui.delete_files(paths, permanent);
                });
                self.refresh_space(ui, space);
                self.refresh_tray(ui);
                self.save(ui);
            }
            // The layer mirrors the Desktop folder, so re-reading it drops
            // whatever the shell removed.
            Surface::Desktop => {
                let paths: Vec<PathBuf> = items
                    .iter()
                    .filter_map(|item| self.config.desktop.item(*item))
                    .filter(|item| item.path.exists())
                    .map(|item| item.path.clone())
                    .collect();
                if !paths.is_empty() {
                    ui.delete_files(&paths, permanent);
                }
                self.resync_desktop_layer(ui);
            }
        }
    }

    /// Deletes `items` from `space` using `delete_files`, recording an undo
    /// entry unless `permanent` is set.
    pub(in crate::app) fn remove_space_items(
        &mut self,
        space: SpaceId,
        items: Vec<ItemId>,
        permanent: bool,
        delete_files: impl FnOnce(&[PathBuf], bool),
    ) {
        if !permanent {
            self.undo.begin(delete_items_label(items.len()));
        }
        let paths: Vec<PathBuf> = items
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .filter(|item| item.path.exists())
            .map(|item| item.path.clone())
            .collect();
        if !paths.is_empty() {
            let at = SystemTime::now();
            delete_files(&paths, permanent);
            if !permanent {
                for path in paths {
                    if !path.exists() {
                        self.undo
                            .note_file(FileChange::Recycled { original: path, at });
                    }
                }
            }
        }
        let near = self.space_center(space);
        let mut removed = 0usize;
        for item in items {
            let gone = self
                .config
                .desktop
                .item(item)
                .is_none_or(|item| !item.path.exists());
            if gone {
                self.forget_item(item);
                // Not `commit`: the file already moved, so the caller saves once.
                if self
                    .dispatch(Command::RemoveItem { id: item }, near)
                    .is_applied()
                {
                    removed += 1;
                }
            }
        }
        if !permanent {
            if removed > 0 {
                self.undo.set_label(delete_items_label(removed));
            }
            self.undo.finish(&self.config.desktop);
        }
    }

    /// Deletes the `index`-th item of space `space` after asking, from the X
    /// on the settings window's Shortcuts page (ROADMAP D3): a file in the
    /// space storage goes to the Recycle Bin, and an item listed in place is
    /// only removed from the space, so the user's original is never deleted.
    /// If the shell cannot delete the file (for example it is in use), the
    /// item stays.
    pub(in crate::app) fn delete_space_item(&mut self, ui: &mut Ui, space: SpaceId, index: usize) {
        let Some((path, label)) = self
            .config
            .desktop
            .space(space)
            .and_then(|space| space.items().get(index))
            .map(|item| (item.path.clone(), item.label.clone()))
        else {
            return;
        };
        let recycles = self
            .config
            .desktop
            .space(space)
            .is_some_and(|space| recycles_on_delete(space, &self.storage, &path));
        if !ui.confirm("Delete item", &delete_item_question(&label, recycles)) {
            return;
        }
        if !self.remove_space_item_at(space, index, recycles, |file| {
            ui.delete_files(std::slice::from_ref(&file.to_path_buf()), false);
        }) {
            return;
        }
        self.refresh_space(ui, space);
        self.refresh_tray(ui);
        self.save(ui);
        self.refresh_settings(ui);
    }

    pub(in crate::app) fn remove_space_item_at(
        &mut self,
        space: SpaceId,
        index: usize,
        recycles: bool,
        delete_file: impl FnOnce(&Path),
    ) -> bool {
        let Some((item, path)) = self
            .config
            .desktop
            .space(space)
            .and_then(|space| space.items().get(index))
            .map(|item| (item.id, item.path.clone()))
        else {
            return false;
        };
        self.undo.begin(delete_items_label(1));
        if recycles && path.exists() {
            let at = SystemTime::now();
            delete_file(&path);
            if path.exists() {
                tracing::warn!(path = %path.display(), "the item's file could not be deleted; it stays");
                self.undo.cancel();
                return false;
            }
            self.undo
                .note_file(FileChange::Recycled { original: path, at });
        }
        self.forget_item(item);
        let near = self.space_center(space);
        // Not `commit`: the file already moved, so the caller saves once.
        self.dispatch(Command::RemoveItem { id: item }, near);
        self.undo.finish(&self.config.desktop);
        true
    }

    /// Forgets an item's transient per-item state (the selection, cut
    /// ghosting, any independent check, and the hover) after it is removed or
    /// its file is gone.
    pub(in crate::app) fn forget_item(&mut self, item: ItemId) {
        self.selection.forget(item);
        self.pointer.clear_hover_for_item(item);
    }

    /// Drops items whose files are gone in every space, after a drag that
    /// may have moved files out of any of them. Startup's check runs on the
    /// icon worker instead (`app/facts.rs`).
    pub(in crate::app) fn prune_all_missing(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.id)
            .collect();
        for id in ids {
            self.prune_missing(ui, id);
        }
    }

    /// Removes items whose files are gone, for example after a shell command
    /// deleted one or a drag moved it elsewhere.
    pub(in crate::app) fn prune_missing(&mut self, ui: &mut Ui, id: SpaceId) {
        let items: Vec<ItemId> = self
            .config
            .desktop
            .space(id)
            .map(|space| space.items().iter().map(|item| item.id).collect())
            .unwrap_or_default();
        if self.remove_gone(id, &items) == 0 {
            return;
        }
        self.refresh_space(ui, id);
        self.refresh_tray(ui);
        self.save(ui);
    }

    /// Removes those of `items` in space `id` whose files are gone, and
    /// returns how many went. The one place an item is dropped for its file:
    /// each file is read here, at the path the item has now, so an item
    /// renamed since someone found its file missing is kept, and so is one
    /// whose drive or share does not answer (`file_is_gone`).
    pub(in crate::app) fn remove_gone(&mut self, id: SpaceId, items: &[ItemId]) -> usize {
        let gone: Vec<ItemId> = self
            .config
            .desktop
            .space(id)
            .map(|space| {
                items
                    .iter()
                    .filter_map(|item| space.item(*item))
                    .filter(|item| file_is_gone(&item.path))
                    .map(|item| item.id)
                    .collect()
            })
            .unwrap_or_default();
        let near = self.space_center(id);
        for item in &gone {
            // Not `commit`: one of several commands, which the caller
            // repaints and saves once.
            self.dispatch(Command::RemoveItem { id: *item }, near);
            self.forget_item(*item);
        }
        gone.len()
    }

    pub(in crate::app) fn delete_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let stored = space
            .items()
            .iter()
            .filter(|item| self.storage.contains(&item.path))
            .count();
        let stored_files = if !delete_needs_confirmation(space) {
            StoredFiles::ToDesktop
        } else {
            let question = format!(
                "Delete the space \"{}\"?\n\n{}",
                space.title(),
                delete_consequence(space, stored)
            );
            if stored > 0 && !space.is_lens() {
                let buttons = STORED_FILES_CHOICES.map(|(label, _)| label);
                match stored_files_for(ui.choose("Delete space", &question, &buttons, 0)) {
                    Some(stored_files) => stored_files,
                    None => return,
                }
            } else if ui.confirm("Delete space", &question) {
                StoredFiles::ToDesktop
            } else {
                return;
            }
        };
        self.delete_space_now(ui, id, stored_files);
    }

    /// Deletes space `id` without asking, sending its stored files where
    /// `stored` says. If any cannot be moved or recycled, the space stays, so
    /// its files stay reachable.
    pub(in crate::app) fn delete_space_now(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        stored: StoredFiles,
    ) {
        if self.config.desktop.space(id).is_none() {
            return;
        }
        let deleted = self.delete_space_with(id, stored, |paths| {
            ui.delete_files(paths, false);
        });
        if !deleted {
            self.refresh_space(ui, id);
            self.save(ui);
            return;
        }
        self.sync_watches(ui);
        self.sync_windows(ui);
        self.save(ui);
    }

    /// Deletes space `id` as one "Delete space" undo entry, and returns
    /// whether it went. With `StoredFiles::ToRecycleBin`, `recycle` first
    /// sends the stored files to the Recycle Bin, and the entry notes each
    /// one, so undo restores them with the space; a file left behind keeps
    /// the space and records nothing. The debug window suppresses undo, so
    /// its deletes record nothing either way.
    pub(in crate::app) fn delete_space_with(
        &mut self,
        id: SpaceId,
        stored: StoredFiles,
        recycle: impl FnOnce(&[PathBuf]),
    ) -> bool {
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        let paths = match stored {
            StoredFiles::ToDesktop => Vec::new(),
            StoredFiles::ToRecycleBin => stored_paths(space, &self.storage),
        };
        let title = space.title().to_string();
        self.undo.begin("Delete space");
        if !paths.is_empty() {
            let at = SystemTime::now();
            recycle(&paths);
            let kept = paths.iter().filter(|path| path.exists()).count();
            if kept > 0 {
                tracing::error!(
                    space = title,
                    kept,
                    "space kept because its files could not be recycled"
                );
                self.undo.cancel();
                return false;
            }
            for original in paths {
                self.undo.note_file(FileChange::Recycled { original, at });
            }
        }
        // Any stored files still here go back to the desktop before the
        // space goes; recycled ones are gone, so `release_item` only unlists
        // them.
        if !self.release_and_delete_space(id) {
            self.undo.cancel();
            return false;
        }
        self.undo.finish(&self.config.desktop);
        true
    }

    pub(in crate::app) fn release_and_delete_space(&mut self, id: SpaceId) -> bool {
        self.release_and_delete_space_to(id, None)
    }

    pub(in crate::app) fn release_and_delete_space_to(
        &mut self,
        id: SpaceId,
        desktop_override: Option<&Path>,
    ) -> bool {
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        let near = self.space_center(id);
        let items: Vec<ItemId> = space.items().iter().map(|item| item.id).collect();
        // Note the space before `release_item` removes its items, so `before`
        // holds the space with all of its items.
        self.undo.note_space(&self.config.desktop, id);
        let kept = items
            .into_iter()
            .filter(|item| !self.release_item(*item, desktop_override))
            .count();
        if kept > 0 {
            tracing::error!(
                kept,
                "space kept because some files could not be moved back"
            );
            return false;
        }
        // Not `commit`: a deleted space closes its window and saves at once.
        if !self
            .dispatch(Command::DeleteSpace { id }, near)
            .is_applied()
        {
            return false;
        }
        if self.selection.surface == Some(Surface::Space(id)) {
            self.selection.clear();
        }
        self.pointer.clear_hover_on(Surface::Space(id));
        self.view_cache.forget(id);
        self.icons.forget_user(IconUser::Space(id.0));
        // A deleted lens's folder stops being watched; its files stay.
        self.unavailable_lenses.remove(&id);
        true
    }
}

/// The stored files of `space` that still exist: what Delete all spaces
/// sends to the Recycle Bin. A lens has none: its files are its folder's.
pub(in crate::app) fn stored_paths(space: &Space, storage: &ItemStore) -> Vec<PathBuf> {
    if space.is_lens() {
        return Vec::new();
    }
    space
        .items()
        .iter()
        .map(|item| item.path.clone())
        .filter(|path| storage.contains(path) && path.exists())
        .collect()
}

/// Whether deleting the item at `path` from `space` recycles its file: a lens
/// item (its folder is the real thing, so the next sync would bring it back)
/// and a stored file do; an item listed in place is only unlisted.
fn recycles_on_delete(space: &Space, store: &ItemStore, path: &Path) -> bool {
    space.is_lens() || store.contains(path)
}

/// The confirmation for deleting one item, saying what happens to its file:
/// a file that `recycles` goes to the Recycle Bin, an item shown in place
/// keeps its file where it is.
fn delete_item_question(label: &str, recycles: bool) -> String {
    let consequence = if recycles {
        "It will be moved to the Recycle Bin."
    } else {
        "It will be removed from this space. The file itself stays where it is."
    };
    format!("Delete \u{201C}{label}\u{201D}?\n\n{consequence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_delete_space_answer_sends_the_stored_files_where_it_says() {
        assert_eq!(
            STORED_FILES_CHOICES[0].1,
            Some(StoredFiles::ToDesktop),
            "the default, so Enter keeps them on the desktop"
        );
        assert_eq!(stored_files_for(Some(0)), Some(StoredFiles::ToDesktop));
        assert_eq!(stored_files_for(Some(1)), Some(StoredFiles::ToRecycleBin));
        assert_eq!(
            stored_files_for(Some(2)),
            None,
            "Cancel, and Esc's IDCANCEL"
        );
        assert_eq!(stored_files_for(Some(3)), None);
        assert_eq!(stored_files_for(None), None, "a closed dialog");
    }

    #[test]
    fn deleting_an_item_says_where_its_file_goes() {
        let stored = delete_item_question("Notes", true);
        assert!(stored.starts_with("Delete \u{201C}Notes\u{201D}?"));
        assert!(stored.contains("Recycle Bin"));
        let in_place = delete_item_question("Notes", false);
        assert!(in_place.contains("stays where it is"));
        assert!(!in_place.contains("Recycle Bin"));
    }

    #[test]
    fn a_lens_item_and_a_stored_file_are_recycled_but_an_item_in_place_is_not() {
        let root = std::env::temp_dir().join("deskspace-recycles-test-storage");
        let store = ItemStore::new(&root);
        let mut desktop = deskspace_core::Desktop::default();
        let bounds = deskspace_core::Rect::new(0, 0, 300, 200);
        let normal = crate::fixtures::create_space(&mut desktop, "S", bounds);
        let lens = crate::fixtures::create_lens(&mut desktop, "L", bounds, "C:/elsewhere");
        let stored = root.join("a.txt");
        let in_place = Path::new("C:/elsewhere/b.txt");
        let normal = desktop.space(normal).unwrap();
        let lens = desktop.space(lens).unwrap();
        assert!(recycles_on_delete(normal, &store, &stored));
        assert!(!recycles_on_delete(normal, &store, in_place));
        assert!(recycles_on_delete(lens, &store, in_place));
    }

    #[test]
    fn a_lens_on_the_store_has_no_stored_paths() {
        let root = std::env::temp_dir().join(format!(
            "deskspace-lens-stored-paths-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let mut desktop = deskspace_core::Desktop::default();
        let lens = crate::fixtures::create_lens(
            &mut desktop,
            "L",
            deskspace_core::Rect::new(0, 0, 300, 200),
            &root.to_string_lossy(),
        );
        crate::fixtures::sync_lens(
            &mut desktop,
            lens,
            vec![deskspace_core::source::FolderEntry {
                path: file,
                label: String::new(),
                kind: deskspace_core::item::ItemKind::Other,
                size: None,
                modified: None,
                created: None,
                hidden: false,
            }],
        );
        let space = desktop.space(lens).unwrap();
        assert_eq!(space.items().len(), 1);
        let paths = stored_paths(space, &ItemStore::new(&root));
        let _ = std::fs::remove_dir_all(&root);
        assert!(paths.is_empty());
    }

    #[test]
    fn stored_paths_lists_stored_files_and_skips_items_in_place() {
        let root = std::env::temp_dir().join(format!(
            "deskspace-stored-paths-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let stored = root.join("dropped.txt");
        std::fs::write(&stored, "x").unwrap();
        let in_place = std::env::current_exe().unwrap();
        let mut desktop = deskspace_core::Desktop::default();
        let space = crate::fixtures::create_space(
            &mut desktop,
            "S",
            deskspace_core::Rect::new(0, 0, 300, 200),
        );
        crate::fixtures::add_item(&mut desktop, space, &stored.to_string_lossy());
        crate::fixtures::add_item(&mut desktop, space, &in_place.to_string_lossy());
        crate::fixtures::add_item(
            &mut desktop,
            space,
            &root.join("gone.txt").to_string_lossy(),
        );
        let paths = stored_paths(desktop.space(space).unwrap(), &ItemStore::new(&root));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(paths, vec![stored]);
    }
}
