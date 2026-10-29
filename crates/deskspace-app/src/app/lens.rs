//! `App` lens behavior: keeping folder watches in line with the model,
//! turning a watched folder's listing into the lens's items (M4 phase 2), and
//! browsing into subfolders (W42).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deskspace_core::source::FolderEntry;
use deskspace_core::{Command, Item, ItemId, ItemKind, SpaceId};
use deskspace_platform::{FolderListing, Ui, WatchKey, desktop_directory, is_inside};

use super::App;
use crate::scene::subfolder_path;

/// The subfolder a lens shows below its own folder, and the lens folder it
/// was browsed under. Once the lens's folder changes, `root` no longer
/// matches, so the browse is ignored and then dropped (D18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Browsed {
    root: PathBuf,
    sub: PathBuf,
}

impl App {
    /// The subfolder lens `id` shows, relative to its own folder: empty at
    /// its own folder, and for a normal space.
    pub(super) fn subfolder_of(&self, id: SpaceId) -> PathBuf {
        let Some(view) = self
            .config
            .desktop
            .space(id)
            .and_then(|space| space.folder_view())
        else {
            return PathBuf::new();
        };
        self.browsed
            .get(&id)
            .filter(|browsed| browsed.root == view.path)
            .map(|browsed| browsed.sub.clone())
            .unwrap_or_default()
    }

    /// The folder lens `id` shows, or `None` for a normal space. Every file
    /// operation on a lens (drop, paste, New shortcut, Open folder) and its
    /// watch go here, so they act on the subfolder it is browsed into.
    pub(super) fn lens_folder(&self, id: SpaceId) -> Option<PathBuf> {
        let view = self.config.desktop.space(id)?.folder_view()?;
        Some(subfolder_path(&view.path, &self.subfolder_of(id)))
    }

    /// Shows `item` in place when it is a folder in a lens, and returns
    /// whether it did; anything else is for the caller to open as before.
    /// The lens's old items go at once, and the re-pointed watch lists the
    /// new folder.
    pub(super) fn browse_into_item(&mut self, ui: &mut Ui, item: ItemId) -> bool {
        let Some(id) = self.config.desktop.space_containing(item) else {
            return false;
        };
        let (Some(root), Some(shown)) = (
            self.config
                .desktop
                .space(id)
                .and_then(|space| space.folder_view())
                .map(|view| view.path.clone()),
            self.lens_folder(id),
        ) else {
            return false;
        };
        let Some(folder) = self
            .config
            .desktop
            .item(item)
            .and_then(|entry| browse_into(&root, &shown, entry))
        else {
            return false;
        };
        let Ok(sub) = folder.strip_prefix(&root).map(Path::to_path_buf) else {
            return false;
        };
        self.browsed.insert(id, Browsed { root, sub });
        self.clear_selection(ui);
        self.view_cache.set_scroll(id, 0);
        self.unavailable_lenses.remove(&id);
        self.sync_lens(ui, id, Vec::new());
        self.sync_watches(ui);
        true
    }

    /// Brings the folder watches in line with the model: the Desktop folder
    /// while the layer is drawn, and the folder every lens shows.
    pub(super) fn sync_watches(&mut self, ui: &mut Ui) {
        // A lens whose folder changed (Change folder…, its undo) or that was
        // deleted forgets the subfolder it showed.
        let spaces = &self.config.desktop;
        self.browsed.retain(|id, browsed| {
            spaces
                .space(*id)
                .and_then(|space| space.folder_view())
                .is_some_and(|view| view.path == browsed.root)
        });
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
        let wanted = self.desired_watches(desktop);
        for key in ui.watched_folders() {
            if !wanted.contains_key(&key) {
                ui.unwatch_folder(key);
            }
        }
        // The worker refuses a folder that resolves to the store, off this
        // thread: resolving a path can block on an unreachable share.
        for (key, path) in wanted {
            ui.watch_folder(key, &path, self.storage.final_root());
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
            if let Some(folder) = self.lens_folder(space.id) {
                wanted.insert(WatchKey::Space(space.id), folder);
            }
        }
        wanted
    }

    /// Reconciles the desktop layer after the Desktop folder changed. The
    /// layer re-reads the shell, not the listing, so a listing from another
    /// folder is not stale data: it means the Desktop moved (OneDrive backup,
    /// the Location tab), and the watch follows it before the layer re-reads.
    pub(super) fn on_desktop_listing(&mut self, ui: &mut Ui, folder: &Path) {
        if let Ok(directory) = desktop_directory()
            && !listing_is_current(folder, &directory)
        {
            tracing::debug!(
                listed = %folder.display(),
                desktop = %directory.display(),
                "the Desktop folder moved; watching the new one"
            );
            self.sync_watches(ui);
        }
        self.resync_desktop_layer(ui);
    }

