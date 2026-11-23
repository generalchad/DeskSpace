//! Watches folders for changes off the UI thread, so the app re-reads one
//! when its contents change instead of polling.
//!
//! `ReadDirectoryChangesW` runs with overlapped I/O on one worker thread that
//! serves every watch through an I/O completion port. The worker also lists
//! each folder and builds a lens's finished item list (`LensBuilder`), so a
//! burst costs the UI thread only swapping the list in and a repaint. A burst
//! of changes (a copy, an extract, an Explorer move) is debounced per folder
//! into a single posted listing, so the app restyles once rather than once per
//! file. M2 uses this for the Desktop folder; M4 phase 2 uses it for every
//! lens's folder.
//! https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports

mod folder;
mod timing;
mod worker;

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use deskspace_core::{ItemId, LensBuilder, LensRules};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME,
};
use windows::Win32::System::IO::{CreateIoCompletionPort, PostQueuedCompletionStatus};

use timing::Bursts;
use worker::Worker;

use crate::error::PlatformError;
use crate::input::{FolderListing, WatchKey};

/// How long a change waits for the burst to settle before the app is woken.
/// Long enough to fold a multi-file copy into one wake, short enough that the
/// desktop layer still follows promptly.
const DEBOUNCE_MS: u64 = 150;

/// How often the worker tries a folder that is missing or cannot be read.
/// The app sets no timer for it; the worker owns the retry.
const RETRY_MS: u64 = 5_000;

/// How often the worker checks that an open watch's path still names a folder.
/// Renaming a watched folder moves its handle with it, so the pending read may
/// never fail and the lens would keep the old items. Tests wait for one check,
/// so they get a short interval instead of the production one.
const CHECK_MS: u64 = if cfg!(test) { 200 } else { 5_000 };

/// How long a stopping worker waits for its canceled reads to come back.
/// Canceling a change read completes at once; this only bounds a broken one,
/// and stays well under the console's ~5 s close deadline.
const DRAIN_MS: u64 = 1_000;

/// The change notifications the watch reports: files and folders added,
/// removed, or renamed, and attribute changes such as a hidden flag.
fn filter() -> FILE_NOTIFY_CHANGE {
    FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_ATTRIBUTES
}

/// The change-record buffer size. The records themselves are ignored (only the
/// arrival of a change matters), so this only has to be large enough that a
/// burst does not overflow into `ERROR_NOTIFY_ENUM_DIR`.
const BUFFER_BYTES: usize = 8 * 1024;

/// Completion keys for the worker's own packets. A folder's slot is its
/// completion key too, so slots start above these.
const ADD_KEY: usize = 0;
const REMOVE_KEY: usize = 1;
const STOP_KEY: usize = 2;
const UPDATE_KEY: usize = 3;
const FIRST_SLOT: usize = 4;

/// Folder reads also start above the control keys. A fresh key per open keeps
/// a canceled read's late packet from being taken for the next read's.
const FIRST_READ_KEY: usize = 4;

/// The listing a worker left for the UI thread, which takes it when the
/// posted change arrives.
type SharedListing = Arc<Mutex<Option<FolderListing>>>;

/// Moves `slot`'s listing from the UI's map to the worker's, so the worker
/// frees a big lens's list off the UI thread. A slot without a listing does
/// nothing.
fn retire_listing(
    listings: &mut HashMap<usize, SharedListing>,
    retired: &Mutex<HashMap<usize, SharedListing>>,
    slot: usize,
) {
    if let Some(listing) = listings.remove(&slot) {
        retired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(slot, listing);
    }
}

/// What the UI has told a lens's watch since the worker last looked, by slot.
/// The newest rules replace older ones, so a burst of edits (typing in a filter
/// field) builds the list once.
#[derive(Default)]
struct LensUpdate {
    rules: Option<LensRules>,
    items: Vec<(PathBuf, ItemId, Option<u64>)>,
}

