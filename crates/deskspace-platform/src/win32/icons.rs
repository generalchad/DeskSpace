//! The shell icon cache: base icons from the image factory, the classic
//! fallback, and the overlays composited as Explorer draws them. A scene's
//! icons load on a worker thread, so a slow folder never stalls the spaces.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use deskspace_core::ItemKind;
use windows::Win32::Foundation::{HWND, LPARAM, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SHFILEINFOW,
    SHGFI_ADDOVERLAYS, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_LINKOVERLAY, SHGFI_OVERLAYINDEX,
    SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO,
    SHIL_LARGE, SHIL_SMALL, SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, PostMessageW};
use windows::core::{Interface, PCWSTR};

use crate::icon_queue::IconQueue;
use crate::render::Icon;
use crate::render::d2d::{decode_bitmap, decode_icon};
use crate::win32::shell_items::shell_path;
use crate::win32::{messages, runtime};

/// How often a busy worker reports what it loaded, so a page of icons fills
/// in a few steps rather than one repaint per icon.
const LOADED_REPORT_INTERVAL: Duration = Duration::from_millis(33);

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
enum IconKind {
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
type Key = (PathBuf, i32, IconKind);

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

/// What the UI thread and the load worker share.
struct Worker {
    state: Mutex<WorkerState>,
    ready: Condvar,
}

struct WorkerState {
    queue: IconQueue<Key, IconUser>,
    loaded: Vec<(Key, Option<Icon>)>,
    /// A report is on its way to the UI thread, which takes everything loaded
    /// by then, so another would be wasted.
    reported: bool,
    stop: bool,
}

impl Worker {
    fn new() -> Self {
        Self {
            state: Mutex::new(WorkerState {
                queue: IconQueue::new(),
                loaded: Vec::new(),
                reported: false,
                stop: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, WorkerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
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
        if let Some(replaced) = self.entries.get(&key) {
            self.by_age.remove(&replaced.last_used);
        } else if self.entries.len() >= self.limit()
            && let Some((_, oldest)) = self.by_age.pop_first()
        {
            self.entries.remove(&oldest);
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

fn start_worker(target: HWND) -> Option<Arc<Worker>> {
    let worker = Arc::new(Worker::new());
    let shared = worker.clone();
    let target = target.0 as isize;
    let spawned = std::thread::Builder::new()
        .name("deskspace-icons".into())
        .spawn(move || run_worker(&shared, target));
    match spawned {
        Ok(_) => Some(worker),
        Err(error) => {
            tracing::warn!(%error, "could not start loading icons");
            None
        }
    }
}

fn run_worker(worker: &Worker, target: isize) {
    // SAFETY: this thread enters its own single-threaded apartment, as the
    // image factory needs. It is never left: the thread-local `Graphics` holds
    // COM objects and is dropped after this function returns, when releasing
    // them still needs COM.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let mut last_report = Instant::now();
    while let Some(key) = next_request(worker, target, &mut last_report) {
        let icon = match key.2 {
            IconKind::Item { overlay_size } => load_icon(&key.0, key.1, overlay_size),
            IconKind::Folder { available } => {
                load_folder_icon(&key.0, key.1.clamp(1, 1024) as u32, available)
            }
        };
        let mut state = worker.lock();
        state.loaded.push((key, icon));
        if should_report(&state, last_report.elapsed()) {
            report_loaded(&mut state, target, &mut last_report);
        }
    }
}

/// Blocks until the UI thread asks for an icon, reporting what is left to
/// report first. `None` means stop.
fn next_request(worker: &Worker, target: isize, last_report: &mut Instant) -> Option<Key> {
    let mut state = worker.lock();
    loop {
        if state.stop {
            return None;
        }
        if let Some(key) = state.queue.pop() {
            return Some(key);
        }
        if !state.loaded.is_empty() && !state.reported {
            report_loaded(&mut state, target, last_report);
        }
        state = worker
            .ready
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// Whether to tell the UI thread now: once the queue has run dry, or when it
/// has waited long enough for a batch.
fn should_report(state: &WorkerState, since_last: Duration) -> bool {
    !state.reported && (state.queue.is_empty() || since_last >= LOADED_REPORT_INTERVAL)
}

fn report_loaded(state: &mut WorkerState, target: isize, last_report: &mut Instant) {
    // SAFETY: posting takes no pointers; a window that has gone just fails.
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            messages::ICONS_LOADED,
            WPARAM(0),
            LPARAM(0),
        )
    };
    state.reported = posted.is_ok();
    *last_report = Instant::now();
}

/// Renders `path` as an icon `size` pixels square, with the shell's overlay
/// (shortcut arrow, sync badge) drawn over it as Explorer does. The base image
/// comes from the shell's image factory, which draws crisp icons at any size
/// (`HICON` only goes up to the small/large system sizes); paths the factory
/// cannot render fall back to the classic shell icon.
fn load_icon(path: &Path, size: i32, overlay_size: u8) -> Option<Icon> {
    let size = size.clamp(1, 1024) as u32;
    let base = load_base_icon(path, size).or_else(|| load_classic_icon(path))?;
    let base = resample(&base, size);
    match overlay_index(path).and_then(|overlay| load_overlay_image(overlay, size, overlay_size)) {
        Some(overlay) => Some(composite_overlay(&base, &overlay)),
        None => Some(base),
    }
}

/// A lens title's folder icon: the folder's own shell icon when it can be
/// listed, else the generic one. Sized by `resample`, so the title bar gets a
/// crisp icon at any DPI.
fn load_folder_icon(path: &Path, size: u32, available: bool) -> Option<Icon> {
    let base = available
        .then(|| load_base_icon(path, size))
        .flatten()
        .or_else(load_generic_folder_icon)?;
    Some(resample(&base, size))
}

/// The shell's generic folder icon, read from the file attributes rather than
/// from the disk: `SHGFI_USEFILEATTRIBUTES` makes `SHGetFileInfoW` trust
/// `FILE_ATTRIBUTE_DIRECTORY`, so a folder that is gone is never touched.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
fn load_generic_folder_icon() -> Option<Icon> {
    load_attribute_icon("folder", FILE_ATTRIBUTE_DIRECTORY)
}

/// The shell's generic icon for a file with no extension, without the disk,
/// as `load_generic_folder_icon` is.
fn load_generic_file_icon() -> Option<Icon> {
    load_attribute_icon("file", FILE_ATTRIBUTE_NORMAL)
}

fn load_attribute_icon(name: &str, attributes: FILE_FLAGS_AND_ATTRIBUTES) -> Option<Icon> {
    let wide = shell_path(Path::new(name));
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES;
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure. The
    // returned HICON is ours and destroyed after its pixels are copied out.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            attributes,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = decode_icon(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
    }
}

/// Whether `path` is a shortcut, which Windows marks with the link arrow.
fn is_shortcut(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("url")
        })
}

/// The classic 32 px `SHGetFileInfoW` icon, without overlays, for paths the
/// image factory rejects (some virtual and shortcut targets).
fn load_classic_icon(path: &Path) -> Option<Icon> {
    let wide = shell_path(path);
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON;
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure. The
    // returned HICON is ours and destroyed after its pixels are copied out.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = decode_icon(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
    }
}

fn load_base_icon(path: &Path, size: u32) -> Option<Icon> {
    let wide = shell_path(path);
    // SAFETY: `wide` is NUL-terminated and outlives the parse.
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }.ok()?;
    shell_item_icon(&item, size)
}

/// Renders a shell item's icon `size` pixels square with the shell's image
/// factory, without overlays. Works for virtual items with no path, such as
/// apps in the Applications folder.
pub(crate) fn shell_item_icon(item: &IShellItem, size: u32) -> Option<Icon> {
    // SAFETY: `item` is live; the image factory and its bitmap are released
    // below, after the pixels are copied.
    unsafe {
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let bitmap: HBITMAP = factory
            .GetImage(
                SIZE {
                    cx: size as i32,
                    cy: size as i32,
                },
                SIIGBF_ICONONLY,
            )
            .ok()?;
        let icon = decode_bitmap(bitmap);
        let _ = DeleteObject(bitmap.into());
        icon
    }
}

/// The 1-based overlay index Explorer would draw on `path`, or `None` when it
/// draws none. `ADDOVERLAYS` asks the registered overlay handlers (sync badges
/// and the like); `LINKOVERLAY` adds the link arrow, and is passed only for
/// shortcuts because it arrows any file it is given.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
fn overlay_index(path: &Path) -> Option<i32> {
    let wide = shell_path(path);
    let mut flags = SHGFI_ICON | SHGFI_OVERLAYINDEX | SHGFI_ADDOVERLAYS;
    if is_shortcut(path) {
        flags |= SHGFI_LINKOVERLAY;
    }
    let mut info = SHFILEINFOW::default();
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure of
    // the size passed. OVERLAYINDEX requires ICON, so an HICON comes back too;
    // it is ours and destroyed at once.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if !info.hIcon.is_invalid() {
            let _ = DestroyIcon(info.hIcon);
        }
        if result == 0 {
            return None;
        }
    }
    // OVERLAYINDEX returns the overlay index in the upper eight bits of iIcon.
    let overlay = (info.iIcon as u32 >> 24) as i32;
    (overlay != 0).then_some(overlay)
}