    /// Applies a listing the watch worker posted for lens `id` from `folder`.
    pub(super) fn on_lens_listing(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        folder: &Path,
        listing: FolderListing,
    ) {
        if !self.lens_shows(id, folder) {
            tracing::debug!(
                space = %id,
                listed = %folder.display(),
                "dropped a listing for a folder the lens no longer shows"
            );
            return;
        }
        let entries = self.take_listing(id, listing);
        self.sync_lens(ui, id, entries);
    }

    /// Notes what `listing` says about lens `id` (whether its folder is
    /// available, and which icons went stale) and returns the entries the lens
    /// holds now.
    fn take_listing(&mut self, id: SpaceId, listing: FolderListing) -> Vec<FolderEntry> {
        match listing {
            FolderListing::Listed(entries) => {
                // The icon cache keeps an icon until it is told to drop it, so
                // a file rewritten in place would keep its old icon.
                let changed: Vec<PathBuf> = self
                    .config
                    .desktop
                    .space(id)
                    .map(|space| {
                        entries
                            .iter()
                            .filter(|entry| {
                                space.items().iter().any(|item| {
                                    item.path == entry.path && item.modified != entry.modified
                                })
                            })
                            .map(|entry| entry.path.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                for path in changed {
                    self.icons.invalidate(&path);
                }
                self.unavailable_lenses.remove(&id);
                entries
            }
            FolderListing::Unavailable => {
                self.unavailable_lenses.insert(id);
                tracing::debug!(
                    space = %id,
                    unavailable = self.unavailable_lenses.len(),
                    "lens folder is unavailable"
                );
                Vec::new()
            }
            FolderListing::Refused => {
                self.unavailable_lenses.insert(id);
                // The worker warns with the folder and where it leads.
                tracing::debug!(space = %id, "lens folder is DeskSpace's storage; not watching it");
                Vec::new()
            }
        }
    }

    /// Whether lens `id` shows `folder` now. A listing can wait in the event
    /// queue while Change folder… or a browse moves the lens, so one for its
    /// earlier folder is dropped.
    fn lens_shows(&self, id: SpaceId, folder: &Path) -> bool {
        self.lens_folder(id)
            .is_some_and(|shown| listing_is_current(folder, &shown))
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

/// Whether a listing of `listed` is for `shown`, the folder a surface shows
/// now. Two folders are the same when each is inside the other, by the one
/// rule `is_inside` keeps for comparing paths.
fn listing_is_current(listed: &Path, shown: &Path) -> bool {
    is_inside(listed, shown) && is_inside(shown, listed)
}

/// The folder to show when `entry` is opened in a lens on `root` that shows
/// `shown`: a folder the listing put there, never a shortcut (it can point
/// anywhere), and never outside `root` (D18). `None` means open it as
/// before.
fn browse_into(root: &Path, shown: &Path, entry: &Item) -> Option<PathBuf> {
    if entry.kind != ItemKind::Folder {
        return None;
    }
    // `file_name` is `None` for a path that ends in `..`.
    let name = entry.path.file_name()?;
    if !listing_is_current(entry.path.parent()?, shown) {
        return None;
    }
    let folder = shown.join(name);
    is_inside(&folder, root).then_some(folder)
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

    fn entry(path: &str, kind: ItemKind) -> Item {
        let mut item = Item::new(ItemId(1), path, deskspace_core::item::ItemFacts::default());
        item.kind = kind;
        item
    }

    #[test]
    fn only_a_folder_listed_in_the_folder_shown_is_browsed_into() {
        let root = Path::new(r"C:\Users\me\Downloads");
        let browse = |shown: &Path, path: &str, kind| browse_into(root, shown, &entry(path, kind));
        assert_eq!(
            browse(root, r"C:\Users\me\Downloads\Photos", ItemKind::Folder),
            Some(root.join("Photos"))
        );
        assert_eq!(
            browse(root, r"C:\Users\me\Downloads\notes.txt", ItemKind::File),
            None,
            "a file"
        );
        assert_eq!(
            browse(
                root,
                r"C:\Users\me\Downloads\Elsewhere.lnk",
                ItemKind::Shortcut
            ),
            None,
            "a shortcut can point anywhere"
        );
        assert_eq!(
            browse(root, r"C:\Users\me\Downloads\..", ItemKind::Folder),
            None,
            "the parent"
        );
        assert_eq!(
            browse(root, r"C:\Users\me\Downloads\a..b", ItemKind::Folder),
            Some(root.join("a..b")),
            "a name with two dots in it"
        );
        assert_eq!(
            browse(root, r"c:\users\ME\downloads\Photos", ItemKind::Folder),
            Some(root.join("Photos")),
            "another case keeps the lens's own spelling"
        );
        assert_eq!(
            browse(root, r"C:\Users\me\Documents\Photos", ItemKind::Folder),
            None,
            "a folder the lens does not show"
        );
        let photos = root.join("Photos");
        assert_eq!(
            browse(
                &photos,
                r"C:\Users\me\Downloads\Photos\2026",
                ItemKind::Folder
            ),
            Some(photos.join("2026")),
            "a second level"
        );
    }

    #[test]
    fn a_browsed_lens_watches_and_takes_listings_for_its_subfolder() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        let root = PathBuf::from(r"C:\Users\me\Downloads");
        let shown = root.join(r"Photos\2026");
        app.browsed.insert(
            lens,
            Browsed {
                root: root.clone(),
                sub: PathBuf::from(r"Photos\2026"),
            },
        );
        assert_eq!(app.lens_folder(lens), Some(shown.clone()));
        assert_eq!(
            app.desired_watches(None).get(&WatchKey::Space(lens)),
            Some(&shown)
        );
        assert!(app.lens_shows(lens, &shown));
        assert!(
            !app.lens_shows(lens, &root),
            "a listing of the lens's own folder is stale now"
        );
    }

    #[test]
    fn change_folder_returns_a_browsed_lens_to_its_own_folder() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        app.browsed.insert(
            lens,
            Browsed {
                root: PathBuf::from(r"C:\Users\me\Downloads"),
                sub: PathBuf::from("Photos"),
            },
        );
        fixtures::apply(
            &mut app.config.desktop,
            Command::SetLensFolder {
                id: lens,
                path: r"C:\Users\me\Documents".into(),
                title: "Documents".into(),
            },
        );
        assert_eq!(app.subfolder_of(lens), PathBuf::new());
        assert_eq!(
            app.lens_folder(lens),
            Some(PathBuf::from(r"C:\Users\me\Documents"))
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
    fn a_lens_on_the_storage_folder_is_left_to_the_worker_to_refuse() {
        let mut app = app();
        let root = app.storage.root().to_string_lossy().into_owned();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            &root,
        );
        // Checking the folder here would resolve it on the UI thread.
        assert!(
            app.desired_watches(None)
                .contains_key(&WatchKey::Space(lens))
        );
    }

    #[test]
    fn a_refused_listing_shows_the_lens_unavailable_and_empty() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            "C:/Users/me/Downloads",
        );
        assert!(app.take_listing(lens, FolderListing::Refused).is_empty());
        assert!(app.unavailable_lenses.contains(&lens));
        assert!(
            app.take_listing(lens, FolderListing::Listed(Vec::new()))
                .is_empty()
        );
        assert!(
            !app.unavailable_lenses.contains(&lens),
            "a later listing makes it available again"
        );
    }

    #[test]
    fn a_listing_of_the_shown_folder_is_current() {
        let shown = Path::new(r"C:\Users\me\Downloads");
        assert!(listing_is_current(shown, shown));
        assert!(
            listing_is_current(Path::new(r"c:\users\ME\downloads"), shown),
            "another case"
        );
        assert!(
            listing_is_current(Path::new("C:/Users/me/Downloads"), shown),
            "other slashes"
        );
        assert!(
            listing_is_current(Path::new(r"C:\Users\me\Downloads\"), shown),
            "a trailing separator"
        );
    }

    #[test]
    fn a_listing_of_another_folder_is_stale() {
        let shown = Path::new(r"C:\Users\me\Downloads");
        assert!(!listing_is_current(
            Path::new(r"C:\Users\me\Documents"),
            shown
        ));
        assert!(
            !listing_is_current(Path::new(r"C:\Users\me"), shown),
            "the parent"
        );
        assert!(
            !listing_is_current(Path::new(r"C:\Users\me\Downloads\Old"), shown),
            "a subfolder"
        );
        assert!(
            !listing_is_current(Path::new(r"C:\Users\me\Downloads.old"), shown),
            "a sibling sharing the prefix"
        );
    }

    #[test]
    fn after_change_folder_a_listing_of_the_old_folder_is_dropped() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            "C:/Users/me/Downloads",
        );
        let space = fixtures::create_space(
            &mut app.config.desktop,
            "Space",
            Rect::new(300, 0, 240, 120),
        );
        assert!(app.lens_shows(lens, Path::new("C:/Users/me/Downloads")));
        fixtures::apply(
            &mut app.config.desktop,
            Command::SetLensFolder {
                id: lens,
                path: "C:/Users/me/Documents".into(),
                title: "Documents".into(),
            },
        );
        assert!(!app.lens_shows(lens, Path::new("C:/Users/me/Downloads")));
        assert!(app.lens_shows(lens, Path::new("C:/Users/me/Documents")));
        assert!(
            !app.lens_shows(space, Path::new("C:/Users/me/Documents")),
            "a normal space shows no folder"
        );
    }
}
