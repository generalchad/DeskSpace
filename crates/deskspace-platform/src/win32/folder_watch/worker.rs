//! The folder-watch worker thread and its completion-port loop.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::IO::{GetQueuedCompletionStatus, OVERLAPPED};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::folder::{CanceledRead, Folder, OpenFailure, is_timeout};
use super::timing::{Bursts, millis};
use super::{
    ADD_KEY, CHECK_MS, DRAIN_MS, LensUpdate, PendingWatch, REMOVE_KEY, RETRY_MS, STOP_KEY,
    SharedListing, UPDATE_KEY,
};
use crate::error::PlatformError;
use crate::input::FolderListing;

/// The worker: one completion port for every folder, a debounce clock per
/// folder, and a retry clock for folders that are missing.
pub(super) struct Worker {
    pub(super) port: HANDLE,
    pub(super) target: HWND,
    pub(super) message: u32,
    pub(super) additions: Arc<Mutex<HashMap<usize, PendingWatch>>>,
    pub(super) updates: Arc<Mutex<HashMap<usize, LensUpdate>>>,
    pub(super) retired: Arc<Mutex<HashMap<usize, SharedListing>>>,
    pub(super) folders: HashMap<usize, Folder>,
    /// The slot behind each open read's completion key.
    pub(super) by_read: HashMap<usize, usize>,
    /// Canceled reads by completion key, kept until their packets arrive.
    pub(super) canceled: HashMap<usize, CanceledRead>,
    pub(super) next_read_key: usize,
    pub(super) bursts: Bursts,
    pub(super) start: Instant,
    /// When an open watch's path is checked again, in milliseconds since
    /// `start`.
    pub(super) next_check: u64,
}

impl Worker {
    pub(super) fn run(mut self) -> Result<(), PlatformError> {
        let outcome = self.watch();
        self.close_all();
        outcome
    }

    fn watch(&mut self) -> Result<(), PlatformError> {
        loop {
            let now = millis(self.start);
            let timeout = self
                .bursts
                .wait_ms(now)
                .min(self.retry_wait(now))
                .min(self.check_wait(now));
            let mut transferred = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
            // SAFETY: `port` is live for the whole loop; the worker never
            // dereferences the returned `overlapped` pointer.
            let waited = unsafe {
                GetQueuedCompletionStatus(
                    self.port,
                    &mut transferred,
                    &mut key,
                    &mut overlapped,
                    timeout,
                )
            };
            let now = millis(self.start);
            match waited {
                Ok(()) if key == ADD_KEY => self.add(transferred as usize),
                Ok(()) if key == REMOVE_KEY => self.remove(transferred as usize),
                Ok(()) if key == STOP_KEY => return Ok(()),
                Ok(()) if key == UPDATE_KEY => self.update(transferred as usize),
                // Any other key is a folder's read, registered with the port
                // when the read's folder was opened. A read that failed or
                // was canceled still sends its packet, as an error with the
                // `OVERLAPPED` set.
                Ok(()) => self.completed(key, Ok(()), now),
                Err(error) if !overlapped.is_null() => self.completed(key, Err(error), now),
                Err(error) if is_timeout(&error) => {}
                Err(error) => return Err(error.into()),
            }
            self.post_due(now);
            self.retry_due(now);
            self.check_paths(now);
        }
    }

    /// Opens a newly added folder and posts its first listing.
    fn add(&mut self, slot: usize) {
        let pending = self
            .additions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
        let Some(PendingWatch {
            path,
            store,
            root,
            listing,
            builder,
        }) = pending
        else {
            tracing::warn!(slot, "folder watch add had no path");
            return;
        };
        let mut folder = Folder {
            path,
            store,
            root,
            listing,
            builder,
            open: None,
            retry_at: None,
        };
        let now = millis(self.start);
        let read_key = self.next_read_key;
        self.next_read_key += 1;
        let listing = match folder.open(self.port, read_key) {
            Ok(()) => {
                self.by_read.insert(read_key, slot);
                let listing = folder.list();
                folder.prefer_moved(listing)
            }
            Err(OpenFailure::Refused(resolved)) => {
                tracing::warn!(
                    path = %folder.path.display(),
                    resolved = %resolved.display(),
                    "the folder is DeskSpace's storage or inside it; not watching it"
                );
                FolderListing::Refused
            }
            Err(OpenFailure::Failed(error)) => {
                tracing::warn!(
                    %error,
                    path = %folder.path.display(),
                    "could not open the folder; retrying"
                );
                FolderListing::Unavailable
            }
        };
        let listed = matches!(listing, FolderListing::Listed(_));
        folder.retry_at = (!listed).then_some(now + RETRY_MS);
        self.folders.insert(slot, folder);
        if !listed {
            // A listing that failed leaves the old handle watching a folder
            // that may be gone; reopen it on the retry to follow a rename back.
            self.close_open(slot);
        }
        self.bursts.add(slot);
        self.publish(slot, listing);
    }

