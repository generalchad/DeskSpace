//! `App` lens behavior: keeping folder watches in line with the model,
//! turning a watched folder's listing into the lens's items (M4 phase 2), and
//! browsing into subfolders (W42).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deskspace_core::{Command, Item, ItemId, ItemKind, LensBuilder, LensList, SpaceId, lens_title};
use deskspace_platform::{FolderListing, Ui, WatchKey, desktop_directory, is_inside};

use super::{App, Surface, Timer};
use crate::scene::subfolder_path;

/// How long a space's notice bar shows.
const NOTICE_MS: u32 = 3_000;

/// The first line of the notice a lens shows after following its folder;
/// the new path follows on the second.
const FOLDER_MOVED_NOTICE: &str = "Folder moved:";

/// Below this many items, freeing a list costs less than starting a thread.
const FREE_OFF_THREAD_FROM: usize = 1_000;

/// What lens `follow_moved_folder` decided about a moved folder.
#[derive(Debug)]
enum Moved {
    /// The lens now shows the new path; run these effects.
    Followed(Vec<super::effect::Effect>),
    /// A browsed lens returns to its own folder, given here.
    ReturnHome(PathBuf),
    /// The space is gone, or the change did not apply.
    Ignored,
}

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
        self.show_subfolder(ui, id, root, sub);
        true
    }

    /// Shows the parent of the subfolder lens `id` shows (D18), and returns
    /// whether there was one; at the lens's own folder it does nothing.
    pub(super) fn browse_up(&mut self, ui: &mut Ui, id: SpaceId) -> bool {
        let Some(root) = self
            .config
            .desktop
            .space(id)
            .and_then(|space| space.folder_view())
            .map(|view| view.path.clone())
        else {
            return false;
        };
        let Some(parent) = parent_subfolder(&self.subfolder_of(id)) else {
            return false;
        };
        self.show_subfolder(ui, id, root, parent);
        true
    }

    /// Whether lens `id` shows a subfolder, so there is somewhere to go back
    /// to.
    /// It runs after every event, so it borrows rather than copies the path.
    pub(super) fn is_browsed(&self, id: SpaceId) -> bool {
        let Some(view) = self
            .config
            .desktop
            .space(id)
            .and_then(|space| space.folder_view())
        else {
            return false;
        };
        self.browsed
            .get(&id)
            .is_some_and(|browsed| browsed.root == view.path)
    }

    /// Points lens `id` (on `root`) at `sub` below its own folder, or at its
    /// own folder when `sub` is empty. The old items go at once, and the
    /// re-pointed watch lists the new folder.
    pub(super) fn show_subfolder(&mut self, ui: &mut Ui, id: SpaceId, root: PathBuf, sub: PathBuf) {
        if sub.as_os_str().is_empty() {
            self.browsed.remove(&id);
        } else {
            self.browsed.insert(id, Browsed { root, sub });
        }
        // The lens stays the current space, so the keys keep reaching it, as
        // Explorer keeps focus in the folder it opens; `sync_lens` repaints.
        if self.selection.surface == Some(Surface::Space(id)) {
            self.selection.clear_items();
        }
        self.view_cache.set_scroll(id, 0);
        self.unavailable_lenses.remove(&id);
        self.sync_lens(ui, id, Vec::new());
        self.sync_watches(ui);
    }

    /// Returns lens `id` to its own folder.
    pub(super) fn forget_browse(&mut self, id: SpaceId) {
        self.browsed.remove(&id);
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
        self.listed
            .retain(|id, _| spaces.space(*id).is_some_and(|space| space.is_lens()));
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
            if ui.is_watching_folder(key, &path) {
                if let WatchKey::Space(id) = key
                    && let Some(rules) = self
                        .config
                        .desktop
                        .space(id)
                        .and_then(|space| space.lens_rules())
                {
                    ui.set_lens_rules(key, rules);
                }
                continue;
            }
            let builder = match key {
                WatchKey::Desktop => None,
                WatchKey::Space(id) => self.new_lens_builder(id),
            };
            ui.watch_folder(key, &path, self.storage.final_root(), builder);
        }
    }

    /// What a new watch on lens `id` builds its lists with: the lens's rules,
    /// ids reserved for it, and the ids of what the lens holds now, such as
    /// items an undo brought back.
    fn new_lens_builder(&mut self, id: SpaceId) -> Option<LensBuilder> {
        let ids = self.config.desktop.reserve_item_ids();
        let space = self.config.desktop.space(id)?;
        Some(LensBuilder::new(space.lens_rules()?, ids, space.items()))
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
            free_off_thread(items_of(listing));
            return;
        }
        if let FolderListing::Moved { to } = listing {
            self.on_lens_moved(ui, id, &to);
            return;
        }
        if let FolderListing::Listed(list) = &listing
            && !self.list_is_current(id, list)
        {
            // The worker builds again for the current rules. Telling it here
            // too means no change of rules can leave a lens waiting for good.
            tracing::debug!(space = %id, "dropped a list built under older rules");
            free_off_thread(items_of(listing));
            self.sync_watches(ui);
            return;
        }
        match &listing {
            FolderListing::Listed(list) => {
                self.listed
                    .insert(id, (folder.to_path_buf(), list.listed_any));
            }
            FolderListing::Unavailable | FolderListing::Refused | FolderListing::Moved { .. } => {
                self.listed.remove(&id);
            }
        }
        let items = self.take_listing(id, listing);
        self.sync_lens(ui, id, items);
    }

    /// Whether `list` was built under the rules lens `id` has now.
    fn list_is_current(&self, id: SpaceId, list: &LensList) -> bool {
        self.config
            .desktop
            .space(id)
            .and_then(|space| space.lens_rules())
            .is_some_and(|rules| rules == list.rules)
    }

    /// Tells lens `id`'s worker where an item the model re-pointed now is, so
    /// the next listing keeps its id. A rename's new path is otherwise unknown
    /// to the worker, and a normal space has no worker to tell.
    pub(super) fn note_renamed_lens_item(&self, ui: &mut Ui, item: ItemId) {
        let desktop = &self.config.desktop;
        let Some(id) = desktop.space_containing(item) else {
            return;
        };
        if desktop.space(id).is_some_and(|space| space.is_lens())
            && let Some(entry) = desktop.item(item)
        {
            ui.note_lens_item(
                WatchKey::Space(id),
                entry.path.clone(),
                item,
                entry.modified,
            );
        }
    }

    /// Acts on the folder lens `id` shows having moved to `to` (ROADMAP D19).
    fn on_lens_moved(&mut self, ui: &mut Ui, id: SpaceId, to: &Path) {
        match self.follow_moved_folder(id, to) {
            Moved::Followed(effects) => {
                self.run(ui, effects);
                self.sync_watches(ui);
                self.resnapshot_settings(ui, id);
                Self::set_timer(ui, Timer::Notice, NOTICE_MS);
                self.refresh_space(ui, id);
            }
            Moved::ReturnHome(root) => self.show_subfolder(ui, id, root, PathBuf::new()),
            Moved::Ignored => {}
        }
    }

    /// The model half of `on_lens_moved`, without a `Ui`. A lens follows its
    /// own folder as one "Change folder" undo entry and says where it went on
    /// its notice bar. A lens browsed into a subfolder that moved returns to
    /// its own folder instead.
    fn follow_moved_folder(&mut self, id: SpaceId, to: &Path) -> Moved {
        let Some((view, current)) = self.config.desktop.space(id).and_then(|space| {
            space
                .folder_view()
                .cloned()
                .map(|view| (view, space.title().to_owned()))
        }) else {
            return Moved::Ignored;
        };
        if self.is_browsed(id) {
            tracing::info!(space = %id, to = %to.display(), "a browsed lens's folder moved; returning to the lens's own folder");
            return Moved::ReturnHome(view.path);
        }
        let mut next = view;
        next.path = to.to_path_buf();
        let title = lens_title(&next, &current, &self.profile);
        let at = self.space_center(id);
        let (applied, effects) = self.set_lens_folder(id, next.path, title, at);
        if !applied {
            return Moved::Ignored;
        }
        tracing::info!(space = %id, to = %to.display(), "a lens followed its moved folder");
        self.unavailable_lenses.remove(&id);
        self.notices
            .insert(id, format!("{FOLDER_MOVED_NOTICE}\n{}", to.display()));
        Moved::Followed(effects)
    }

    /// Clears every space's notice bar once `Timer::Notice` fires.
    pub(super) fn clear_notices(&mut self, ui: &mut Ui) {
        Self::clear_timer(ui, Timer::Notice);
        for id in std::mem::take(&mut self.notices).into_keys() {
            self.refresh_space(ui, id);
        }
    }

    /// Whether lens `id`'s filters leave none of its listing's entries, so
    /// its placeholder says so instead of looking broken (W35).
    pub(super) fn filtered_out(&self, id: SpaceId) -> bool {
        let Some(space) = self.config.desktop.space(id) else {
            return false;
        };
        let filtered = space
            .folder_view()
            .is_some_and(|view| !view.include.is_empty() || !view.exclude.is_empty());
        filtered
            && space.items().is_empty()
            && self
                .listed
                .get(&id)
                .is_some_and(|(folder, listed_any)| *listed_any && self.lens_shows(id, folder))
    }

    /// Notes what `listing` says about lens `id` (whether its folder is
    /// available, and which icons went stale) and returns the items the lens
    /// holds now.
    fn take_listing(&mut self, id: SpaceId, listing: FolderListing) -> Vec<Item> {
        match listing {
            FolderListing::Listed(list) => {
                // The icon cache keeps an icon until it is told to drop it, so
                // a file rewritten in place would keep its old icon.
                for path in &list.rewritten {
                    self.icons.invalidate(path);
                }
                self.unavailable_lenses.remove(&id);
                list.items
            }
            // A move the lens follows never reaches here (`on_lens_listing`).
            FolderListing::Unavailable | FolderListing::Moved { .. } => {
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

    /// Swaps in lens `id`'s finished list and repaints it. The items are not
    /// persisted, so this needs no save.
    fn sync_lens(&mut self, ui: &mut Ui, id: SpaceId, items: Vec<Item>) {
        let Some(at) = self
            .config
            .desktop
            .space(id)
            .map(|space| space.rect().origin())
        else {
            return;
        };
        if self
            .dispatch(Command::SyncLens { id, items }, at)
            .is_applied()
        {
            self.refresh_space(ui, id);
        }
    }
}

/// The items a dropped listing carries.
fn items_of(listing: FolderListing) -> Vec<Vec<Item>> {
    match listing {
        FolderListing::Listed(list) => vec![list.items],
        FolderListing::Unavailable | FolderListing::Refused | FolderListing::Moved { .. } => {
            Vec::new()
        }
    }
}

/// Frees item lists without holding up the UI thread: freeing a 50,000-item
/// list takes several milliseconds of a frame's 8. A small list is not worth a
/// thread. If the thread cannot start, the lists are freed here.
pub(super) fn free_off_thread(lists: Vec<Vec<Item>>) {
    if lists.iter().map(Vec::len).sum::<usize>() < FREE_OFF_THREAD_FROM {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("deskspace-free-items".into())
        .spawn(move || drop(lists));
}

/// Whether a listing of `listed` is for `shown`, the folder a surface shows
/// now. Two folders are the same when each is inside the other, by the one
/// rule `is_inside` keeps for comparing paths.
fn listing_is_current(listed: &Path, shown: &Path) -> bool {
    is_inside(listed, shown) && is_inside(shown, listed)
}

/// The parent of `sub`, a lens's subfolder relative to its own folder: empty
/// for the lens's own folder, and `None` when `sub` already is it, since a
/// lens never goes above its folder (D18).
fn parent_subfolder(sub: &Path) -> Option<PathBuf> {
    if sub.as_os_str().is_empty() {
        return None;
    }
    Some(sub.parent().map(Path::to_path_buf).unwrap_or_default())
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
    use deskspace_core::source::FolderEntry;

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
    fn a_list_built_under_older_rules_is_not_current() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        let rules = app
            .config
            .desktop
            .space(lens)
            .and_then(|space| space.lens_rules())
            .unwrap();
        let list = LensList {
            rules,
            ..LensList::default()
        };
        assert!(app.list_is_current(lens, &list));
        let mut view = app
            .config
            .desktop
            .space(lens)
            .unwrap()
            .folder_view()
            .unwrap()
            .clone();
        view.include = vec!["*.pdf".into()];
        fixtures::apply(
            &mut app.config.desktop,
            Command::SetLensView { id: lens, view },
        );
        assert!(!app.list_is_current(lens, &list));
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
    fn filters_that_leave_no_entry_are_told_apart_from_an_empty_folder() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        let folder = PathBuf::from(r"C:\Users\me\Downloads");
        let entry = FolderEntry {
            path: folder.join("notes.txt"),
            label: String::new(),
            kind: ItemKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        };
        // An empty folder, with no filter: nothing to say.
        app.listed.insert(lens, (folder.clone(), false));
        assert!(!app.filtered_out(lens));
        // A file the "Show only" list leaves out.
        app.listed.insert(lens, (folder.clone(), true));
        let mut view = app
            .config
            .desktop
            .space(lens)
            .unwrap()
            .folder_view()
            .unwrap()
            .clone();
        view.include = vec!["*.pdf".into()];
        fixtures::apply(
            &mut app.config.desktop,
            Command::SetLensView { id: lens, view },
        );
        fixtures::sync_lens(&mut app.config.desktop, lens, vec![entry]);
        assert!(app.filtered_out(lens));
        // A listing of a folder the lens no longer shows proves nothing.
        app.listed
            .insert(lens, (PathBuf::from(r"C:\Elsewhere"), true));
        assert!(!app.filtered_out(lens));
    }

    #[test]
    fn back_goes_to_the_parent_and_never_above_the_lens() {
        assert_eq!(
            parent_subfolder(Path::new(r"Photos\2026")),
            Some(PathBuf::from("Photos"))
        );
        assert_eq!(
            parent_subfolder(Path::new("Photos")),
            Some(PathBuf::new()),
            "one level down goes back to the lens's own folder"
        );
        assert_eq!(parent_subfolder(Path::new("")), None, "nothing above it");
    }

    #[test]
    fn a_lens_follows_its_moved_folder_and_says_so() {
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Work",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Work",
        );
        app.unavailable_lenses.insert(lens);
        let to = Path::new(r"C:\Archive\Projects");
        assert!(matches!(
            app.follow_moved_folder(lens, to),
            Moved::Followed(_)
        ));
        let space = app.config.desktop.space(lens).unwrap();
        assert_eq!(space.folder_view().unwrap().path, to);
        assert_eq!(space.title(), "Projects", "the title follows the folder");
        assert!(!app.unavailable_lenses.contains(&lens));
        assert_eq!(
            app.notices.get(&lens).map(String::as_str),
            Some("Folder moved:\nC:\\Archive\\Projects")
        );
        assert_eq!(app.undo.undo_label(), Some("Change folder"));
    }

    #[test]
    fn a_browsed_lens_whose_folder_moved_returns_to_its_own_folder() {
        let mut app = app();
        let root = PathBuf::from(r"C:\Users\me\Downloads");
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Downloads",
        );
        app.browsed.insert(
            lens,
            Browsed {
                root: root.clone(),
                sub: PathBuf::from("Photos"),
            },
        );
        let moved = app.follow_moved_folder(lens, Path::new(r"C:\Users\me\Pictures\Photos"));
        assert!(matches!(moved, Moved::ReturnHome(home) if home == root));
        let space = app.config.desktop.space(lens).unwrap();
        assert_eq!(
            space.folder_view().unwrap().path,
            root,
            "the lens keeps its folder"
        );
        assert!(app.notices.is_empty());
        assert_eq!(app.undo.undo_label(), None);
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
            app.take_listing(lens, FolderListing::Listed(LensList::default()))
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

    /// Run by hand in release for the Performance rule's dated timing of what
    /// stays on the UI thread for a big lens:
    /// `cargo test --release -p deskspace-app -- --ignored --nocapture time_the_ui`.
    #[test]
    #[ignore = "a timing, for release builds; run by hand"]
    fn time_the_ui_threads_share_of_a_fifty_thousand_entry_lens() {
        let time = |label: &str, work: &mut dyn FnMut()| {
            let start = std::time::Instant::now();
            work();
            println!("{label}: {:?}", start.elapsed());
        };
        let entries: Vec<FolderEntry> = (0..50_000)
            .map(|n| FolderEntry {
                path: PathBuf::from(format!(
                    r"C:\Users\me\Big\quarterly report {n} final.{}",
                    ["pdf", "docx", "png", "txt", "zip", "exe", "lnk"][n as usize % 7]
                )),
                label: format!("quarterly report {n} final"),
                kind: ItemKind::File,
                size: Some(n),
                modified: Some(n),
                created: Some(n),
                hidden: false,
            })
            .collect();
        let mut app = app();
        let lens = fixtures::create_lens(
            &mut app.config.desktop,
            "Lens",
            Rect::new(0, 0, 240, 120),
            r"C:\Users\me\Big",
        );
        let mut builder = app.new_lens_builder(lens).unwrap();
        let mut list = builder.listed(entries.clone());
        time("swap, a first list", &mut || {
            let items = std::mem::take(&mut list.items);
            app.dispatch(Command::SyncLens { id: lens, items }, Default::default());
        });
        let mut list = builder.listed(entries.clone());
        time("swap, replacing a list", &mut || {
            let items = std::mem::take(&mut list.items);
            app.dispatch(Command::SyncLens { id: lens, items }, Default::default());
        });
        time("the undo snapshot of the lens", &mut || {
            app.undo.begin("Timing");
            app.undo.note_space(&app.config.desktop, lens);
            app.undo.cancel();
        });
        time("rename one item, kept order", &mut || {
            let id = app.config.desktop.space(lens).unwrap().items()[25_000].id;
            app.dispatch(
                Command::RenameItem {
                    id,
                    path: PathBuf::from(r"C:\Users\me\Big\zzzz renamed.pdf"),
                    label: "zzzz renamed".into(),
                },
                Default::default(),
            );
        });
        time("the Shortcuts preview of the lens", &mut || {
            let preview = app.space_preview(Some(lens));
            assert_eq!(preview.items.len(), 50_000);
        });
        for key in [
            deskspace_core::SortKey::Name,
            deskspace_core::SortKey::Kind,
            deskspace_core::SortKey::Size,
            deskspace_core::SortKey::Modified,
            deskspace_core::SortKey::Created,
        ] {
            let mut view = app.config.desktop.space(lens).unwrap().view().clone();
            view.group = Some(deskspace_core::view::GroupBy {
                key,
                descending: false,
            });
            fixtures::apply(
                &mut app.config.desktop,
                Command::SetSpaceView { id: lens, view },
            );
            time(&format!("sync_grouping by {key:?}, built"), &mut || {
                app.sync_grouping(lens);
            });
            time(&format!("sync_grouping by {key:?}, unchanged"), &mut || {
                app.sync_grouping(lens);
            });
            app.view_cache.toggle_group(lens, "no such group");
            time(&format!("sync_grouping by {key:?}, again"), &mut || {
                app.sync_grouping(lens);
            });
        }
    }
}