/// The watches and the worker that serves them. Dropping it stops the worker
/// and joins it.
pub(crate) struct FolderWatch {
    port: HANDLE,
    thread: Option<JoinHandle<()>>,
    /// The paths and listings for add packets the worker has not handled yet,
    /// by slot.
    additions: Arc<Mutex<HashMap<usize, PendingWatch>>>,
    /// The lens updates the worker has not handled yet, by slot.
    updates: Arc<Mutex<HashMap<usize, LensUpdate>>>,
    /// Listings the UI stopped watching and handed to the worker, so a big
    /// lens's list is freed on the watch thread, not the UI thread.
    retired: Arc<Mutex<HashMap<usize, SharedListing>>>,
    /// The slot and path behind each app-level key.
    active: HashMap<WatchKey, ActiveWatch>,
    /// The app-level key behind each slot, to decode a posted change.
    slots: HashMap<usize, WatchKey>,
    /// The listing slot the worker fills before posting a change.
    listings: HashMap<usize, SharedListing>,
    next_slot: usize,
    port_closed: bool,
}

/// A watch the UI has added but the worker has not opened yet.
struct PendingWatch {
    path: PathBuf,
    store: PathBuf,
    root: Option<PathBuf>,
    listing: SharedListing,
    builder: Option<LensBuilder>,
}

/// One key's watch, as the UI thread remembers it.
struct ActiveWatch {
    slot: usize,
    path: PathBuf,
    /// The rules last sent to the worker, so unchanged ones build nothing.
    /// `None` for a watch that builds no list.
    rules: Option<LensRules>,
}

