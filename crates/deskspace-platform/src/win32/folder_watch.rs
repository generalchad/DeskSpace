//! Watches folders for changes off the UI thread, so the app re-reads one
//! when its contents change instead of polling.
//!
//! `ReadDirectoryChangesW` runs with overlapped I/O on one worker thread that
//! serves every watch through an I/O completion port. A burst of changes (a
//! copy, an extract, an Explorer move) is debounced per folder into a single
//! posted message, so the app restyles once rather than once per file. M2 uses
//! this for the Desktop folder; M4 phase 2 uses it for every lens's folder.
//! https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::Win32::System::IO::{
    CancelIoEx, CreateIoCompletionPort, GetOverlappedResult, GetQueuedCompletionStatus, OVERLAPPED,
    PostQueuedCompletionStatus,
};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::{HRESULT, PCWSTR};

use crate::error::PlatformError;
use crate::input::WatchKey;
use crate::win32::system::wide;

/// How long a change waits for the burst to settle before the app is woken.
/// Long enough to fold a multi-file copy into one wake, short enough that the
/// desktop layer still follows promptly.
const DEBOUNCE_MS: u64 = 150;

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
const FIRST_SLOT: usize = 3;

/// The watches and the worker that serves them. Dropping it stops the worker
/// and joins it.
pub(crate) struct FolderWatch {
    port: HANDLE,
    thread: Option<JoinHandle<()>>,
    /// The paths for add packets the worker has not handled yet, by slot.
    additions: Arc<Mutex<HashMap<usize, PathBuf>>>,
    /// The slot and path behind each app-level key.
    active: HashMap<WatchKey, ActiveWatch>,
    /// The app-level key behind each slot, to decode a posted change.
    slots: HashMap<usize, WatchKey>,
    next_slot: usize,
    port_closed: bool,
}

/// One key's watch, as the UI thread remembers it.
struct ActiveWatch {
    slot: usize,
    path: PathBuf,
}

