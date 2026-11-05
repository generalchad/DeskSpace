//! The shell icon cache: base icons from the image factory, the classic
//! fallback, and the overlays composited as Explorer draws them. A scene's
//! icons load on a worker thread, so a slow folder never stalls the spaces.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use deskspace_core::ItemKind;

use crate::render::Icon;
use crate::win32::runtime;

mod load;
mod worker;

use load::{load_generic_file_icon, load_generic_folder_icon, load_icon};
pub(crate) use load::{resample, shell_item_icon};
use worker::{Worker, start_worker};

/// The window a loaded icon's scene goes to.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum IconUser {
    Space(u64),
    DesktopLayer,
    /// The settings window's Shortcuts preview of space `u64`, which asks for
    /// only the icons its first page shows.
    SettingsPreview(u64),
}

/// What a cached icon was rendered for, so two uses of one path never share
/// the wrong pixels: an item with the shell's overlays, or a lens title's
/// folder icon (whose availability decides the fallback).
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum IconKind {
    Item {
        overlay_size: u8,
    },
    /// A folder's icon for a lens title bar, with whether the folder could be
    /// listed.
    Folder {
        available: bool,
    },
}

/// One icon: a path, the pixel size it was rendered at, and what it is for.
pub(super) type Key = (PathBuf, i32, IconKind);

/// The icon drawn until an item's own has loaded.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum GenericIcon {
    Folder,
    File,
}

impl GenericIcon {
    fn for_kind(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Folder | ItemKind::Drive => Self::Folder,
            ItemKind::File | ItemKind::Shortcut | ItemKind::Other => Self::File,
        }
    }
}

/// Icons decoded per path, pixel size, and overlay size, bounded by a
/// least-recently-used limit. Evicting is always safe: scenes hold their own
/// reference-counted `Icon`s. The size is part of the key, so a per-monitor DPI
/// or icon-size change does not hand out stale pixels, and the same holds for a
/// changed overlay size.
pub struct IconCache {
    capacity: usize,
    clock: u64,
    entries: HashMap<Key, CachedIcon>,
    /// The entries' keys by last use, so the oldest is the first and eviction
    /// needs no scan of `entries`.
    by_age: BTreeMap<u64, Key>,
    /// The generic icon per kind and size, including a failed shell read, so a
    /// missing one is not asked for again on every item.
    generic: HashMap<(GenericIcon, i32), Option<Icon>>,
    /// The scene being built, and the icons it found missing or out of date.
    scene: Option<IconUser>,
    wanted: Vec<Key>,
    scene_items: usize,
    /// How many icons each window's last scene drew. The limit never falls
    /// below their sum, or a repaint would evict icons still on screen and
    /// load them again, for as long as the windows stay up.
    demand: HashMap<IconUser, usize>,
    worker: Option<Arc<Worker>>,
}

struct CachedIcon {
    /// `None` when the shell could not draw it, so it is not asked again.
    icon: Option<Icon>,
    /// The file changed: draw these pixels until the new ones have loaded,
    /// so a rewritten file does not flash the generic icon.
    stale: bool,
    last_used: u64,
}