impl FolderWatch {
    /// Starts the worker that serves every folder watch. Each change posts
    /// `message` with the folder's slot in `wParam` to `target`.
    pub(crate) fn start(target: HWND, message: u32) -> Result<Self, PlatformError> {
        // SAFETY: creates a completion port with no file handle; it is closed
        // in `stop`, after the worker that waits on it has finished.
        let port = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 0) }?;
        let additions = Arc::new(Mutex::new(HashMap::new()));
        let pending = Arc::clone(&additions);
        let updates = Arc::new(Mutex::new(HashMap::new()));
        let pending_updates = Arc::clone(&updates);
        let retired = Arc::new(Mutex::new(HashMap::new()));
        let worker_retired = Arc::clone(&retired);
        // A raw `HWND`/`HANDLE` is not `Send`, so carry them as `isize` and
        // rebuild them on the worker, which only ever posts to the window.
        let target_raw = target.0 as isize;
        let port_raw = port.0 as isize;
        let spawned = std::thread::Builder::new()
            .name("deskspace-folder-watch".into())
            .spawn(move || {
                let worker = Worker {
                    port: HANDLE(port_raw as *mut c_void),
                    target: HWND(target_raw as *mut c_void),
                    message,
                    additions: pending,
                    updates: pending_updates,
                    retired: worker_retired,
                    folders: HashMap::new(),
                    by_read: HashMap::new(),
                    canceled: HashMap::new(),
                    next_read_key: FIRST_READ_KEY,
                    bursts: Bursts::default(),
                    start: Instant::now(),
                    next_check: CHECK_MS,
                };
                if let Err(error) = worker.run() {
                    tracing::debug!(%error, "folder watch stopped");
                }
            });
        match spawned {
            Ok(thread) => Ok(Self {
                port,
                thread: Some(thread),
                additions,
                updates,
                retired,
                active: HashMap::new(),
                slots: HashMap::new(),
                listings: HashMap::new(),
                next_slot: FIRST_SLOT,
                port_closed: false,
            }),
            Err(error) => {
                // SAFETY: `port` was created above and no worker uses it yet.
                unsafe {
                    let _ = CloseHandle(port);
                }
                Err(error.into())
            }
        }
    }

    /// Whether `key` watches `path` now.
    pub(crate) fn is_watching(&self, key: WatchKey, path: &Path) -> bool {
        self.active
            .get(&key)
            .is_some_and(|active| active.path == path)
    }

    /// Watches `path` for `key`, refusing it when it resolves to `store` or
    /// inside it. `root` is the lens's own folder when `path` is a subfolder
    /// below it: its handle is opened there and reads below it, while `path` is
    /// what gets listed, so renaming that folder stays possible. `builder`
    /// turns a lens's listings into its item list; a watch without one posts
    /// empty lists. Does nothing when that key already watches the same path;
    /// another path replaces the key's watch.
    pub(crate) fn watch(
        &mut self,
        key: WatchKey,
        path: &Path,
        store: &Path,
        root: Option<&Path>,
        builder: Option<LensBuilder>,
    ) -> Result<(), PlatformError> {
        if self.is_watching(key, path) {
            return Ok(());
        }
        self.unwatch(key);
        let slot = self.next_slot;
        self.next_slot += 1;
        let listing: SharedListing = Arc::new(Mutex::new(None));
        self.active.insert(
            key,
            ActiveWatch {
                slot,
                path: path.to_path_buf(),
                rules: builder.as_ref().map(|builder| builder.rules().clone()),
            },
        );
        self.slots.insert(slot, key);
        self.listings.insert(slot, Arc::clone(&listing));
        self.additions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                slot,
                PendingWatch {
                    path: path.to_path_buf(),
                    store: store.to_path_buf(),
                    root: root.map(Path::to_path_buf),
                    listing,
                    builder,
                },
            );
        if let Err(error) = self.post(ADD_KEY, slot) {
            self.forget(key, slot);
            return Err(error.into());
        }
        Ok(())
    }

    /// Has the worker build `key`'s list under `rules`, without reading the
    /// folder. Rules the worker already has send nothing.
    pub(crate) fn set_lens_rules(&mut self, key: WatchKey, rules: LensRules) {
        let Some(active) = self.active.get_mut(&key) else {
            return;
        };
        if active.rules.as_ref() == Some(&rules) {
            return;
        }
        active.rules = Some(rules.clone());
        let slot = active.slot;
        self.send_update(slot, |update| update.rules = Some(rules));
    }

    /// Tells `key`'s worker that item `id` now has `path`, so the next listing
    /// keeps its id.
    pub(crate) fn note_lens_item(
        &self,
        key: WatchKey,
        path: PathBuf,
        id: ItemId,
        modified: Option<u64>,
    ) {
        if let Some(active) = self.active.get(&key) {
            self.send_update(active.slot, |update| {
                update.items.push((path, id, modified));
            });
        }
    }

    fn send_update(&self, slot: usize, change: impl FnOnce(&mut LensUpdate)) {
        change(
            self.updates
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(slot)
                .or_default(),
        );
        if let Err(error) = self.post(UPDATE_KEY, slot) {
            tracing::warn!(%error, "could not update a lens's watch");
        }
    }

    /// Stops the watch for `key`, if it has one.
    pub(crate) fn unwatch(&mut self, key: WatchKey) {
        let Some(active) = self.active.remove(&key) else {
            return;
        };
        self.slots.remove(&active.slot);
        retire_listing(&mut self.listings, &self.retired, active.slot);
        self.updates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&active.slot);
        if let Err(error) = self.post(REMOVE_KEY, active.slot) {
            tracing::warn!(%error, "could not remove a folder watch");
        }
    }

    /// The key, folder, and listing behind `slot`, taking the listing the
    /// worker left. A slot watches one path for its whole life, so the path is
    /// the folder the worker listed.
    pub(crate) fn change_for_slot(
        &self,
        slot: usize,
    ) -> Option<(WatchKey, PathBuf, FolderListing)> {
        let key = *self.slots.get(&slot)?;
        let folder = self
            .active
            .get(&key)
            .filter(|active| active.slot == slot)?
            .path
            .clone();
        let listing = self
            .listings
            .get(&slot)?
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        Some((key, folder, listing))
    }

    /// The keys the app is watching now.
    pub(crate) fn watched_keys(&self) -> Vec<WatchKey> {
        self.active.keys().copied().collect()
    }

    /// Forgets an add that never reached the worker.
    fn forget(&mut self, key: WatchKey, slot: usize) {
        self.active.remove(&key);
        self.slots.remove(&slot);
        self.listings.remove(&slot);
        self.additions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
    }

    /// Posts a control packet. The slot travels in the packet's byte count,
    /// because the packet itself carries only numbers.
    fn post(&self, key: usize, slot: usize) -> windows::core::Result<()> {
        // SAFETY: the port is live until `stop` closes it, after the worker has
        // finished; the packet carries no pointer.
        unsafe { PostQueuedCompletionStatus(self.port, slot as u32, key, None) }
    }

    /// Stops the worker and waits for it to finish. Idempotent.
    pub(crate) fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            if self.post(STOP_KEY, 0).is_err() {
                // Closing the port makes the worker's wait fail, so `join`
                // returns even when the stop packet could not be queued.
                // SAFETY: the worker is the only other user of the port.
                unsafe {
                    let _ = CloseHandle(self.port);
                }
                self.port_closed = true;
            }
            let _ = thread.join();
        }
        if !self.port_closed {
            // SAFETY: closed once, after the worker that waited on it finished.
            unsafe {
                let _ = CloseHandle(self.port);
            }
            self.port_closed = true;
        }
    }
}