    /// Takes what the UI told a lens's watch, and posts the list again when
    /// its rules changed. An earlier packet may already have taken it all.
    fn update(&mut self, slot: usize) {
        let update = self
            .updates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
        let Some(update) = update else {
            return;
        };
        let Some(builder) = self
            .folders
            .get_mut(&slot)
            .and_then(|folder| folder.builder.as_mut())
        else {
            return;
        };
        for (path, id, modified) in update.items {
            builder.note_item(path, id, modified);
        }
        if let Some(list) = update.rules.and_then(|rules| builder.set_rules(rules)) {
            self.publish(slot, FolderListing::Listed(list));
        }
    }

    /// Stops one folder's watch.
    fn remove(&mut self, slot: usize) {
        self.bursts.remove(slot);
        self.close_open(slot);
        self.folders.remove(&slot);
        self.retired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
    }

    /// Closes `slot`'s handle, if it is open, and cancels its pending read.
    /// The canceled read is kept until its packet arrives, because Windows
    /// writes into its buffer and `OVERLAPPED` until then.
    fn close_open(&mut self, slot: usize) {
        let Some(open) = self
            .folders
            .get_mut(&slot)
            .and_then(|folder| folder.open.take())
        else {
            return;
        };
        let read_key = open.read_key;
        self.by_read.remove(&read_key);
        if let Some(read) = open.close() {
            self.canceled.insert(read_key, read);
        }
    }

    /// One folder's read finished: start the next read and extend its burst.
    /// A failed read means the folder is gone; empty the lens and retry. A
    /// canceled read's packet only frees the read.
    fn completed(&mut self, read_key: usize, read: windows::core::Result<()>, now_ms: u64) {
        if self.canceled.remove(&read_key).is_some() {
            return;
        }
        let Some(&slot) = self.by_read.get(&read_key) else {
            return;
        };
        let failed = {
            let Some(folder) = self.folders.get_mut(&slot) else {
                return;
            };
            let Some(open) = folder.open.as_mut() else {
                return;
            };
            if !open.pending {
                return;
            }
            open.pending = false;
            match read
                .map_err(PlatformError::from)
                .and_then(|()| open.read_changes())
            {
                Ok(()) => false,
                Err(error) => {
                    tracing::debug!(
                        %error,
                        path = %folder.path.display(),
                        "folder watch stopped for this folder"
                    );
                    true
                }
            }
        };
        if failed {
            let listing = self
                .folders
                .get(&slot)
                .map(|folder| folder.prefer_moved(FolderListing::Unavailable))
                .unwrap_or(FolderListing::Unavailable);
            self.close_open(slot);
            if let Some(folder) = self.folders.get_mut(&slot) {
                folder.retry_at = Some(now_ms + RETRY_MS);
            }
            self.bursts.remove(slot);
            self.publish(slot, listing);
        } else {
            self.bursts.mark(slot, now_ms);
        }
    }

    /// Lists every folder whose burst is due and posts the result.
    fn post_due(&mut self, now_ms: u64) {
        for slot in self.bursts.take_due(now_ms) {
            let Some(folder) = self.folders.get_mut(&slot) else {
                continue;
            };
            let listing = folder.list();
            let listing = folder.prefer_moved(listing);
            let listed = matches!(listing, FolderListing::Listed(_));
            folder.retry_at = (!listed).then_some(now_ms + RETRY_MS);
            if !listed {
                self.close_open(slot);
            }
            self.publish(slot, listing);
        }
    }