impl IconCache {
    /// Enough for every visible item on a crowded desktop.
    pub const DEFAULT_CAPACITY: usize = 1024;

    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            clock: 0,
            entries: HashMap::new(),
            by_age: BTreeMap::new(),
            generic: HashMap::new(),
            scene: None,
            wanted: Vec::new(),
            scene_items: 0,
            demand: HashMap::new(),
            worker: None,
        }
    }

    /// Starts `user`'s scene: `icon_for` now notes what it is missing, and
    /// `end_scene` asks for those icons. Outside a scene it asks for nothing.
    pub fn begin_scene(&mut self, user: IconUser) {
        self.scene = Some(user);
        self.wanted.clear();
        self.scene_items = 0;
    }

    /// Ends the scene: asks for what it was missing, and forgets what its
    /// window asked for before and no longer shows.
    pub fn end_scene(&mut self) {
        let Some(user) = self.scene.take() else {
            return;
        };
        self.demand.insert(user, self.scene_items);
        if self.worker.is_none() && self.wanted.is_empty() {
            return;
        }
        let Some(worker) = self.worker() else {
            return;
        };
        worker.lock().queue.set_wanted(&user, &self.wanted);
        worker.ready.notify_one();
        self.wanted.clear();
    }

    /// The icon to draw now for an item at `path`, rendered at `size` pixels
    /// square with the shell's overlay at `overlay_size` percent of that. Never
    /// touches the item's file: an icon not loaded yet is the generic one for
    /// `kind`, and `Event::IconsLoaded` says when to draw the scene again.
    pub fn icon_for(
        &mut self,
        path: &Path,
        kind: ItemKind,
        size: i32,
        overlay_size: u8,
    ) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Item { overlay_size });
        self.scene_items += 1;
        let (icon, wanted) = match self.use_entry(&key) {
            Some(entry) => (entry.icon.clone(), entry.stale),
            None => (None, true),
        };
        if wanted && self.scene.is_some() {
            self.wanted.push(key);
        }
        icon.or_else(|| self.generic_icon(GenericIcon::for_kind(kind), size))
    }

    /// `icon_for`, but loading a missing icon on this thread, for the few uses
    /// that cannot draw again later: a drag image, and the takeover layer.
    pub fn icon_now(&mut self, path: &Path, size: i32, overlay_size: u8) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Item { overlay_size });
        if let Some(entry) = self.use_entry(&key)
            && !entry.stale
            && let Some(icon) = &entry.icon
        {
            return Some(icon.clone());
        }
        let icon = load_icon(path, size, overlay_size);
        self.store(key, icon.clone(), false);
        icon
    }

    /// The icon to draw now for an item at `path`: the cached one when there is
    /// one, else the generic icon for `kind`. Asks for nothing, for the part of
    /// a picture that is not a scene's request, such as the settings preview's
    /// items past its first page.
    pub fn icon_ready(
        &mut self,
        path: &Path,
        kind: ItemKind,
        size: i32,
        overlay_size: u8,
    ) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Item { overlay_size });
        let icon = self.use_entry(&key).and_then(|entry| entry.icon.clone());
        icon.or_else(|| self.generic_icon(GenericIcon::for_kind(kind), size))
    }

    /// The requests the current scene still wants. For tests that check how
    /// many icons a caller asks for.
    pub fn wanted_len(&self) -> usize {
        self.wanted.len()
    }

    /// Moves what the worker loaded into the cache, and returns the windows
    /// that should draw their scenes again.
    pub fn take_loaded(&mut self) -> Vec<IconUser> {
        let Some(worker) = self.worker.clone() else {
            return Vec::new();
        };
        let mut users = BTreeSet::new();
        let mut state = worker.lock();
        state.reported = false;
        tracing::trace!(count = state.loaded.len(), "icons loaded");
        for (key, icon) in std::mem::take(&mut state.loaded) {
            if let Some(finished) = state.queue.finish(&key) {
                users.extend(finished.users);
                self.store(key, icon, finished.stale);
            }
        }
        users.into_iter().collect()
    }

    /// The icon for a lens's folder, drawn `size` pixels square in a title bar.
    /// An available folder uses its own shell icon once the load worker has it;
    /// until then, and for an unavailable one, this draws the shell's generic
    /// folder icon, so building a scene never waits on a slow folder.
    pub fn folder_icon(&mut self, path: &Path, size: i32, available: bool) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Folder { available });
        self.scene_items += 1;
        let (icon, wanted) = match self.use_entry(&key) {
            Some(entry) => (entry.icon.clone(), entry.stale),
            None => (None, true),
        };
        if wanted && self.scene.is_some() {
            self.wanted.push(key);
        }
        icon.or_else(|| self.generic_icon(GenericIcon::Folder, size))
    }

    /// Marks every size of the icon for `path` out of date, so the next scene
    /// reloads it (for example after the file changed).
    pub fn invalidate(&mut self, path: &Path) {
        let by_age = &mut self.by_age;
        self.entries.retain(|key, entry| {
            if key.0 != path {
                return true;
            }
            entry.stale = true;
            let keep = matches!(key.2, IconKind::Item { .. });
            if !keep {
                by_age.remove(&entry.last_used);
            }
            keep
        });
        if let Some(worker) = &self.worker {
            worker.lock().queue.mark_stale(|key| key.0 == path);
        }
    }

    /// Forgets every cached icon, so each is rendered again on next use.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_age.clear();
        if let Some(worker) = &self.worker {
            worker.lock().queue.mark_stale(|_| true);
        }
    }

    /// Forgets what `user`'s scene last drew, so a window that closed no
    /// longer raises the cache's limit or leaves icons queued for it. Cached
    /// entries stay until age evicts them.
    pub fn forget_user(&mut self, user: IconUser) {
        self.demand.remove(&user);
        if let Some(worker) = &self.worker {
            worker.lock().queue.set_wanted(&user, &[]);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Marks `key` as used now and returns its entry, when it is cached.
    fn use_entry(&mut self, key: &Key) -> Option<&CachedIcon> {
        let entry = self.entries.get_mut(key)?;
        self.clock += 1;
        self.by_age.remove(&entry.last_used);
        entry.last_used = self.clock;
        self.by_age.insert(self.clock, key.clone());
        Some(entry)
    }

    /// Marks `key` as used now and returns its icon, when it is cached.
    #[cfg(test)]
    fn touch(&mut self, key: &Key) -> Option<Icon> {
        self.use_entry(key)?.icon.clone()
    }

    #[cfg(test)]
    fn insert(&mut self, key: Key, icon: Icon) {
        self.store(key, Some(icon), false);
    }

    fn store(&mut self, key: Key, icon: Option<Icon>, stale: bool) {
        self.clock += 1;
        let replaced = self.entries.contains_key(&key);
        if let Some(entry) = self.entries.get(&key) {
            self.by_age.remove(&entry.last_used);
        }
        self.by_age.insert(self.clock, key.clone());
        self.entries.insert(
            key,
            CachedIcon {
                icon,
                stale,
                last_used: self.clock,
            },
        );
        if replaced {
            return;
        }
        while self.entries.len() > self.limit() {
            let Some((_, oldest)) = self.by_age.pop_first() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn limit(&self) -> usize {
        self.capacity.max(self.demand.values().sum())
    }

    fn generic_icon(&mut self, generic: GenericIcon, size: i32) -> Option<Icon> {
        if let Some(cached) = self.generic.get(&(generic, size)) {
            return cached.clone();
        }
        let base = match generic {
            GenericIcon::Folder => load_generic_folder_icon(),
            GenericIcon::File => load_generic_file_icon(),
        };
        let icon = base.map(|base| resample(&base, size.clamp(1, 1024) as u32));
        self.generic.insert((generic, size), icon.clone());
        icon
    }

    /// The load worker, started the first time a scene needs it. Without a
    /// running UI there is no window to report to, so there is none.
    ///
    /// While no `SHGetFileInfoW` call has finished yet in the process, calls
    /// on other threads can succeed with a null icon: 2 or 3 of 300 on each of
    /// 8 threads, and none in 14,400 once one call had finished (measured
    /// 2026-10-31, Windows 11 build 26300). So one finishes here first.
    fn worker(&mut self) -> Option<Arc<Worker>> {
        if self.worker.is_none() {
            let target = runtime::controller_window()?;
            let _ = load_generic_file_icon();
            self.worker = start_worker(target);
        }
        self.worker.clone()
    }
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IconCache {
    /// Asks the worker to stop without waiting for it: a shell call on a dead
    /// network path can block for a long time, and exit must not wait for that.
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.lock().stop = true;
            worker.ready.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon() -> Icon {
        Icon::new(1, 1, vec![0; 4].into_boxed_slice())
    }

    fn key(path: &str) -> Key {
        (PathBuf::from(path), 32, IconKind::Item { overlay_size: 80 })
    }

    #[test]
    fn evicts_the_least_recently_used_icon() {
        let mut cache = IconCache::with_capacity(2);
        cache.insert(key("a"), icon());
        cache.insert(key("b"), icon());
        // Touch "a" so "b" becomes the oldest.
        assert!(cache.touch(&key("a")).is_some());
        cache.insert(key("c"), icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&key("a")));
        assert!(!cache.entries.contains_key(&key("b")));
    }

    #[test]
    fn a_hit_protects_the_icon_from_the_next_eviction() {
        let mut cache = IconCache::with_capacity(2);
        cache.insert(key("a"), icon());
        cache.insert(key("b"), icon());
        // A hit on the newest still moves its age, so the oldest is evicted.
        assert!(cache.touch(&key("b")).is_some());
        cache.insert(key("c"), icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&key("b")));
        assert!(cache.entries.contains_key(&key("c")));
        assert!(!cache.entries.contains_key(&key("a")));
        assert_eq!(cache.by_age.len(), cache.entries.len(), "ages stay in step");
    }

    #[test]
    fn invalidate_marks_every_item_size_and_forgets_a_folder_icon() {
        let mut cache = IconCache::new();
        let larger = (PathBuf::from("a"), 48, key("a").2);
        cache.insert(key("a"), icon());
        cache.insert(larger.clone(), icon());
        cache.insert(
            (PathBuf::from("a"), 32, IconKind::Folder { available: true }),
            icon(),
        );
        cache.insert(key("b"), icon());
        cache.invalidate(Path::new("a"));
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.by_age.len(), 3, "ages stay in step");
        assert!(cache.entries[&key("a")].stale);
        assert!(cache.entries[&larger].stale);
        assert!(!cache.entries[&key("b")].stale);
    }

    /// A cache with a worker's shared state but no thread, so a test plays the
    /// worker's part.
    fn cache_with_queue() -> IconCache {
        let mut cache = IconCache::new();
        cache.worker = Some(Arc::new(Worker::new()));
        // Already known, so these tests never reach the shell.
        cache.generic.insert((GenericIcon::File, 32), Some(icon()));
        cache
    }

    fn scene(cache: &mut IconCache, user: IconUser, paths: &[&str]) {
        cache.begin_scene(user);
        for path in paths {
            cache.icon_for(Path::new(path), ItemKind::File, 32, 80);
        }
        cache.end_scene();
    }

    /// Plays the worker: takes every request and hands back `result` for each.
    fn load_all(cache: &IconCache, result: Option<Icon>) -> usize {
        let worker = cache.worker.as_ref().expect("a worker");
        let mut state = worker.lock();
        let mut count = 0;
        while let Some(key) = state.queue.pop() {
            state.loaded.push((key, result.clone()));
            count += 1;
        }
        count
    }

    const SPACE: IconUser = IconUser::Space(1);

    #[test]
    fn a_missing_icon_is_asked_for_and_reaches_the_cache() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["a"]);
        assert!(cache.is_empty(), "nothing loads on this thread");
        assert_eq!(load_all(&cache, Some(icon())), 1);
        assert_eq!(cache.take_loaded(), [SPACE]);
        assert!(cache.touch(&key("a")).is_some());
    }

    #[test]
    fn icon_ready_draws_the_generic_icon_and_asks_for_nothing() {
        let mut cache = cache_with_queue();
        cache.generic.insert((GenericIcon::File, 32), Some(icon()));
        cache.begin_scene(SPACE);
        assert!(
            cache
                .icon_ready(Path::new("a"), ItemKind::File, 32, 80)
                .is_some()
        );
        cache.end_scene();
        assert_eq!(cache.wanted_len(), 0);
        assert_eq!(load_all(&cache, Some(icon())), 0);
    }

    #[test]
    fn icon_ready_uses_a_cached_icon() {
        let mut cache = cache_with_queue();
        let held = icon();
        cache.insert(key("a"), held.clone());
        let drawn = cache.icon_ready(Path::new("a"), ItemKind::File, 32, 80);
        assert_eq!(drawn.map(|icon| icon.id()), Some(held.id()));
        assert_eq!(cache.wanted_len(), 0);
    }

    #[test]
    fn a_loaded_icon_is_not_asked_for_again() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["a"]);
        load_all(&cache, Some(icon()));
        cache.take_loaded();
        scene(&mut cache, SPACE, &["a"]);
        assert_eq!(load_all(&cache, Some(icon())), 0);
    }

    #[test]
    fn an_icon_the_shell_cannot_draw_is_not_asked_for_again() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["a"]);
        load_all(&cache, None);
        assert_eq!(cache.take_loaded(), [SPACE]);
        scene(&mut cache, SPACE, &["a"]);
        assert_eq!(load_all(&cache, Some(icon())), 0);
    }

    #[test]
    fn only_the_windows_that_asked_draw_again() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["a"]);
        scene(&mut cache, IconUser::DesktopLayer, &["b"]);
        scene(&mut cache, IconUser::Space(2), &[]);
        load_all(&cache, Some(icon()));
        assert_eq!(cache.take_loaded(), [SPACE, IconUser::DesktopLayer]);
        assert!(cache.take_loaded().is_empty(), "each is reported once");
    }

    #[test]
    fn an_icon_scrolled_out_of_view_never_loads() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["gone"]);
        scene(&mut cache, SPACE, &["kept"]);
        assert_eq!(load_all(&cache, Some(icon())), 1);
        cache.take_loaded();
        assert!(cache.touch(&key("kept")).is_some());
        assert!(cache.touch(&key("gone")).is_none());
    }

    #[test]
    fn a_changed_file_keeps_its_old_icon_until_the_new_one_loads() {
        let mut cache = cache_with_queue();
        let old = icon();
        cache.insert(key("a"), old.clone());
        cache.invalidate(Path::new("a"));
        cache.begin_scene(SPACE);
        let drawn = cache.icon_for(Path::new("a"), ItemKind::File, 32, 80);
        cache.end_scene();
        assert_eq!(drawn.map(|icon| icon.id()), Some(old.id()));
        let new = icon();
        assert_eq!(load_all(&cache, Some(new.clone())), 1);
        cache.take_loaded();
        assert_eq!(cache.touch(&key("a")).map(|icon| icon.id()), Some(new.id()));
        assert!(!cache.entries[&key("a")].stale);
        assert_eq!(cache.by_age.len(), cache.len(), "ages stay in step");
    }

    #[test]
    fn a_file_that_changes_while_loading_is_loaded_again() {
        let mut cache = cache_with_queue();
        scene(&mut cache, SPACE, &["a"]);
        let worker = cache.worker.clone().expect("a worker");
        let popped = worker.lock().queue.pop().expect("a request");
        cache.invalidate(Path::new("a"));
        worker.lock().loaded.push((popped, Some(icon())));
        assert_eq!(cache.take_loaded(), [SPACE]);
        scene(&mut cache, SPACE, &["a"]);
        assert_eq!(load_all(&cache, Some(icon())), 1);
    }

    #[test]
    fn a_scene_outside_a_window_asks_for_nothing() {
        let mut cache = cache_with_queue();
        cache.icon_for(Path::new("a"), ItemKind::File, 32, 80);
        cache.end_scene();
        assert_eq!(load_all(&cache, Some(icon())), 0);
    }

    #[test]
    fn a_scene_larger_than_the_cache_keeps_its_icons() {
        let mut cache = cache_with_queue();
        cache.capacity = 2;
        let paths = ["a", "b", "c"];
        scene(&mut cache, SPACE, &paths);
        load_all(&cache, Some(icon()));
        cache.take_loaded();
        assert_eq!(cache.len(), 3);
        scene(&mut cache, SPACE, &paths);
        assert_eq!(load_all(&cache, Some(icon())), 0, "or it never settles");
    }

    #[test]
    fn folders_and_drives_get_the_folder_icon_until_loaded() {
        assert_eq!(GenericIcon::for_kind(ItemKind::Folder), GenericIcon::Folder);
        assert_eq!(GenericIcon::for_kind(ItemKind::Drive), GenericIcon::Folder);
        for kind in [ItemKind::File, ItemKind::Shortcut, ItemKind::Other] {
            assert_eq!(GenericIcon::for_kind(kind), GenericIcon::File);
        }
    }

    #[test]
    fn evicted_icons_stay_usable_by_their_holders() {
        let mut cache = IconCache::with_capacity(1);
        let held = icon();
        cache.insert(key("a"), held.clone());
        cache.insert(key("b"), icon());
        assert_eq!(held.pixels().len(), 4);
    }

    #[test]
    fn forgetting_a_window_lowers_the_limit_and_evicts_down_to_it() {
        let mut cache = IconCache::with_capacity(2);
        let user = IconUser::Space(1);
        cache.demand.insert(user, 3);
        for path in ["a", "b", "c"] {
            cache.insert(key(path), icon());
        }
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.limit(), 3, "the scene's icons hold the limit up");

        cache.forget_user(user);
        assert_eq!(cache.limit(), 2, "the limit falls with the window's demand");
        cache.insert(key("d"), icon());
        assert_eq!(cache.len(), 2);
        assert!(!cache.entries.contains_key(&key("a")), "oldest first");
        assert!(!cache.entries.contains_key(&key("b")));
        assert!(cache.entries.contains_key(&key("c")));
        assert!(cache.entries.contains_key(&key("d")));
        assert_eq!(cache.by_age.len(), cache.entries.len(), "ages stay in step");
    }

    #[test]
    fn a_folder_title_icon_is_generic_until_the_worker_loads_it() {
        let missing = Path::new("C:/nowhere/DeskSpace title icon test");
        let folder_key = |available| (missing.to_path_buf(), 16, IconKind::Folder { available });
        let mut cache = cache_with_queue();
        cache
            .generic
            .insert((GenericIcon::Folder, 16), Some(icon()));

        // Outside a scene nothing is asked for, and the generic icon is drawn.
        assert!(cache.folder_icon(missing, 16, true).is_some());
        assert_eq!(load_all(&cache, Some(icon())), 0);

        // In a scene, the generic icon is drawn now and the real one is queued.
        cache.begin_scene(SPACE);
        assert!(cache.folder_icon(missing, 16, true).is_some());
        cache.end_scene();
        assert!(
            cache.touch(&folder_key(true)).is_none(),
            "nothing loads on this thread"
        );
        assert_eq!(load_all(&cache, Some(icon())), 1);
        assert_eq!(cache.take_loaded(), [SPACE]);
        assert!(cache.touch(&folder_key(true)).is_some());

        // Once loaded, the folder is not asked for again.
        cache.begin_scene(SPACE);
        cache.folder_icon(missing, 16, true);
        cache.end_scene();
        assert_eq!(load_all(&cache, Some(icon())), 0);

        // An unavailable folder keeps its own entry, so the two never share.
        cache.begin_scene(SPACE);
        assert!(cache.folder_icon(missing, 16, false).is_some());
        cache.end_scene();
        assert_eq!(load_all(&cache, Some(icon())), 1);
        cache.take_loaded();
        assert!(cache.touch(&folder_key(false)).is_some());
        assert_eq!(cache.len(), 2);
    }
}