/// The overlay image `overlay` from the system image list nearest `size`, sized
/// to `overlay_size` percent of `size` and anchored where Explorer draws it: the
/// glyph sits in the bottom-left corner of the full icon, so the scaled image is
/// placed flush to that corner of a `size`-square transparent canvas. The
/// returned image is full `size` square, ready to composite over the base.
/// https://learn.microsoft.com/en-us/windows/win32/api/commoncontrols/nf-commoncontrols-iimagelist-getoverlayimage
fn load_overlay_image(overlay: i32, size: u32, overlay_size: u8) -> Option<Icon> {
    let list = system_image_list(size)?;
    // SAFETY: `list` is a live image list; the HICON `GetIcon` creates is ours
    // and destroyed after its pixels are copied out.
    let icon = unsafe {
        let index = list.GetOverlayImage(overlay).ok()?;
        let hicon = list.GetIcon(index, ILD_TRANSPARENT.0).ok()?;
        let icon = decode_icon(hicon);
        let _ = DestroyIcon(hicon);
        icon
    }?;
    Some(scale_overlay_to_corner(&icon, size, overlay_size))
}

/// `icon` (a full-icon-sized overlay image) scaled to `overlay_size` percent of
/// `size` and placed in the bottom-left corner of a `size`-square transparent
/// canvas, where Explorer draws overlays.
fn scale_overlay_to_corner(icon: &Icon, size: u32, overlay_size: u8) -> Icon {
    let scaled = resample(icon, (size * overlay_size as u32 / 100).max(1));
    let (width, height) = (scaled.width(), scaled.height());
    let mut canvas = vec![0u8; (size * size * 4) as usize];
    let top = size.saturating_sub(height);
    for y in 0..height.min(size) {
        for x in 0..width.min(size) {
            let from = ((y * width + x) * 4) as usize;
            let to = (((top + y) * size + x) * 4) as usize;
            canvas[to..to + 4].copy_from_slice(&scaled.pixels()[from..from + 4]);
        }
    }
    Icon::new(size, size, canvas.into_boxed_slice())
}

