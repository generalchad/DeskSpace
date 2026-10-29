//! `App` files: drops, storage, deletion, and desktop reconciliation (R1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, Space, SpaceId};
use deskspace_platform::{
    CustomShortcut, DropAction, OnCollision, ShellItem, Ui, create_custom_shortcut,
    create_shortcut, desktop_directory, item_facts, notify_moved,
};

use super::undo::{
    FileChange, copy_items_label, delete_items_label, move_items_label, new_shortcuts_label,
};
use super::{App, Surface, delete_consequence, delete_needs_confirmation};
use crate::storage::{ItemStore, is_inside, path_key, same_path, unique_destination};

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
pub(super) enum StoredFiles {
    /// Back to the desktop, as Delete space does (ROADMAP D3).
    ToDesktop,
    /// To the Recycle Bin, as the debug window's Delete all spaces does
    /// (ROADMAP D14), so test files never pile up on the desktop.
    ToRecycleBin,
}

impl App {
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

    /// The folder space `id` shows, or `None` for a normal space. Drops and
    /// pastes ask it first: a lens takes its files by the shell, not as items.
    pub(super) fn lens_folder(&self, id: SpaceId) -> Option<PathBuf> {
        let space = self.config.desktop.space(id)?;
        space.folder_view().map(|view| view.path.clone())
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
            // never offers it over a lens (W3c); the arm keeps the match total.
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

    /// Creates a shortcut in the store for each source and lists it in
    /// `target`, or, for a lens, writes it into the lens's folder, where its
    /// watch lists it: shell items dropped on a space (a Start-menu or taskbar app,
    /// or an Alt-drag; F-50), apps picked from the Applications folder, and
    /// custom shortcuts (F-53). Explorer writes a `.lnk` when a shell item is
    /// dropped on a folder, and the store is the space's folder, so the
    /// shortcut moves, opens, and deletes like any other item. A dropped
    /// shortcut file is copied instead, as Explorer does, so no shortcut
    /// points at a shortcut; the original is never moved or deleted. Every
    /// way of adding a shortcut ends here.
    pub(super) fn add_shortcuts(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        sources: Vec<ShortcutSource>,
    ) {
        if !self.create_shortcuts(target, at, sources) {
            return;
        }
        self.refresh_space(ui, target);
        self.refresh_tray(ui);
        self.save(ui);
        self.refresh_settings(ui);
    }

    pub(super) fn create_shortcuts(
        &mut self,
        target: SpaceId,
        at: Point,
        sources: Vec<ShortcutSource>,
    ) -> bool {
        let lens = self.lens_folder(target);
        let destination = match &lens {
            Some(folder) if !folder.is_dir() || !self.storage.allows_lens(folder) => {
                tracing::warn!(folder = %folder.display(), "lens folder is unavailable; no shortcut written");
                return false;
            }
            Some(folder) => folder.clone(),
            None => {
                let root = self.storage.root();
                if let Err(error) = std::fs::create_dir_all(root) {
                    tracing::warn!(%error, root = %root.display(), "cannot create the space storage folder");
                    return false;
                }
                root.to_path_buf()
            }
        };
        self.undo.begin(new_shortcuts_label(sources.len()));
        let mut count = 0usize;
        for source in sources {
            match write_shortcut(&destination, &source) {
                Ok(path) => {
                    tracing::info!(name = source.name(), path = %path.display(), "created a shortcut");
                    self.undo
                        .note_file(FileChange::Created { path: path.clone() });
                    if lens.is_some() {
                        continue;
                    }
                    // Not `commit`: one of several commands, refreshed once below.
                    if self
                        .dispatch(
                            Command::AddItem {
                                space: target,
                                facts: item_facts(&path),
                                path,
                            },
                            at,
                        )
                        .is_applied()
                    {
                        count += 1;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, name = source.name(), "could not create a shortcut");
                }
            }
        }
        if count > 0 {
            self.undo.set_label(new_shortcuts_label(count));
        }
        self.undo.finish(&self.config.desktop);
        true
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
    /// leaving the original where it is, as a Copy drop does (G5b) and a paste
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

    /// Deletes every selected item's file with the shell, then drops the items
    /// whose files are gone. Unless `permanent` is set they go to the Recycle
    /// Bin, as Explorer's Delete key does; `permanent` is Shift+Delete. A file
    /// the shell could not delete (for example one in use) stays listed. An item
    /// whose file is already missing is dropped without a shell operation. This
    /// is distinct from deleting a space, which returns its stored files to
    /// the desktop.
    pub(super) fn delete_selected_items(&mut self, ui: &mut Ui, permanent: bool) {
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
    pub(super) fn remove_space_items(
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
    pub(super) fn delete_space_item(&mut self, ui: &mut Ui, space: SpaceId, index: usize) {
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

    pub(super) fn remove_space_item_at(
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
    pub(super) fn forget_item(&mut self, item: ItemId) {
        self.selection.forget(item);
        self.pointer.clear_hover_for_item(item);
    }

    /// Drops items whose files no longer exist in every space. Called once at
    /// startup so leftovers from a move made while DeskSpace was closed go away.
    pub(super) fn prune_all_missing(&mut self, ui: &mut Ui) {
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

    /// Removes items whose files no longer exist, for example after a shell
    /// command deleted one or a drag moved it elsewhere.
    pub(super) fn prune_missing(&mut self, ui: &mut Ui, id: SpaceId) {
        let missing: Vec<ItemId> = self
            .config
            .desktop
            .space(id)
            .map(|space| {
                space
                    .items()
                    .iter()
                    .filter(|item| !item.path.exists())
                    .map(|item| item.id)
                    .collect()
            })
            .unwrap_or_default();
        if missing.is_empty() {
            return;
        }
        for item in missing {
            let near = self.space_center(id);
            // Not `commit`: one of several commands, refreshed once below.
            self.dispatch(Command::RemoveItem { id: item }, near);
            self.forget_item(item);
        }
        self.refresh_space(ui, id);
        self.refresh_tray(ui);
        self.save(ui);
    }

    pub(super) fn delete_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let stored = space
            .items()
            .iter()
            .filter(|item| self.storage.contains(&item.path))
            .count();
        if delete_needs_confirmation(space) {
            let question = format!(
                "Delete the space \"{}\"?\n\n{}",
                space.title(),
                delete_consequence(space, stored)
            );
            if !ui.confirm("Delete space", &question) {
                return;
            }
        }
        self.delete_space_now(ui, id, StoredFiles::ToDesktop);
    }

    /// Deletes space `id` without asking, sending its stored files where
    /// `stored` says. If any cannot be moved, the space stays, so its files
    /// stay reachable.
    pub(super) fn delete_space_now(&mut self, ui: &mut Ui, id: SpaceId, stored: StoredFiles) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if stored == StoredFiles::ToRecycleBin {
            let paths = stored_paths(space, &self.storage);
            let recycled = paths.is_empty()
                || (ui.delete_files(&paths, false) && !paths.iter().any(|path| path.exists()));
            if !recycled {
                tracing::error!(
                    space = space.title(),
                    "space kept because its files could not be recycled"
                );
                self.refresh_space(ui, id);
                self.save(ui);
                return;
            }
        }
        self.undo.begin("Delete space");
        if stored == StoredFiles::ToRecycleBin {
            self.undo.cancel();
        }
        // Any stored files still here go back to the desktop before the
        // space goes; recycled ones are gone, so `release_item` only unlists
        // them.
        if !self.release_and_delete_space(id) {
            self.undo.cancel();
            self.refresh_space(ui, id);
            self.save(ui);
            return;
        }
        self.undo.finish(&self.config.desktop);
        self.sync_watches(ui);
        self.sync_windows(ui);
        self.save(ui);
    }

    pub(super) fn release_and_delete_space(&mut self, id: SpaceId) -> bool {
        self.release_and_delete_space_to(id, None)
    }

    pub(super) fn release_and_delete_space_to(
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
        // A deleted lens's folder stops being watched; its files stay.
        self.unavailable_lenses.remove(&id);
        true
    }
}

/// The stored files of `space` that still exist: what Delete all spaces
/// sends to the Recycle Bin. A lens has none: its files are its folder's.
pub(super) fn stored_paths(space: &Space, storage: &ItemStore) -> Vec<PathBuf> {
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

/// Writes one shortcut into `folder` under a clash-free name and returns its
/// path: a copy of a dropped shortcut file, a new `.lnk` to a shell item, or
/// the `.lnk` or `.url` a custom shortcut describes.
fn write_shortcut(folder: &Path, source: &ShortcutSource) -> anyhow::Result<PathBuf> {
    match source {
        ShortcutSource::Shell(item) => {
            if let Some(link) = item.path.as_deref().filter(|path| is_link_file(path)) {
                let path = unique_destination(folder, Path::new(&link_copy_name(link, &item.name)));
                std::fs::copy(link, &path)?;
                return Ok(path);
            }
            let name = shortcut_file_name(&item.name, "lnk");
            let path = unique_destination(folder, Path::new(&name));
            create_shortcut(item, &path)?;
            Ok(path)
        }
        ShortcutSource::Custom(shortcut) => {
            let name = shortcut_file_name(&shortcut.name, shortcut.target.extension());
            let path = unique_destination(folder, Path::new(&name));
            create_custom_shortcut(shortcut, &path)?;
            Ok(path)
        }
    }
}

/// Whether `path` is a shortcut file (`.lnk` or `.url`, in any case), which a
/// drop copies rather than links to.
fn is_link_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("url")
        })
}

/// The store name for a copied shortcut file: its own file name, which keeps
/// the `.lnk` or `.url` extension, or a name built from `display` when the
/// path has none.
fn link_copy_name(link: &Path, display: &str) -> String {
    link.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| shortcut_file_name(display, "lnk"))
}

/// A shortcut file name with `extension` (`lnk` or `url`) for a display name
/// (F-50, F-53). Characters that are illegal in a Windows file name become
/// `_`, trailing dots and spaces are trimmed (Windows ignores them), and an
/// empty result falls back to "Shortcut", as Explorer does.
fn shortcut_file_name(display: &str, extension: &str) -> String {
    let mut name: String = display
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    while name.ends_with([' ', '.']) {
        name.pop();
    }
    if name.is_empty() {
        name.push_str("Shortcut");
    }
    name.push('.');
    name.push_str(extension);
    name
}

/// Something a shortcut is made from; see `App::add_shortcuts`.
#[derive(Debug, Clone)]
pub(super) enum ShortcutSource {
    /// A dropped or picked shell item.
    Shell(ShellItem),
    /// A shortcut described in the "Custom shortcut" dialog.
    Custom(CustomShortcut),
}

impl ShortcutSource {
    /// The name the shortcut is listed under, for logging.
    fn name(&self) -> &str {
        match self {
            Self::Shell(item) => &item.name,
            Self::Custom(shortcut) => &shortcut.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_dropped_shortcut_is_copied_into_the_folder_under_a_free_name() {
        let root = std::env::temp_dir().join(format!(
            "deskspace-write-shortcut-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let (from, to) = (root.join("from"), root.join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let link = from.join("App.lnk");
        std::fs::write(&link, "x").unwrap();
        let source = ShortcutSource::Shell(ShellItem {
            name: "App".to_owned(),
            path: Some(link),
            id_list: Vec::new(),
        });
        let first = write_shortcut(&to, &source).unwrap();
        let second = write_shortcut(&to, &source).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(first, to.join("App.lnk"));
        assert_eq!(second, to.join("App (2).lnk"));
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

    #[test]
    fn shortcut_names_make_illegal_characters_safe() {
        assert_eq!(shortcut_file_name("Minesweeper", "lnk"), "Minesweeper.lnk");
        assert_eq!(shortcut_file_name("A/B:C?D", "lnk"), "A_B_C_D.lnk");
    }

    #[test]
    fn shortcut_names_trim_trailing_dots_and_fall_back_when_empty() {
        assert_eq!(shortcut_file_name("Report...", "lnk"), "Report.lnk");
        assert_eq!(shortcut_file_name("   ", "lnk"), "Shortcut.lnk");
        assert_eq!(shortcut_file_name("", "lnk"), "Shortcut.lnk");
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
        crate::fixtures::apply(
            &mut desktop,
            Command::SyncLens {
                id: lens,
                entries: vec![deskspace_core::source::FolderEntry {
                    path: file,
                    label: String::new(),
                    kind: deskspace_core::item::ItemKind::Other,
                    size: None,
                    modified: None,
                    hidden: false,
                }],
            },
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

    #[test]
    fn a_web_shortcut_name_takes_the_url_extension() {
        assert_eq!(shortcut_file_name("example.com", "url"), "example.com.url");
    }

    #[test]
    fn shortcut_files_are_recognized_in_any_case() {
        assert!(is_link_file(Path::new(r"C:\Start\Notepad.lnk")));
        assert!(is_link_file(Path::new(r"C:\Links\Docs.URL")));
        assert!(!is_link_file(Path::new(r"C:\Desktop\notes.txt")));
        assert!(!is_link_file(Path::new(r"C:\Desktop\lnk")));
    }

    #[test]
    fn a_copied_shortcut_keeps_its_file_name() {
        assert_eq!(
            link_copy_name(Path::new(r"C:\Start\Notepad.lnk"), "Notepad"),
            "Notepad.lnk"
        );
        assert_eq!(link_copy_name(Path::new(""), "Docs"), "Docs.lnk");
    }
}
