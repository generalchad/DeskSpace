//! `App` files: drops, storage, deletion, and desktop reconciliation (R1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, SpaceId};
use deskspace_platform::{ShellItem, Ui, create_shortcut, desktop_directory, notify_moved};

use super::{App, Surface, delete_consequence, delete_needs_confirmation};

impl App {
    pub(super) fn add_dropped(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        paths: Vec<PathBuf>,
        keep_in_place: bool,
    ) {
        let mut touched = HashSet::from([target]);
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
                    Command::MoveItem { id: item, target }
                }
                None => match self.place_dropped(path, keep_in_place) {
                    Some(path) => Command::AddItem {
                        space: target,
                        path,
                    },
                    None => continue,
                },
            };
            self.dispatch(command, at);
        }
        for id in &touched {
            self.refresh_space(ui, *id);
        }
        // A loose desktop item that was dropped in has moved off the desktop,
        // so reconcile the layer now instead of at the next shell refresh.
        if self.layer_active() {
            self.resync_desktop_layer(ui);
        }
        self.refresh_tray(ui);
        self.save();
    }

    /// Creates a shortcut in the store for each shell item dropped on a space
    /// (a Start-menu or taskbar app, or an Alt-drag; F-50) and lists it in
    /// `target`. Explorer writes a `.lnk` when such an item is dropped on a
    /// folder, and the store is the space's folder, so the shortcut moves,
    /// opens, and releases like any other item. A dropped shortcut file is
    /// copied instead, as Explorer does, so no shortcut points at a shortcut;
    /// the original is never moved or deleted.
    pub(super) fn add_shortcuts(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        items: Vec<ShellItem>,
    ) {
        let root = self.storage.root();
        if let Err(error) = std::fs::create_dir_all(root) {
            tracing::warn!(%error, root = %root.display(), "cannot create the space storage folder");
            return;
        }
        for item in items {
            match self.write_shortcut(&item) {
                Ok(path) => {
                    tracing::info!(name = %item.name, path = %path.display(), "created a shortcut in the store");
                    self.dispatch(
                        Command::AddItem {
                            space: target,
                            path,
                        },
                        at,
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, name = %item.name, "could not create a shortcut");
                }
            }
        }
        self.refresh_space(ui, target);
        self.save();
    }

    /// Writes one dropped item's shortcut into the store under a clash-free
    /// name and returns its path: a copy of a dropped shortcut file, or a new
    /// `.lnk` to anything else.
    fn write_shortcut(&self, item: &ShellItem) -> anyhow::Result<PathBuf> {
        if let Some(link) = item.path.as_deref().filter(|path| is_link_file(path)) {
            let path = self.storage.unique_path(&link_copy_name(link, &item.name));
            std::fs::copy(link, &path)?;
            return Ok(path);
        }
        let path = self.storage.unique_path(&shortcut_file_name(&item.name));
        create_shortcut(item, &path)?;
        Ok(path)
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

    /// Where a newly dropped file will live: in the storage folder, or where
    /// it already is. `None` if it vanished before it could be moved.
    fn place_dropped(&self, path: PathBuf, keep_in_place: bool) -> Option<PathBuf> {
        if keep_in_place {
            return Some(path);
        }
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

    /// Takes an item out of its space. A file that was moved into the space
    /// storage goes back to the desktop first, so nothing is left behind in
    /// a folder the user never sees. Returns false if that move failed; the
    /// item then stays.
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
        self.dispatch(Command::RemoveItem { id }, near)
    }

    /// Deletes every selected item's file with the shell, then drops the items
    /// whose files are gone. Unless `permanent` is set they go to the Recycle
    /// Bin, as Explorer's Delete key does; `permanent` is Shift+Delete. A file
    /// the shell could not delete (for example one in use) stays listed. An item
    /// whose file is already missing is dropped without a shell operation. This
    /// is distinct from the item menu's "Remove from space", which returns a
    /// stored file to the desktop.
    pub(super) fn delete_selected_items(&mut self, ui: &mut Ui, permanent: bool) {
        // Deleting a loose desktop item would need a way to drop it from the
        // layer, so only a space's items are deleted here.
        let Some(Surface::Space(space)) = self.selection.surface else {
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
        // Drop every item whose file is gone, whether the shell removed it or
        // it was already stale, so no label without an icon is left behind.
        let near = self.space_center(space);
        for item in items {
            let gone = self
                .config
                .desktop
                .item(item)
                .is_none_or(|item| !item.path.exists());
            if gone {
                self.forget_item(item);
                self.dispatch(Command::RemoveItem { id: item }, near);
            }
        }
        self.refresh_space(ui, space);
        self.refresh_tray(ui);
        self.save();
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
            .spaces
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
                    .items
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
            self.dispatch(Command::RemoveItem { id: item }, near);
            self.forget_item(item);
        }
        self.refresh_space(ui, id);
        self.refresh_tray(ui);
        self.save();
    }

    pub(super) fn delete_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let stored = space
            .items
            .iter()
            .filter(|item| self.storage.contains(&item.path))
            .count();
        if delete_needs_confirmation(space) {
            let question = format!(
                "Delete the space \"{}\"?\n\n{}",
                space.title,
                delete_consequence(space.items.len(), stored)
            );
            if !ui.confirm("Delete space", &question) {
                return;
            }
        }
        // Return stored files to the desktop before the space goes; if any
        // cannot be moved, keep the space so its files stay reachable.
        let items: Vec<ItemId> = space.items.iter().map(|item| item.id).collect();
        let kept = items
            .into_iter()
            .filter(|item| !self.release_item(*item))
            .count();
        if kept > 0 {
            tracing::error!(
                kept,
                "space kept because some files could not be moved back"
            );
            self.refresh_space(ui, id);
            self.save();
            return;
        }
        let near = self.space_center(id);
        if self.dispatch(Command::DeleteSpace { id }, near) {
            if self.selection.surface == Some(Surface::Space(id)) {
                self.selection.clear();
            }
            self.pointer.clear_hover_on(Surface::Space(id));
            self.scroll.remove(&id);
            self.sync_windows(ui);
            self.save();
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
        .unwrap_or_else(|| shortcut_file_name(display))
}

/// A `.lnk` file name for a shell item's display name (F-50). Characters that
/// are illegal in a Windows file name become `_`, trailing dots and spaces are
/// trimmed (Windows ignores them), and an empty result falls back to
/// "Shortcut", as Explorer does.
fn shortcut_file_name(display: &str) -> String {
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
    name.push_str(".lnk");
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_names_make_illegal_characters_safe() {
        assert_eq!(shortcut_file_name("Minesweeper"), "Minesweeper.lnk");
        assert_eq!(shortcut_file_name("A/B:C?D"), "A_B_C_D.lnk");
    }

    #[test]
    fn shortcut_names_trim_trailing_dots_and_fall_back_when_empty() {
        assert_eq!(shortcut_file_name("Report..."), "Report.lnk");
        assert_eq!(shortcut_file_name("   "), "Shortcut.lnk");
        assert_eq!(shortcut_file_name(""), "Shortcut.lnk");
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