impl Drop for FolderWatch {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_watch_hands_its_listing_to_the_worker() {
        let mut listings = HashMap::new();
        let listing: SharedListing = Arc::new(Mutex::new(None));
        listings.insert(4, Arc::clone(&listing));
        let retired = Mutex::new(HashMap::new());
        retire_listing(&mut listings, &retired, 4);
        assert!(listings.is_empty(), "the UI no longer holds the list");
        let handed = retired.lock().unwrap().remove(&4).expect("handed over");
        assert!(Arc::ptr_eq(&handed, &listing));
    }

    #[test]
    fn retiring_a_slot_without_a_listing_does_nothing() {
        let mut listings = HashMap::new();
        let retired = Mutex::new(HashMap::new());
        retire_listing(&mut listings, &retired, 9);
        assert!(retired.lock().unwrap().is_empty());
    }

    /// A store path no test folder is inside.
    pub(super) fn no_store() -> PathBuf {
        std::env::temp_dir().join("deskspace-watch-no-store")
    }

    /// A fresh, empty folder under the system temp folder, unique to `name`
    /// and this process.
    pub(super) fn scratch_folder(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("deskspace-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    /// Waits up to 5 s for the worker's first change on `slot`.
    pub(super) fn first_change(
        watch: &FolderWatch,
        slot: usize,
    ) -> Option<(WatchKey, PathBuf, FolderListing)> {
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(change) = watch.change_for_slot(slot) {
                return Some(change);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    /// Waits up to 5 s for the worker's first listing of `slot`.
    pub(super) fn first_listing(watch: &FolderWatch, slot: usize) -> Option<FolderListing> {
        first_change(watch, slot).map(|(_, _, listing)| listing)
    }

    /// Waits up to `millis` for a listing of `slot`, for a test that expects
    /// none.
    pub(super) fn first_change_within(
        watch: &FolderWatch,
        slot: usize,
        millis: u64,
    ) -> Option<FolderListing> {
        let deadline = Instant::now() + std::time::Duration::from_millis(millis);
        while Instant::now() < deadline {
            if let Some((_, _, listing)) = watch.change_for_slot(slot) {
                return Some(listing);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    /// Waits up to 8 s, past one path check, for a change on `slot` that is
    /// not a listing.
    pub(super) fn first_non_listing(watch: &FolderWatch, slot: usize) -> Option<FolderListing> {
        let deadline = Instant::now() + std::time::Duration::from_secs(8);
        while Instant::now() < deadline {
            match watch.change_for_slot(slot) {
                Some((_, _, FolderListing::Listed(_))) | None => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Some((_, _, listing)) => return Some(listing),
            }
        }
        None
    }

    /// Runs `body` on a thread and reports whether it finished within 10 s,
    /// so a hung worker fails the test instead of stalling it.
    pub(super) fn finishes(body: impl FnOnce() + Send + 'static) -> bool {
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            body();
            let _ = done.send(());
        });
        finished
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_ok()
    }

    /// A change names the folder its slot watches, so the app can drop a
    /// listing that waited in the queue while the key moved to another folder.
    #[test]
    fn a_change_names_the_folder_it_listed() {
        let folder = scratch_folder("names");
        let path = folder.clone();
        let named = std::sync::Arc::new(Mutex::new(None));
        let seen = std::sync::Arc::clone(&named);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &path, &no_store(), None, None)
                .unwrap();
            *seen.lock().unwrap() =
                first_change(&watch, FIRST_SLOT).map(|(key, folder, _)| (key, folder));
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&folder);
        assert!(finished, "the watch did not finish");
        assert_eq!(
            named.lock().unwrap().take(),
            Some((WatchKey::Desktop, folder))
        );
    }

    /// Makes `link` a junction to `target`. `mklink /J` needs no admin rights.
    fn junction(link: &Path, target: &Path) {
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J failed: {made:?}");
    }

    /// A scratch folder holding `storage\inner` and `link`, a junction into
    /// it, with the store's final path.
    pub(super) fn store_with_junction(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let scratch = scratch_folder(name);
        let store = scratch.join("storage");
        std::fs::create_dir_all(store.join("inner")).unwrap();
        let link = scratch.join("link");
        junction(&link, &store.join("inner"));
        let final_store = std::fs::canonicalize(&store).unwrap();
        (scratch, link, final_store)
    }
}