impl FolderWatch {
    /// Starts the worker that serves every folder watch. Each debounced change
    /// posts `message` with the folder's slot in `wParam` to `target`.
    pub(crate) fn start(target: HWND, message: u32) -> Result<Self, PlatformError> {
        // SAFETY: creates a completion port with no file handle; it is closed
        // in `stop`, after the worker that waits on it has finished.
        let port = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 0) }?;
        let additions = Arc::new(Mutex::new(HashMap::new()));
        let pending = Arc::clone(&additions);
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
                    folders: HashMap::new(),
                    bursts: Bursts::default(),
                    start: Instant::now(),
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
                active: HashMap::new(),
                slots: HashMap::new(),
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

    /// Watches `path` for `key`. Does nothing when that key already watches
    /// the same path; another path replaces the key's watch.
    pub(crate) fn watch(&mut self, key: WatchKey, path: &Path) -> Result<(), PlatformError> {
        if let Some(active) = self.active.get(&key) {
            if active.path == path {
                return Ok(());
            }
            self.unwatch(key);
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.additions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(slot, path.to_path_buf());
        if let Err(error) = self.post(ADD_KEY, slot) {
            self.additions
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&slot);
            return Err(error.into());
        }
        self.active.insert(
            key,
            ActiveWatch {
                slot,
                path: path.to_path_buf(),
            },
        );
        self.slots.insert(slot, key);
        Ok(())
    }

    /// Stops the watch for `key`, if it has one.
    pub(crate) fn unwatch(&mut self, key: WatchKey) {
        let Some(active) = self.active.remove(&key) else {
            return;
        };
        self.slots.remove(&active.slot);
        if let Err(error) = self.post(REMOVE_KEY, active.slot) {
            tracing::warn!(%error, "could not remove a folder watch");
        }
    }

    /// The app-level key behind `slot`, if it is still watched.
    pub(crate) fn key_for_slot(&self, slot: usize) -> Option<WatchKey> {
        self.slots.get(&slot).copied()
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

/// The worker: one completion port for every folder, and a debounce clock per
/// folder.
struct Worker {
    port: HANDLE,
    target: HWND,
    message: u32,
    additions: Arc<Mutex<HashMap<usize, PathBuf>>>,
    folders: HashMap<usize, Box<Folder>>,
    bursts: Bursts,
    start: Instant,
}

impl Worker {
    fn run(mut self) -> Result<(), PlatformError> {
        let outcome = self.watch();
        self.close_all();
        outcome
    }

    fn watch(&mut self) -> Result<(), PlatformError> {
        loop {
            let timeout = self.bursts.wait_ms(millis(self.start));
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
                // Any other key is a folder's slot, registered with the port
                // when the folder's first read started.
                Ok(()) => self.completed(key, now),
                Err(error) if is_timeout(&error) => self.post_due(now),
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Opens a newly added folder and starts its first read.
    fn add(&mut self, slot: usize) {
        let path = self
            .additions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
        let Some(path) = path else {
            tracing::warn!(slot, "folder watch add had no path");
            return;
        };
        match open_folder(&path, self.port, slot) {
            Ok(folder) => {
                self.bursts.add(slot);
                self.folders.insert(slot, folder);
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "could not watch the folder");
            }
        }
    }

    /// Stops one folder's watch, canceling its pending read before the handle
    /// and buffer are freed.
    fn remove(&mut self, slot: usize) {
        self.bursts.remove(slot);
        if let Some(folder) = self.folders.remove(&slot) {
            folder.close();
        }
    }

    /// One folder's read finished: start the next read and extend its burst.
    fn completed(&mut self, slot: usize, now_ms: u64) {
        let Some(folder) = self.folders.get_mut(&slot) else {
            return;
        };
        if !folder.pending {
            return;
        }
        folder.pending = false;
        self.bursts.mark(slot, now_ms);
        if let Err(error) = folder.read_changes() {
            tracing::debug!(
                %error,
                path = %folder.path.display(),
                "folder watch stopped for this folder"
            );
        }
    }

    /// Posts `message` for every folder whose burst is due.
    fn post_due(&mut self, now_ms: u64) {
        for slot in self.bursts.take_due(now_ms) {
            // SAFETY: `target` is the controller window on the UI thread; a
            // post to a destroyed window just fails.
            unsafe {
                let _ = PostMessageW(Some(self.target), self.message, WPARAM(slot), LPARAM(0));
            }
        }
    }

    /// Stops every folder's read before the worker exits.
    fn close_all(&mut self) {
        self.bursts.clear();
        for (_, folder) in self.folders.drain() {
            folder.close();
        }
    }
}

/// One watched folder. Boxed so its buffer and `OVERLAPPED` stay at a stable
/// address while Windows is reading into them.
struct Folder {
    path: PathBuf,
    directory: HANDLE,
    buffer: Vec<u8>,
    overlapped: OVERLAPPED,
    pending: bool,
}

impl Folder {
    /// Starts the next overlapped read. The records are ignored; the
    /// completion alone means "something changed".
    fn read_changes(&mut self) -> Result<(), PlatformError> {
        self.overlapped = OVERLAPPED::default();
        // SAFETY: `directory` is open for overlapped I/O, `buffer` and
        // `overlapped` outlive the request because the folder is boxed and
        // lives until the read completes, and a synchronous success still
        // queues a completion packet for the port.
        let started = unsafe {
            ReadDirectoryChangesW(
                self.directory,
                self.buffer.as_mut_ptr().cast(),
                self.buffer.len() as u32,
                false,
                filter(),
                None,
                Some(&mut self.overlapped),
                None,
            )
        };
        match started {
            Ok(()) => {
                self.pending = true;
                Ok(())
            }
            Err(error) if error.code() == ERROR_IO_PENDING.to_hresult() => {
                self.pending = true;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Cancels a pending read, waits for it to finish, and closes the handle,
    /// so Windows cannot write into freed memory.
    fn close(self) {
        if self.pending {
            // SAFETY: `directory` and `overlapped` are the live handle and
            // request. Waiting for completion before the handles drop keeps
            // the buffer valid for the whole read.
            unsafe {
                let _ = CancelIoEx(self.directory, Some(&self.overlapped));
                let mut transferred = 0;
                let _ =
                    GetOverlappedResult(self.directory, &self.overlapped, &mut transferred, true);
            }
        }
        // SAFETY: the handle was opened in `open_folder` and is closed once.
        unsafe {
            let _ = CloseHandle(self.directory);
        }
    }
}

/// Opens `path` for overlapped change reads and registers it with `port`
/// under `slot`.
fn open_folder(path: &Path, port: HANDLE, slot: usize) -> Result<Box<Folder>, PlatformError> {
    let name = wide(&path.to_string_lossy());
    // SAFETY: `name` is NUL-terminated. The directory is opened for overlapped
    // change reads; every error path below closes it before returning.
    let directory = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )
    }?;
    // SAFETY: `directory` is a live handle and `port` is live for the worker's
    // lifetime. Registering the folder makes its reads report under `slot`.
    if let Err(error) = unsafe { CreateIoCompletionPort(directory, Some(port), slot, 0) } {
        // SAFETY: `directory` was opened above and no read is pending.
        unsafe {
            let _ = CloseHandle(directory);
        }
        return Err(error.into());
    }
    let mut folder = Box::new(Folder {
        path: path.to_path_buf(),
        directory,
        buffer: vec![0u8; BUFFER_BYTES],
        overlapped: OVERLAPPED::default(),
        pending: false,
    });
    if let Err(error) = folder.read_changes() {
        // SAFETY: `read_changes` failed before a read started, so no read is
        // pending; the handle is closed once.
        unsafe {
            let _ = CloseHandle(folder.directory);
        }
        return Err(error);
    }
    Ok(folder)
}

/// Whether a failed wait timed out rather than broke.
fn is_timeout(error: &windows::core::Error) -> bool {
    error.code() == HRESULT::from_win32(WAIT_TIMEOUT.0)
}

/// Milliseconds since the worker started.
fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// The debounce clock of every watched folder, by slot. Pure, so the timing is
/// tested without real folders or waits.
#[derive(Default)]
struct Bursts {
    by_slot: HashMap<usize, Coalescer>,
}

impl Bursts {
    fn add(&mut self, slot: usize) {
        self.by_slot.insert(slot, Coalescer::default());
    }

    fn remove(&mut self, slot: usize) {
        self.by_slot.remove(&slot);
    }

    fn mark(&mut self, slot: usize, now_ms: u64) {
        if let Some(coalescer) = self.by_slot.get_mut(&slot) {
            coalescer.mark(now_ms);
        }
    }

    /// The shortest wait any folder needs, or `INFINITE` when none does.
    fn wait_ms(&self, now_ms: u64) -> u32 {
        self.by_slot
            .values()
            .map(|coalescer| coalescer.wait_ms(now_ms))
            .min()
            .unwrap_or(INFINITE)
    }

    /// Takes the slots whose burst is due.
    fn take_due(&mut self, now_ms: u64) -> Vec<usize> {
        self.by_slot
            .iter_mut()
            .filter_map(|(slot, coalescer)| coalescer.take_if_due(now_ms).then_some(*slot))
            .collect()
    }

    fn clear(&mut self) {
        self.by_slot.clear();
    }
}

/// Debounces a burst of changes into one post: the first change opens a quiet
/// window, and a change within it extends the window; when the window passes
/// with no further change the burst is posted once. Pure, so the timing is
/// tested without real waits.
#[derive(Default)]
struct Coalescer {
    /// When the last change of the pending burst arrived, in milliseconds since
    /// the worker started; `None` while no burst is pending.
    pending: Option<u64>,
}

impl Coalescer {
    /// Folds a change at `now_ms` into the pending burst.
    fn mark(&mut self, now_ms: u64) {
        self.pending = Some(now_ms);
    }

    /// How long to wait for the next change: `INFINITE` while no burst is
    /// pending, else the time left in the quiet window (never negative).
    fn wait_ms(&self, now_ms: u64) -> u32 {
        match self.pending {
            None => INFINITE,
            Some(last) => DEBOUNCE_MS.saturating_sub(now_ms.saturating_sub(last)) as u32,
        }
    }

    /// Clears the pending burst and returns true to post, once `now_ms` is at
    /// least `DEBOUNCE_MS` past its last change.
    fn take_if_due(&mut self, now_ms: u64) -> bool {
        match self.pending {
            Some(last) if now_ms.saturating_sub(last) >= DEBOUNCE_MS => {
                self.pending = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_watch_waits_forever() {
        assert_eq!(Coalescer::default().wait_ms(10_000), INFINITE);
    }

    #[test]
    fn a_burst_of_changes_posts_once() {
        let mut coalescer = Coalescer::default();
        coalescer.mark(0);
        assert_eq!(coalescer.wait_ms(0), DEBOUNCE_MS as u32);
        // A change inside the window extends it instead of posting early.
        coalescer.mark(100);
        assert_eq!(coalescer.wait_ms(100), DEBOUNCE_MS as u32);
        assert!(!coalescer.take_if_due(200));
        assert!(coalescer.take_if_due(250));
        assert_eq!(coalescer.wait_ms(250), INFINITE);
    }

    #[test]
    fn changes_separated_by_the_window_post_apart() {
        let mut coalescer = Coalescer::default();
        coalescer.mark(0);
        assert!(coalescer.take_if_due(150));
        coalescer.mark(500);
        assert!(!coalescer.take_if_due(600));
        assert!(coalescer.take_if_due(650));
    }

    #[test]
    fn two_folders_debounce_independently() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.add(2);
        bursts.mark(1, 0);
        // Folder 1 is due at 150 ms; folder 2 has no burst at all.
        assert!(!bursts.take_due(100).contains(&1));
        assert_eq!(bursts.take_due(150), vec![1]);
        assert_eq!(bursts.wait_ms(150), INFINITE);
        // A later change to folder 2 opens its own window.
        bursts.mark(2, 200);
        assert_eq!(bursts.wait_ms(250), 100);
        assert_eq!(bursts.take_due(350), vec![2]);
    }

    #[test]
    fn the_soonest_timeout_wins() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.add(2);
        bursts.mark(1, 0);
        bursts.mark(2, 100);
        // At 100 ms folder 1 has 50 ms left and folder 2 has the full window.
        assert_eq!(bursts.wait_ms(100), 50);
        assert_eq!(bursts.take_due(150), vec![1]);
        assert_eq!(bursts.wait_ms(150), 100);
    }

    #[test]
    fn removing_an_unknown_slot_changes_nothing() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.mark(1, 0);
        bursts.remove(2);
        assert_eq!(bursts.wait_ms(100), 50);
        assert_eq!(bursts.take_due(150), vec![1]);
    }
}