/// The smallest system image list whose icons are at least `size` pixels, or
/// the largest one when none is, so the overlay is only ever scaled down.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetimagelist
fn system_image_list(size: u32) -> Option<IImageList> {
    let mut largest: Option<(i32, IImageList)> = None;
    for kind in [SHIL_SMALL, SHIL_LARGE, SHIL_EXTRALARGE, SHIL_JUMBO] {
        // SAFETY: plain out-parameter calls on a live, shell-owned image list.
        let Some((width, list)) = (unsafe {
            SHGetImageList::<IImageList>(kind as i32)
                .ok()
                .and_then(|list| {
                    let (mut width, mut height) = (0, 0);
                    list.GetIconSize(&mut width, &mut height).ok()?;
                    Some((width, list))
                })
        }) else {
            continue;
        };
        if width >= size as i32 {
            return Some(list);
        }
        if largest.as_ref().is_none_or(|(best, _)| width > *best) {
            largest = Some((width, list));
        }
    }
    largest.map(|(_, list)| list)
}

/// `icon` scaled to `size` square by area averaging, which keeps a downscaled
/// glyph's thin outline instead of dropping rows as nearest-neighbor does.
/// Returns `icon` itself when it is already that size.
pub(crate) fn resample(icon: &Icon, size: u32) -> Icon {
    let (width, height) = (icon.width(), icon.height());
    if (width, height) == (size, size) || width == 0 || height == 0 {
        return icon.clone();
    }
    let source = icon.pixels();
    let (scale_x, scale_y) = (width as f32 / size as f32, height as f32 / size as f32);
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        let (top, bottom) = (y as f32 * scale_y, (y + 1) as f32 * scale_y);
        for x in 0..size {
            let (left, right) = (x as f32 * scale_x, (x + 1) as f32 * scale_x);
            let mut sum = [0f32; 4];
            let mut area = 0f32;
            for sy in top.floor() as u32..(bottom.ceil() as u32).min(height) {
                let cover_y = (bottom.min(sy as f32 + 1.0) - top.max(sy as f32)).max(0.0);
                for sx in left.floor() as u32..(right.ceil() as u32).min(width) {
                    let cover_x = (right.min(sx as f32 + 1.0) - left.max(sx as f32)).max(0.0);
                    let weight = cover_x * cover_y;
                    let from = ((sy * width + sx) * 4) as usize;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += source[from + channel] as f32 * weight;
                    }
                    area += weight;
                }
            }
            if area > 0.0 {
                let to = ((y * size + x) * 4) as usize;
                for (channel, total) in sum.iter().enumerate() {
                    out[to + channel] = (total / area).round().min(255.0) as u8;
                }
            }
        }
    }
    Icon::new(size, size, out.into_boxed_slice())
}

