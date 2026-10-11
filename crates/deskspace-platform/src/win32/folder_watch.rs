//! Watches a folder for changes off the UI thread, so the app re-reads it
//! when its contents change instead of polling.
//!
//! `ReadDirectoryChangesW` runs with overlapped I/O on a worker thread. A burst
//! of changes (a copy, an extract, an Explorer move) is debounced into a single
//! posted message, so the app restyles once rather than once per file. M2 uses
//! this for the Desktop folder; M4 phase 2 reuses it for a lens's bound folder.
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw

use std::ffi::c_void;
use std::path::Path;
use std::thread::JoinHandle;
use std::time::Instant;

use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::PCWSTR;

use crate::error::PlatformError;
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

/// A running folder watch. Dropping it stops the worker and joins it.
pub(crate) struct FolderWatch {
    stop: HANDLE,
    thread: Option<JoinHandle<()>>,
}

impl FolderWatch {
    /// Starts watching `path` on a worker thread. Each debounced change posts
    /// `message` with zero `wParam`/`lParam` to `target`.
    pub(crate) fn start(path: &Path, target: HWND, message: u32) -> Result<Self, PlatformError> {
        // SAFETY: a manual-reset, initially unsignaled event used to tell the
        // worker to finish; it is closed in `stop`.
        let stop = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }?;
        let watched = path.to_path_buf();
        // A raw `HWND`/`HANDLE` is not `Send`, so carry them as `isize` and
        // rebuild them on the worker, which only ever posts to the window.
        let target_raw = target.0 as isize;
        let stop_raw = stop.0 as isize;
        let spawned = std::thread::Builder::new()
            .name("deskspace-folder-watch".into())
            .spawn(move || watch(&watched, target_raw, message, stop_raw));
        match spawned {
            Ok(thread) => Ok(Self {
                stop,
                thread: Some(thread),
            }),
            Err(error) => {
                // SAFETY: `stop` was created above and is not yet used.
                unsafe {
                    let _ = CloseHandle(stop);
                }
                Err(error.into())
            }
        }
    }

    /// Stops the watch and waits for the worker to finish. Idempotent.
    pub(crate) fn stop(&mut self) {
        // SAFETY: `stop` is the live event created in `start`; signaling it
        // wakes the worker's wait.
        unsafe {
            let _ = SetEvent(self.stop);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // SAFETY: closed once, after the worker that waited on it has finished.
        unsafe {
            let _ = CloseHandle(self.stop);
        }
    }
}

impl Drop for FolderWatch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The worker: opens the directory and posts `message` to `target` once per
/// debounced burst until `stop` is signaled.
fn watch(path: &Path, target: isize, message: u32, stop: isize) {
    if let Err(error) = run(path, target, message, stop) {
        tracing::debug!(%error, path = %path.display(), "folder watch stopped");
    }
}

fn run(path: &Path, target: isize, message: u32, stop: isize) -> Result<(), PlatformError> {
    let target = HWND(target as *mut c_void);
    let stop = HANDLE(stop as *mut c_void);
    let name = wide(&path.to_string_lossy());
    // SAFETY: `name` is NUL-terminated. The directory is opened for overlapped
    // change reads and closed before this function returns.
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
    // SAFETY: an auto-reset, initially unsignaled event used as the overlapped
    // read's completion event; it is closed before this function returns.
    let io = match unsafe { CreateEventW(None, false, false, PCWSTR::null()) } {
        Ok(io) => io,
        Err(error) => {
            // SAFETY: `directory` was opened above and is not yet used.
            unsafe {
                let _ = CloseHandle(directory);
            }
            return Err(error.into());
        }
    };

    let mut buffer = vec![0u8; BUFFER_BYTES];
    let mut overlapped = OVERLAPPED {
        hEvent: io,
        ..Default::default()
    };
    let start = Instant::now();
    let mut coalescer = Coalescer::default();
    let mut pending = false;

    let outcome = loop {
        if !pending {
            overlapped = OVERLAPPED {
                hEvent: io,
                ..Default::default()
            };
            // SAFETY: `directory` is open for overlapped I/O, `buffer` outlives
            // the request, and `overlapped` is a stable local whose event is
            // signaled on completion.
            let started = unsafe {
                ReadDirectoryChangesW(
                    directory,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    false,
                    filter(),
                    None,
                    Some(&mut overlapped),
                    None,
                )
            };
            if let Err(error) = started {
                break Err(error.into());
            }
            pending = true;
        }
        let timeout = coalescer.wait_ms(millis(start));
        // SAFETY: both handles are live for the whole loop; `stop` is signaled
        // by the UI thread.
        let waited = unsafe { WaitForMultipleObjects(&[io, stop], false, timeout) };
        if waited == WAIT_OBJECT_0 {
            pending = false;
            coalescer.mark(millis(start));
        } else if waited.0 == WAIT_OBJECT_0.0 + 1 {
            break Ok(());
        } else if waited == WAIT_TIMEOUT {
            if coalescer.take_if_due(millis(start)) {
                // SAFETY: `target` is the controller window on the UI thread;
                // a post to a destroyed window just fails.
                unsafe {
                    let _ = PostMessageW(Some(target), message, WPARAM(0), LPARAM(0));
                }
            }
        } else {
            break Ok(());
        }
    };

    // A read may still be pending when the stop event wins. Cancel it and wait
    // for completion so `buffer` and `overlapped` are not in use when they drop.
    if pending {
        // SAFETY: `directory` and `overlapped` are the live handle and request.
        unsafe {
            let _ = CancelIoEx(directory, Some(&overlapped));
            let mut transferred = 0u32;
            let _ = GetOverlappedResult(directory, &overlapped, &mut transferred, true);
        }
    }
    // SAFETY: both handles were created here and are no longer used.
    unsafe {
        let _ = CloseHandle(io);
        let _ = CloseHandle(directory);
    }
    outcome
}

/// Milliseconds since the watch started.
fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// Debounces a burst of changes into one post: the first change opens a quiet
/// window, and a change within it extends the window; when the window passes
/// with no further change the burst is posted once. Pure, so the timing is
/// tested without real waits.
#[derive(Default)]
struct Coalescer {
    /// When the last change of the pending burst arrived, in milliseconds since
    /// the watch started; `None` while no burst is pending.
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
}