    /// Tries every missing folder whose retry is due, and posts a listing when
    /// it comes back.
    fn retry_due(&mut self, now_ms: u64) {
        let due: Vec<usize> = self
            .folders
            .iter()
            .filter(|(_, folder)| folder.retry_at.is_some_and(|at| at <= now_ms))
            .map(|(slot, _)| *slot)
            .collect();
        for slot in due {
            let needs_open = self
                .folders
                .get(&slot)
                .is_some_and(|folder| folder.open.is_none());
            let mut opened = None;
            if needs_open {
                let read_key = self.next_read_key;
                self.next_read_key += 1;
                let result = match self.folders.get_mut(&slot) {
                    Some(folder) => folder.open(self.port, read_key),
                    None => continue,
                };
                match result {
                    Ok(()) => opened = Some(read_key),
                    // A refused folder stays unavailable to the app, so a
                    // retry that is still refused posts nothing.
                    Err(failure) => {
                        let folder = self.folders.get_mut(&slot);
                        if let Some(folder) = folder {
                            tracing::debug!(
                                ?failure,
                                path = %folder.path.display(),
                                "folder is still unavailable"
                            );
                            folder.retry_at = Some(now_ms + RETRY_MS);
                        }
                        continue;
                    }
                }
            }
            if let Some(key) = opened {
                self.by_read.insert(key, slot);
            }
            let recovered = {
                let Some(folder) = self.folders.get_mut(&slot) else {
                    continue;
                };
                let listing = folder.list();
                let listing = folder.prefer_moved(listing);
                match listing {
                    FolderListing::Listed(list) => {
                        folder.retry_at = None;
                        Some(FolderListing::Listed(list))
                    }
                    listing @ FolderListing::Moved { .. } => {
                        folder.retry_at = Some(now_ms + RETRY_MS);
                        Some(listing)
                    }
                    FolderListing::Unavailable | FolderListing::Refused => {
                        folder.retry_at = Some(now_ms + RETRY_MS);
                        None
                    }
                }
            };
            match recovered {
                Some(listing) => self.publish(slot, listing),
                None => self.close_open(slot),
            }
        }
    }

    /// How long until the next missing folder is tried.
    fn retry_wait(&self, now_ms: u64) -> u32 {
        self.folders
            .values()
            .filter_map(|folder| folder.retry_at)
            .map(|at| at.saturating_sub(now_ms).min(u64::from(u32::MAX)) as u32)
            .min()
            .unwrap_or(INFINITE)
    }

    /// How long until the open watches' paths are checked again.
    fn check_wait(&self, now_ms: u64) -> u32 {
        if self.folders.is_empty() {
            return INFINITE;
        }
        self.next_check
            .saturating_sub(now_ms)
            .min(u64::from(u32::MAX)) as u32
    }

    /// A watched folder renamed away keeps reporting for the moved directory,
    /// so the pending read may stay open forever. Check that the path still
    /// names a folder outside the store, and empty the lens when it does not.
    fn check_paths(&mut self, now_ms: u64) {
        if now_ms < self.next_check {
            return;
        }
        self.next_check = now_ms + CHECK_MS;
        let gone: Vec<(usize, FolderListing)> = self
            .folders
            .iter()
            .filter(|(_, folder)| folder.open.is_some())
            .filter_map(|(slot, folder)| folder.moved().map(|listing| (*slot, listing)))
            .collect();
        for (slot, listing) in gone {
            let path = self
                .folders
                .get(&slot)
                .map(|folder| folder.watch_path().to_path_buf());
            if let Some(path) = path {
                tracing::debug!(path = %path.display(), ?listing, "watched folder moved or disappeared");
            }
            self.close_open(slot);
            if let Some(folder) = self.folders.get_mut(&slot) {
                folder.retry_at = Some(now_ms + RETRY_MS);
            }
            self.bursts.remove(slot);
            self.publish(slot, listing);
        }
    }

    /// Leaves `listing` for the UI thread and wakes it with the folder's slot.
    /// A list the UI has not taken yet is replaced and freed here, off its
    /// thread.
    fn publish(&mut self, slot: usize, listing: FolderListing) {
        let Some(folder) = self.folders.get_mut(&slot) else {
            return;
        };
        if !matches!(listing, FolderListing::Listed(_))
            && let Some(builder) = folder.builder.as_mut()
        {
            builder.folder_gone();
        }
        *folder
            .listing
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(listing);
        // SAFETY: `target` is the controller window on the UI thread; a post
        // to a destroyed window just fails.
        unsafe {
            let _ = PostMessageW(Some(self.target), self.message, WPARAM(slot), LPARAM(0));
        }
    }