/// Draws `overlay` over `base`, source-over. Both are premultiplied BGRA and
/// the same size.
fn composite_overlay(base: &Icon, overlay: &Icon) -> Icon {
    let mut out = base.pixels().to_vec();
    for (destination, source) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(overlay.pixels().as_chunks::<4>().0.iter())
    {
        let alpha = source[3] as u16;
        if alpha == 0 {
            continue;
        }
        for channel in 0..4 {
            let under = destination[channel] as u16;
            destination[channel] =
                (source[channel] as u16 + (under * (255 - alpha) + 127) / 255).min(255) as u8;
        }
    }
    Icon::new(base.width(), base.height(), out.into_boxed_slice())
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
    fn a_report_waits_for_a_batch_unless_the_queue_is_dry() {
        let worker = Worker::new();
        let mut state = worker.lock();
        let soon = Duration::ZERO;
        assert!(should_report(&state, soon), "nothing else is coming");
        state.queue.set_wanted(&SPACE, &[key("next")]);
        assert!(!should_report(&state, soon));
        assert!(should_report(&state, LOADED_REPORT_INTERVAL));
        state.reported = true;
        assert!(!should_report(&state, LOADED_REPORT_INTERVAL));
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

    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Icon {
        Icon::new(1, 1, vec![r, g, b, a].into_boxed_slice())
    }

    #[test]
    fn an_opaque_overlay_replaces_the_base() {
        let base = pixel(255, 0, 0, 255);
        let overlay = pixel(0, 0, 255, 255);
        let combined = composite_overlay(&base, &overlay);
        assert_eq!(combined.pixels(), [0, 0, 255, 255]);
    }

    #[test]
    fn a_transparent_overlay_leaves_the_base() {
        let base = pixel(255, 0, 0, 255);
        let overlay = pixel(0, 0, 255, 0);
        let combined = composite_overlay(&base, &overlay);
        assert_eq!(combined.pixels(), [255, 0, 0, 255]);
    }

    #[test]
    fn a_half_transparent_overlay_blends() {
        let base = pixel(200, 0, 0, 255);
        let overlay = pixel(0, 0, 0, 128);
        let combined = composite_overlay(&base, &overlay);
        // out = 0 + 200 * (255 - 128)/255 = 99.6 -> 100 (premultiplied red).
        assert_eq!(combined.pixels()[0], 100);
        assert_eq!(combined.pixels()[3], 255);
    }

    #[test]
    fn resampling_to_the_same_size_keeps_the_pixels() {
        let pixels: Vec<u8> = (0..16).collect();
        let original = Icon::new(2, 2, pixels.clone().into_boxed_slice());
        assert_eq!(resample(&original, 2).pixels(), pixels.as_slice());
    }

    #[test]
    fn downscaling_averages_the_covered_pixels() {
        let quad = Icon::new(
            2,
            2,
            [
                [0, 0, 0, 0],
                [200, 0, 0, 255],
                [0, 0, 0, 0],
                [200, 0, 0, 255],
            ]
            .concat()
            .into_boxed_slice(),
        );
        let scaled = resample(&quad, 1);
        assert_eq!(scaled.pixels(), [100, 0, 0, 128]);
    }

    #[test]
    fn a_corner_glyph_stays_in_its_corner_when_downscaled() {
        let mut pixels = vec![0u8; 8 * 8 * 4];
        for y in 4..8 {
            for x in 0..4 {
                pixels[(y * 8 + x) * 4 + 3] = 255;
            }
        }
        let scaled = resample(&Icon::new(8, 8, pixels.into_boxed_slice()), 4);
        let alpha = |x: usize, y: usize| scaled.pixels()[(y * 4 + x) * 4 + 3];
        assert_eq!((alpha(0, 3), alpha(1, 2)), (255, 255));
        assert_eq!((alpha(2, 2), alpha(1, 1), alpha(3, 0)), (0, 0, 0));
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

    #[test]
    fn the_overlay_is_scaled_into_the_bottom_left_corner() {
        let mut pixels = vec![0u8; 8 * 8 * 4];
        for y in 4..8 {
            for x in 0..4 {
                pixels[(y * 8 + x) * 4 + 3] = 255;
            }
        }
        let overlay = Icon::new(8, 8, pixels.into_boxed_slice());
        let placed = scale_overlay_to_corner(&overlay, 8, 50);
        assert_eq!((placed.width(), placed.height()), (8, 8));
        let alpha = |x: usize, y: usize| placed.pixels()[(y * 8 + x) * 4 + 3];
        // The 4x4 glyph halves to 2x2, still flush to the bottom-left.
        let solid = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| alpha(x, y) > 0)
            .count();
        assert_eq!(solid, 4);
        assert!(alpha(0, 7) > 0 && alpha(1, 6) > 0);
        assert_eq!((alpha(0, 0), alpha(7, 0), alpha(7, 7)), (0, 0, 0));
    }
}
