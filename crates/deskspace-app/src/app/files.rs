//! `App` files: drops, storage, deletion, and desktop reconciliation (R1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, Space, SpaceId};
use deskspace_platform::{
    CustomShortcut, DropAction, ShellItem, Ui, create_custom_shortcut, create_shortcut,
    desktop_directory, item_facts, notify_moved,
};

use super::undo::move_items_label;
use super::{App, Surface, delete_consequence, delete_needs_confirmation};
use crate::storage::ItemStore;

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
        self.undo.begin("New space");
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
        self.undo.finish(&self.config.desktop);
    }

    pub(super) fn add_dropped(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        paths: Vec<PathBuf>,
        action: DropAction,
    ) {
        self.undo.begin(move_items_label(paths.len()));
        let mut touched = HashSet::from([target]);
        let mut moved_items = 0usize;
        let mut touched_files = false;
        for path in paths {
            // A drop can name a file that no longer exists (for example a
            // stale desktop icon). Listing it would show a label without an
            // icon, so it is skipped.
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
                    moved_items += 1;
                    Command::MoveItem { id: item, target }
                }
                None => {
                    let placed = match action {
                        DropAction::KeepInPlace => Some(path),
                        DropAction::Move => {
                            touched_files = true;
                            self.place_dropped(path)
                        }
                        DropAction::Copy => {
                            touched_files = true;
                            self.copy_dropped(ui, path)
                        }
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
            self.dispatch(command, at);
        }
        if touched_files {
            // File changes are reversed in U5-U7; until then an entry that
            // holds any is not recorded.
            self.undo.cancel();
        } else if moved_items > 0 {
            self.undo.set_label(move_items_label(moved_items));
        }
        self.undo.finish(&self.config.desktop);
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

    /// Creates a shortcut in the store for each source and lists it in
    /// `target`: shell items dropped on a space (a Start-menu or taskbar app,
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
        let root = self.storage.root();
        if let Err(error) = std::fs::create_dir_all(root) {
            tracing::warn!(%error, root = %root.display(), "cannot create the space storage folder");
            return;
        }
        for source in sources {
            match self.write_shortcut(&source) {
                Ok(path) => {
                    tracing::info!(name = source.name(), path = %path.display(), "created a shortcut in the store");
                    // Not `commit`: one of several commands, refreshed once below.
                    self.dispatch(
                        Command::AddItem {
                            space: target,
                            facts: item_facts(&path),
                            path,
                        },
                        at,
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, name = source.name(), "could not create a shortcut");
                }
            }
        }
        self.refresh_space(ui, target);
        self.refresh_tray(ui);
        self.save(ui);
        self.refresh_settings(ui);
    }

    /// Writes one shortcut into the store under a clash-free name and returns
    /// its path: a copy of a dropped shortcut file, a new `.lnk` to a shell
    /// item, or the `.lnk` or `.url` a custom shortcut describes.
    fn write_shortcut(&self, source: &ShortcutSource) -> anyhow::Result<PathBuf> {
        match source {
            ShortcutSource::Shell(item) => {
                if let Some(link) = item.path.as_deref().filter(|path| is_link_file(path)) {
                    let path = self.storage.unique_path(&link_copy_name(link, &item.name));
                    std::fs::copy(link, &path)?;
                    return Ok(path);
                }
                let path = self
                    .storage
                    .unique_path(&shortcut_file_name(&item.name, "lnk"));
                create_shortcut(item, &path)?;
                Ok(path)
            }
            ShortcutSource::Custom(shortcut) => {
                let name = shortcut_file_name(&shortcut.name, shortcut.target.extension());
                let path = self.storage.unique_path(&name);
                create_custom_shortcut(shortcut, &path)?;
                Ok(path)
            }
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
    fn place_dropped(&self, path: PathBuf) -> Option<PathBuf> {
        match self.storage.adopt(&path) {
            Ok(stored) => {
                if stored != path {
                    tracing::info!(from = %path.display(), to = %stored.display(), "moved into space storage");
                    notify_moved(&path, &stored);
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
        let created = ui.transfer_files(std::slice::from_ref(&path), root, false)?;
        created.into_iter().next()
    }

    /// Takes an item out of its space when the space is deleted. A file that
    /// was moved into the space storage goes back to the desktop first, so
    /// nothing is left behind in a folder the user never sees. Returns false
    /// if that move failed; the item then stays.
    pub(super) fn release_item(&mut self, id: ItemId) -> bool {
        let Some(path) = self.config.desktop.item(id).map(|item| item.path.clone()) else {
            return false;
        };
        if self.storage.contains(&path) && path.exists() {
            let desktop = match desktop_directory() {
                Ok(desktop) => desktop,
                Err(error) => {
                    tracing::error!(%error, "cannot find the desktop folder");
                    return false;
                }
            };
            match self.storage.release(&path, &desktop) {
                Ok(back) => {
                    tracing::info!(to = %back.display(), "moved back to the desktop");
                    notify_moved(&path, &back);
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
        let paths: Vec<PathBuf> = items
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .filter(|item| item.path.exists())
            .map(|item| item.path.clone())
            .collect();
        if !paths.is_empty() {
            ui.delete_files(&paths, permanent);
        }
        match surface {
            Surface::Space(space) => self.drop_deleted_items(ui, space, items),
            // The layer mirrors the Desktop folder, so re-reading it drops
            // whatever the shell removed.
            Surface::Desktop => self.resync_desktop_layer(ui),
        }
    }

    /// Drops every one of `items` in `space` whose file is gone, whether the
    /// shell removed it or it was already stale, so no label without an icon
    /// is left behind.
    fn drop_deleted_items(&mut self, ui: &mut Ui, space: SpaceId, items: Vec<ItemId>) {
        let near = self.space_center(space);
        for item in items {
            let gone = self
                .config
                .desktop
                .item(item)
                .is_none_or(|item| !item.path.exists());
            if gone {
                self.forget_item(item);
                // Not `commit`: the file already moved, so this saves at once below.
                self.dispatch(Command::RemoveItem { id: item }, near);
            }
        }
        self.refresh_space(ui, space);
        self.refresh_tray(ui);
        self.save(ui);
    }

    /// Deletes the `index`-th item of space `space` after asking, from the X
    /// on the settings window's Shortcuts page (ROADMAP D3): a file in the
    /// space storage goes to the Recycle Bin, and an item listed in place is
    /// only removed from the space, so the user's original is never deleted.
    /// If the shell cannot delete the file (for example it is in use), the
    /// item stays.
    pub(super) fn delete_space_item(&mut self, ui: &mut Ui, space: SpaceId, index: usize) {
        let Some((item, path, label)) = self
            .config
            .desktop
            .space(space)
            .and_then(|space| space.items().get(index))
            .map(|item| (item.id, item.path.clone(), item.label.clone()))
        else {
            return;
        };
        let stored = self.storage.contains(&path);
        if !ui.confirm("Delete item", &delete_item_question(&label, stored)) {
            return;
        }
        if stored && path.exists() {
            ui.delete_files(std::slice::from_ref(&path), false);
            if path.exists() {
                tracing::warn!(path = %path.display(), "the item's file could not be deleted; it stays");
                return;
            }
        }
        self.forget_item(item);
        let near = self.space_center(space);
        // Not `commit`: the file already moved, so this saves at once below.
        self.dispatch(Command::RemoveItem { id: item }, near);
        self.refresh_space(ui, space);
        self.refresh_tray(ui);
        self.save(ui);
        self.refresh_settings(ui);
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
                delete_consequence(space.items().len(), stored)
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
        let touches_files =
            stored == StoredFiles::ToRecycleBin || !stored_paths(space, &self.storage).is_empty();
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
        if touches_files {
            // File changes are reversed in U5-U7; until then an entry that
            // holds any is not recorded.
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
        self.sync_windows(ui);
        self.save(ui);
    }

    pub(super) fn release_and_delete_space(&mut self, id: SpaceId) -> bool {
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
            .filter(|item| !self.release_item(*item))
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
        true
    }
}

/// The stored files of `space` that still exist: what Delete all spaces
/// sends to the Recycle Bin.
pub(super) fn stored_paths(space: &Space, storage: &ItemStore) -> Vec<PathBuf> {
    space
        .items()
        .iter()
        .map(|item| item.path.clone())
        .filter(|path| storage.contains(path) && path.exists())
        .collect()
}

/// The confirmation for deleting one item, saying what happens to its file:
/// a stored file goes to the Recycle Bin, an item shown in place keeps its
/// file where it is.
fn delete_item_question(label: &str, stored: bool) -> String {
    let consequence = if stored {
        "It will be moved to the Recycle Bin."
    } else {
        "It will be removed from this space. The file itself stays where it is."
    };
    format!("Delete \u{201C}{label}\u{201D}?\n\n{consequence}")
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