    /// Stops every folder's read before the worker exits.
    fn close_all(&mut self) {
        self.bursts.clear();
        let slots: Vec<usize> = self.folders.keys().copied().collect();
        for slot in slots {
            self.close_open(slot);
        }
        self.folders.clear();
        self.retired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.drain_canceled();
    }

    /// Takes the canceled reads' packets off the port, so no read outlives
    /// its buffer. A cancel completes at once, but this waits at most
    /// `DRAIN_MS`; a read still out after that is leaked, never freed.
    fn drain_canceled(&mut self) {
        let start = Instant::now();
        while !self.canceled.is_empty() {
            let left = DRAIN_MS.saturating_sub(millis(start));
            if left == 0 {
                break;
            }
            let mut transferred = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
            // SAFETY: `port` is live until `FolderWatch::stop` closes it after
            // this thread ends; the returned `overlapped` is not dereferenced.
            let waited = unsafe {
                GetQueuedCompletionStatus(
                    self.port,
                    &mut transferred,
                    &mut key,
                    &mut overlapped,
                    left as u32,
                )
            };
            // A failed wait with no packet is a timeout or a closed port.
            if waited.is_err() && overlapped.is_null() {
                break;
            }
            self.canceled.remove(&key);
        }
        if !self.canceled.is_empty() {
            tracing::warn!(
                reads = self.canceled.len(),
                "folder watch reads did not finish; leaking their buffers"
            );
            for (_, read) in self.canceled.drain() {
                std::mem::forget(read);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use deskspace_core::{LensBuilder, LensRules};

    use super::super::tests::{
        finishes, first_change_within, first_listing, first_non_listing, no_store, scratch_folder,
        store_with_junction,
    };
    use super::super::{FIRST_SLOT, FolderWatch};
    use super::*;
    use crate::input::WatchKey;
    use crate::win32::names::plain_path;

    /// A watched folder renamed in place reports its new path, so a lens can
    /// follow it (ROADMAP D19).
    #[test]
    fn a_renamed_folder_reports_where_it_went() {
        let parent = scratch_folder("rename");
        let (old, new) = (parent.join("old"), parent.join("new"));
        std::fs::create_dir(&old).unwrap();
        let (watched, renamed) = (old.clone(), new.clone());
        let seen = Arc::new(Mutex::new(None));
        let result = Arc::clone(&seen);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &watched, &no_store(), None, None)
                .unwrap();
            assert!(first_listing(&watch, FIRST_SLOT).is_some());
            std::fs::rename(&watched, &renamed).unwrap();
            *result.lock().unwrap() = first_non_listing(&watch, FIRST_SLOT);
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&parent);
        assert!(finished, "the watch did not finish");
        let seen = seen.lock().unwrap().take();
        let Some(FolderListing::Moved { to, .. }) = seen else {
            panic!("expected Moved, got {seen:?}");
        };
        // The temp folder may be spelled with a short name, and the final
        // path never is, so compare the folder's own name.
        assert_eq!(to.file_name(), new.file_name(), "moved to {to:?}");
        assert_eq!(plain_path(&to), to, "the path reads plain");
    }

    /// A browsed lens watches its own folder, so renaming that folder works
    /// and the watch reports the new path for the lens to follow (ROADMAP
    /// D19).
    #[test]
    fn a_browsed_lens_reports_where_its_own_folder_went() {
        let parent = scratch_folder("browsed-rename");
        let root = parent.join("root");
        let shown = root.join("A").join("B");
        std::fs::create_dir_all(&shown).unwrap();
        let watched = shown.clone();
        let watched_root = root.clone();
        let renamed_root = parent.join("new-root");
        let seen = Arc::new(Mutex::new(None));
        let result = Arc::clone(&seen);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(
                    WatchKey::Desktop,
                    &watched,
                    &no_store(),
                    Some(&watched_root),
                    None,
                )
                .unwrap();
            assert!(first_listing(&watch, FIRST_SLOT).is_some());
            // The handle is on `root`, so this rename is not blocked by the
            // subfolder it lists.
            std::fs::rename(&watched_root, &renamed_root).unwrap();
            *result.lock().unwrap() = first_non_listing(&watch, FIRST_SLOT);
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&parent);
        assert!(finished, "the watch did not finish");
        let seen = seen.lock().unwrap().take();
        let Some(FolderListing::Moved { to }) = seen else {
            panic!("expected Moved, got {seen:?}");
        };
        // The temp folder may be spelled with a short name, and the final
        // path never is, so compare the folder's own name.
        assert_eq!(to.file_name().and_then(OsStr::to_str), Some("new-root"));
    }

