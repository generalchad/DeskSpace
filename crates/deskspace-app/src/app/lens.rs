//! `App` lens behavior: keeping folder watches in line with the model and
//! turning a watched folder's listing into the lens's items (M4 phase 2).

use std::collections::HashMap;
use std::path::PathBuf;

use deskspace_core::source::FolderEntry;
use deskspace_core::{Command, SpaceId};
use deskspace_platform::{FolderListing, Ui, WatchKey, desktop_directory};

use super::App;

impl App {
    /// Brings the folder watches in line with the model: the Desktop folder
    /// while the layer is drawn, and every lens's folder.
    pub(super) fn sync_watches(&mut self, ui: &mut Ui) {
        let desktop = if self.layer_active() {
            match desktop_directory() {
                Ok(directory) => Some(directory),
                Err(error) => {
                    tracing::warn!(%error, "could not resolve the Desktop folder to watch");
                    None
                }
            }
        } else {
            None
        };
        for id in self.refused_lenses() {
            if self.unavailable_lenses.insert(id) {
                let folder = self
                    .config
                    .desktop
                    .space(id)
                    .and_then(|space| space.folder_view())
                    .map(|view| view.path.display().to_string());
                tracing::warn!(space = %id, ?folder, "lens folder is DeskSpace's storage; not watching it");
            }
        }
        let wanted = self.desired_watches(desktop);
        for key in ui.watched_folders() {
            if !wanted.contains_key(&key) {
                ui.unwatch_folder(key);
            }
        }
        for (key, path) in wanted {
            ui.watch_folder(key, &path);
        }
    }

    /// The watches the model wants. Pure: the caller passes the Desktop folder
    /// it resolved, so the set is tested without the shell.
    fn desired_watches(&self, desktop: Option<PathBuf>) -> HashMap<WatchKey, PathBuf> {
        let mut wanted = HashMap::new();
        if let Some(directory) = desktop.filter(|_| self.layer_active()) {
            wanted.insert(WatchKey::Desktop, directory);
        }
        for space in self.config.desktop.spaces() {
            if let Some(view) = space.folder_view()
                && self.storage.allows_lens(&view.path)
            {
                wanted.insert(WatchKey::Space(space.id), view.path.clone());
            }
        }
        wanted
    }

    /// The lenses on the storage folder or inside it. They stay in the config
    /// but are never watched.
    fn refused_lenses(&self) -> Vec<SpaceId> {
        self.config
            .desktop
            .spaces()
            .iter()
            .filter(|space| {
                space
                    .folder_view()
                    .is_some_and(|view| !self.storage.allows_lens(&view.path))
            })
            .map(|space| space.id)
            .collect()
    }

    /// Applies a listing the watch worker posted for lens `id`.
    pub(super) fn on_lens_listing(&mut self, ui: &mut Ui, id: SpaceId, listing: FolderListing) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if !space.is_lens() {
            return;
        }
        match listing {
            FolderListing::Listed(entries) => {
                // The icon cache keeps an icon until it is told to drop it, so
                // a file rewritten in place would keep its old icon.
                let changed: Vec<PathBuf> = entries
                    .iter()
                    .filter(|entry| {
                        space
                            .items()
                            .iter()
                            .any(|item| item.path == entry.path && item.modified != entry.modified)
                    })
                    .map(|entry| entry.path.clone())
                    .collect();
                for path in changed {
                    self.icons.invalidate(&path);
                }
                self.unavailable_lenses.remove(&id);
                self.sync_lens(ui, id, entries);
            }
            FolderListing::Unavailable => {
                self.unavailable_lenses.insert(id);
                tracing::debug!(
                    space = %id,
                    unavailable = self.unavailable_lenses.len(),
                    "lens folder is unavailable"
                );
                self.sync_lens(ui, id, Vec::new());
            }
        }
    }

    /// Replaces lens `id`'s items from `entries` and repaints it. The items are
    /// not persisted, so this needs no save.
    fn sync_lens(&mut self, ui: &mut Ui, id: SpaceId, entries: Vec<FolderEntry>) {
        let Some(at) = self
            .config
            .desktop
            .space(id)
            .map(|space| space.rect().origin())
        else {
            return;
        };
        if self
            .dispatch(Command::SyncLens { id, entries }, at)
            .is_applied()
        {
            self.refresh_space(ui, id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Rect;

    use crate::app::LayerState;
    use crate::fixtures;

    fn app() -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        App::new(Config::new(), store, false, false)
    }

    #[test]
    fn a_quiet_desktop_wants_no_watch() {
        let app = app();
        assert!(
            app.desired_watches(Some("C:/Users/me/Desktop".into()))
                .is_empty()
        );
    }

    #[test]
    fn the_drawn_layer_wants_the_desktop_folder() {
        let mut app = app();
        app.layer = LayerState::new(true, false);
        let wanted = app.desired_watches(Some("C:/Users/me/Desktop".into()));
        assert_eq!(
            wanted.get(&WatchKey::Desktop),
            Some(&PathBuf::from("C:/Users/me/Desktop"))
        );
    }

    #[test]
    fn every_lens_wants_its_folder_and_normal_spaces_want_none() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            "C:/Users/me/Downloads",
        );
        fixtures::create_space(
            &mut app.config.desktop,
            "Space",
            Rect::new(300, 0, 240, 120),
        );
        let wanted = app.desired_watches(None);
        assert_eq!(
            wanted.get(&WatchKey::Space(lens)),
            Some(&PathBuf::from("C:/Users/me/Downloads"))
        );
        assert_eq!(wanted.len(), 1, "the normal space wants no watch");
    }

    #[test]
    fn a_lens_on_the_storage_folder_is_refused_and_not_watched() {
        let mut app = app();
        let root = app.storage.root().to_string_lossy().into_owned();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            &root,
        );
        assert_eq!(app.refused_lenses(), vec![lens]);
        assert!(app.desired_watches(None).is_empty());
    }
}