    /// A browsed lens sees a file added to the subfolder it shows, though its
    /// handle is on the folder above it.
    #[test]
    fn a_browsed_lens_sees_changes_in_the_subfolder_it_shows() {
        let root = scratch_folder("browsed-subtree");
        let shown = root.join("A").join("B");
        std::fs::create_dir_all(&shown).unwrap();
        let watched = shown.clone();
        let watched_root = root.clone();
        let added = shown.join("new.txt");
        let counts = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&counts);
        let finished = finishes(move || {
            let builder = LensBuilder::new(
                LensRules::default(),
                deskspace_core::Desktop::new().reserve_item_ids(),
                &[],
            );
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(
                    WatchKey::Space(deskspace_core::SpaceId(7)),
                    &watched,
                    &no_store(),
                    Some(&watched_root),
                    Some(builder),
                )
                .unwrap();
            let before = first_listing(&watch, FIRST_SLOT);
            std::fs::write(&added, "").unwrap();
            let after = first_listing(&watch, FIRST_SLOT);
            let count = |listing: Option<FolderListing>| match listing {
                Some(FolderListing::Listed(list)) => list.items.len(),
                _ => usize::MAX,
            };
            *seen.lock().unwrap() = vec![count(before), count(after)];
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&root);
        assert!(finished, "the watch did not finish");
        assert_eq!(
            std::mem::take(&mut *counts.lock().unwrap()),
            vec![0, 1],
            "the subfolder's new file was listed"
        );
    }

    /// A change burst that finds the subfolder gone because the lens's own
    /// folder moved reports the move, not a vanished subfolder, so the app
    /// follows it even when the two signals race.
    #[test]
    fn a_burst_that_saw_the_root_move_reports_the_move() {
        let parent = scratch_folder("browsed-race");
        let root = parent.join("root");
        let shown = root.join("A").join("B");
        std::fs::create_dir_all(&shown).unwrap();
        std::fs::write(shown.join("old.txt"), "").unwrap();
        let watched = shown.clone();
        let watched_root = root.clone();
        let renamed_root = parent.join("new-root");
        let seen = Arc::new(Mutex::new(None));
        let result = Arc::clone(&seen);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(
                    WatchKey::Desktop,
                    &watched,
                    &no_store(),
                    Some(&watched_root),
                    None,
                )
                .unwrap();
            assert!(first_listing(&watch, FIRST_SLOT).is_some());
            // A change under the root arms a burst; the rename lands before it
            // lists, so the burst itself has to notice the move.
            std::fs::write(watched.join("new.txt"), "").unwrap();
            std::fs::rename(&watched_root, &renamed_root).unwrap();
            *result.lock().unwrap() = first_non_listing(&watch, FIRST_SLOT);
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&parent);
        assert!(finished, "the watch did not finish");
        let seen = seen.lock().unwrap().take();
        let Some(FolderListing::Moved { to }) = seen else {
            panic!("expected Moved, got {seen:?}");
        };
        assert_eq!(to.file_name().and_then(OsStr::to_str), Some("new-root"));
    }

    /// A subfolder that is a junction into the store is refused while the
    /// lens's own folder is still watched.
    #[test]
    fn a_browsed_lens_refuses_a_subfolder_that_leads_into_the_store() {
        let (scratch, link, final_store) = store_with_junction("browsed-junction");
        let root = scratch.clone();
        let listed = Arc::new(Mutex::new(None));
        let seen = Arc::clone(&listed);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &link, &final_store, Some(&root), None)
                .unwrap();
            *seen.lock().unwrap() = first_listing(&watch, FIRST_SLOT);
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(finished, "the watch did not finish");
        assert_eq!(listed.lock().unwrap().take(), Some(FolderListing::Refused));
    }

    /// A quiet folder's read is still out when the watch stops. Waiting for
    /// that read used to hang the worker, and with it the app's exit.
    #[test]
    fn stopping_with_a_quiet_folder_watched_returns() {
        let folder = scratch_folder("stop");
        let path = folder.clone();
        let stopped = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &path, &no_store(), None, None)
                .unwrap();
            assert!(first_listing(&watch, FIRST_SLOT).is_some());
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&folder);
        assert!(stopped, "stopping the watch did not return");
    }

    /// Pointing a key at another folder (Change folder…) cancels the old
    /// folder's read, and the worker goes on to serve the new one.
    #[test]
    fn replacing_a_watch_keeps_the_worker_serving() {
        let old = scratch_folder("replace-old");
        let new = scratch_folder("replace-new");
        let (old_path, new_path) = (old.clone(), new.clone());
        let served = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &old_path, &no_store(), None, None)
                .unwrap();
            assert!(first_listing(&watch, FIRST_SLOT).is_some());
            watch
                .watch(WatchKey::Desktop, &new_path, &no_store(), None, None)
                .unwrap();
            assert!(matches!(
                first_listing(&watch, FIRST_SLOT + 1),
                Some(FolderListing::Listed(_))
            ));
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
        assert!(
            served,
            "the worker stopped serving after a watch was replaced"
        );
    }

    /// New rules build the list again from the last listing, and the same
    /// rules again build nothing.
    #[test]
    fn new_rules_rebuild_the_list_from_the_last_listing() {
        let folder = scratch_folder("rules");
        std::fs::write(folder.join("a.txt"), "").unwrap();
        std::fs::write(folder.join("b.pdf"), "").unwrap();
        let path = folder.clone();
        let lists = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&lists);
        let finished = finishes(move || {
            let key = WatchKey::Space(deskspace_core::SpaceId(7));
            let builder = LensBuilder::new(
                LensRules::default(),
                deskspace_core::Desktop::new().reserve_item_ids(),
                &[],
            );
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(key, &path, &no_store(), None, Some(builder))
                .unwrap();
            let first = first_listing(&watch, FIRST_SLOT);
            let rules = LensRules {
                include: vec!["*.pdf".into()],
                ..LensRules::default()
            };
            watch.set_lens_rules(key, rules.clone());
            let second = first_listing(&watch, FIRST_SLOT);
            watch.set_lens_rules(key, rules);
            let third = first_change_within(&watch, FIRST_SLOT, 300);
            watch.stop();
            *seen.lock().unwrap() = vec![first, second, third];
        });
        let _ = std::fs::remove_dir_all(&folder);
        assert!(finished, "the watch did not finish");
        let lists = lists.lock().unwrap();
        let labels = |listing: &FolderListing| match listing {
            FolderListing::Listed(list) => list
                .items
                .iter()
                .map(|item| item.label.clone())
                .collect::<Vec<_>>(),
            other => panic!("expected a list, got {other:?}"),
        };
        assert_eq!(labels(lists[0].as_ref().unwrap()), ["a", "b"]);
        assert_eq!(labels(lists[1].as_ref().unwrap()), ["b"]);
        assert_eq!(lists[2], None, "unchanged rules post nothing");
    }

    /// A junction into the store is refused by its final path, though the
    /// path the lens names is outside the store.
    #[test]
    fn a_junction_into_the_store_is_refused() {
        let (scratch, link, final_store) = store_with_junction("junction");
        let listed = Arc::new(Mutex::new(None));
        let seen = Arc::clone(&listed);
        let finished = finishes(move || {
            let mut watch = FolderWatch::start(HWND::default(), 0).unwrap();
            watch
                .watch(WatchKey::Desktop, &link, &final_store, None, None)
                .unwrap();
            *seen.lock().unwrap() = first_listing(&watch, FIRST_SLOT);
            watch.stop();
        });
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(finished, "the watch did not finish");
        assert_eq!(listed.lock().unwrap().take(), Some(FolderListing::Refused));
    }
}
