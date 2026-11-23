//! Open folder handles, overlapped change reads, and move/store checks.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use deskspace_core::{LensBuilder, LensList};
use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, HANDLE, MAX_PATH, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NAME_NORMALIZED, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GETFINALPATHNAMEBYHANDLE_FLAGS, GetFinalPathNameByHandleW, OPEN_EXISTING,
    ReadDirectoryChangesW, VOLUME_NAME_DOS,
};
use windows::Win32::System::IO::{CancelIoEx, CreateIoCompletionPort, OVERLAPPED};
use windows::core::{HRESULT, PCWSTR};

use super::{BUFFER_BYTES, SharedListing, filter};
use crate::error::PlatformError;
use crate::input::FolderListing;
use crate::win32::folder_list::list_folder;
use crate::win32::names::{in_recycle_bin, is_inside, plain_path, same_volume};
use crate::win32::system::wide;

/// A canceled read's buffer and `OVERLAPPED`. Windows writes the read's result
/// into the `OVERLAPPED` when its packet leaves the port, so both stay alive
/// until then.
pub(super) struct CanceledRead {
    _buffer: Vec<u8>,
    _overlapped: Box<OVERLAPPED>,
}

/// One watched folder.
pub(super) struct Folder {
    /// The folder whose contents are listed: a lens's own folder, or the
    /// subfolder it shows while browsed.
    pub(super) path: PathBuf,
    /// DeskSpace's storage folder as a final path. A folder is refused when
    /// its own final path is this or inside it.
    pub(super) store: PathBuf,
    /// The lens's own folder whose handle is watched, when `path` is a
    /// subfolder below it. `None` for a watch on the folder it lists and for
    /// the Desktop.
    pub(super) root: Option<PathBuf>,
    pub(super) listing: SharedListing,
    /// Turns a lens's listings into its item list. The Desktop's watch has
    /// none: the layer re-reads the shell.
    pub(super) builder: Option<LensBuilder>,
    /// `None` while the folder is missing and waiting for a retry.
    pub(super) open: Option<OpenFolder>,
    /// When to try the folder again, in milliseconds since the worker started.
    pub(super) retry_at: Option<u64>,
}

impl Folder {
    /// The folder whose handle is open. A browsed lens watches its own folder,
    /// not the subfolder it lists, because Windows refuses to rename a folder
    /// while a directory below it is open.
    pub(super) fn watch_path(&self) -> &Path {
        self.root.as_deref().unwrap_or(&self.path)
    }

    /// Opens the watched folder if it is not open and starts its read under
    /// `read_key`, watching below it when it lists a subfolder. A folder that
    /// resolves to the store or inside it is closed again before any read
    /// starts. The final path comes from the handle just opened, on this
    /// worker, so the UI thread never waits on it.
    pub(super) fn open(&mut self, port: HANDLE, read_key: usize) -> Result<(), OpenFailure> {
        if self.open.is_some() {
            return Ok(());
        }
        let watch_path = self.watch_path();
        let subtree = self.root.is_some();
        let mut open =
            open_folder(watch_path, port, read_key, subtree).map_err(OpenFailure::Failed)?;
        // Some file systems cannot name a final path; `canonicalize` fails
        // there too, and the store check has always fallen back to the path
        // as given, with its `..` steps taken (no IO).
        let resolved = final_path(open.directory).unwrap_or_else(|error| {
            tracing::debug!(%error, path = %watch_path.display(), "no final path; checking the path as given");
            std::path::absolute(watch_path).unwrap_or_else(|_| watch_path.to_path_buf())
        });
        if is_inside(&resolved, &self.store) {
            // No read started, so nothing is left to wait for.
            let _ = open.close();
            return Err(OpenFailure::Refused(resolved));
        }
        // The subfolder a browsed lens lists is a path inside the watched
        // root, so it is checked on its own too: it may be a junction into
        // the store even when the root is not.
        if subtree
            && let Ok(resolved) = std::fs::canonicalize(&self.path)
            && is_inside(&resolved, &self.store)
        {
            let _ = open.close();
            return Err(OpenFailure::Refused(resolved));
        }
        if let Err(error) = open.read_changes() {
            let _ = open.close();
            return Err(OpenFailure::Failed(error));
        }
        self.open = Some(open);
        Ok(())
    }

    /// What the watched folder's path names now, when that is no longer a
    /// folder: `Unavailable` when it is gone, or `Refused` when the watched or
    /// listed folder resolves into the store. The open handle follows a
    /// renamed folder, but a listing reads the path, so a junction into the
    /// store put in the old folder's place would be listed.
    /// A folder renamed or moved where a lens may follow it (`follows_to`)
    /// is `Moved` instead of `Unavailable`; the open handle names where it
    /// went. A browsed lens's own folder is the one watched, so its new path
    /// is reported to the app, which follows it and drops the subfolder.
    pub(super) fn moved(&self) -> Option<FolderListing> {
        let watch_path = self.watch_path();
        if !watch_path.is_dir() {
            let to = self
                .open
                .as_ref()
                .and_then(|open| final_path(open.directory).ok())
                .map(|to| plain_path(&to))
                .filter(|to| follows_to(watch_path, to, &self.store) && to.is_dir());
            return Some(match to {
                Some(to) => FolderListing::Moved { to },
                None => FolderListing::Unavailable,
            });
        }
        let into_store = |path: &Path| {
            std::fs::canonicalize(path).is_ok_and(|resolved| is_inside(&resolved, &self.store))
        };
        (into_store(watch_path) || (self.root.is_some() && into_store(&self.path)))
            .then_some(FolderListing::Refused)
    }

    /// The listing to report for `listing`, preferring a moved root to a
    /// subfolder that is gone: a change burst can list the old subfolder a
    /// moment before the path check sees that the lens's own folder moved.
    pub(super) fn prefer_moved(&self, listing: FolderListing) -> FolderListing {
        match listing {
            FolderListing::Unavailable => self.moved().unwrap_or(FolderListing::Unavailable),
            other => other,
        }
    }

    /// The folder's current contents as a finished list, or `Unavailable` with
    /// the reason logged.
    pub(super) fn list(&mut self) -> FolderListing {
        match list_folder(&self.path) {
            Ok(entries) => {
                let Some(builder) = self.builder.as_mut() else {
                    return FolderListing::Listed(LensList::default());
                };
                let start = Instant::now();
                let list = builder.listed(entries);
                tracing::debug!(
                    items = list.items.len(),
                    elapsed = ?start.elapsed(),
                    path = %self.path.display(),
                    "built a lens's list"
                );
                FolderListing::Listed(list)
            }
            Err(error) => {
                tracing::debug!(%error, path = %self.path.display(), "could not list the folder");
                FolderListing::Unavailable
            }
        }
    }
}

/// One open folder handle and its pending change read.
///
/// The buffer and the `OVERLAPPED` live on the heap, so their addresses stay
/// the ones Windows was given however this value moves: `CancelIoEx` finds a
/// read by that address, and Windows writes the read's result there.
pub(super) struct OpenFolder {
    pub(super) read_key: usize,
    directory: HANDLE,
    buffer: Vec<u8>,
    overlapped: Box<OVERLAPPED>,
    pub(super) pending: bool,
    /// Whether a change below the watched folder counts too. A browsed lens
    /// watches its own folder, so changes inside the subfolder it shows are
    /// only reported with this set.
    subtree: bool,
}

impl OpenFolder {
    /// Starts the next overlapped read. The records are ignored; the
    /// completion alone means "something changed".
    pub(super) fn read_changes(&mut self) -> Result<(), PlatformError> {
        *self.overlapped = OVERLAPPED::default();
        // SAFETY: `directory` is open for overlapped I/O, `buffer` and
        // `overlapped` are heap allocations that live until the read's packet
        // is taken off the port (`close` hands a pending read's to the
        // worker), and a synchronous success still queues a packet.
        let started = unsafe {
            ReadDirectoryChangesW(
                self.directory,
                self.buffer.as_mut_ptr().cast(),
                self.buffer.len() as u32,
                self.subtree,
                filter(),
                None,
                Some(&mut *self.overlapped),
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

    /// Cancels a pending read and closes the handle. A canceled read is not
    /// over until its packet leaves the port, which only the worker's wait
    /// does, so waiting for it here would never end. Its buffer and
    /// `OVERLAPPED` come back instead, for the worker to keep until then.
    pub(super) fn close(self) -> Option<CanceledRead> {
        if self.pending {
            // SAFETY: `directory` and `overlapped` are the live handle and
            // request.
            unsafe {
                let _ = CancelIoEx(self.directory, Some(&*self.overlapped));
            }
        }
        // SAFETY: the handle was opened in `open_folder` and is closed once.
        // Closing it with a read out is safe: the read still completes to the
        // port.
        unsafe {
            let _ = CloseHandle(self.directory);
        }
        self.pending.then_some(CanceledRead {
            _buffer: self.buffer,
            _overlapped: self.overlapped,
        })
    }
}

/// Opens `path` for overlapped change reads and registers it with `port`
/// under `read_key`.
fn open_folder(
    path: &Path,
    port: HANDLE,
    read_key: usize,
    subtree: bool,
) -> Result<OpenFolder, PlatformError> {
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
    // lifetime. Registering the folder makes its reads report under `read_key`.
    if let Err(error) = unsafe { CreateIoCompletionPort(directory, Some(port), read_key, 0) } {
        // SAFETY: `directory` was opened above and no read is pending.
        unsafe {
            let _ = CloseHandle(directory);
        }
        return Err(error.into());
    }
    Ok(OpenFolder {
        read_key,
        directory,
        buffer: vec![0u8; BUFFER_BYTES],
        overlapped: Box::default(),
        pending: false,
        subtree,
    })
}

/// Whether a lens on `from` follows its folder to `to` (ROADMAP D19): only on
/// the same volume, where a move is a rename the open handle follows, and
/// never into a Recycle Bin or DeskSpace's storage folder `store`.
fn follows_to(from: &Path, to: &Path, store: &Path) -> bool {
    same_volume(from, to) && !in_recycle_bin(to) && !is_inside(to, store)
}

/// Why a watched folder could not be opened.
#[derive(Debug)]
pub(super) enum OpenFailure {
    /// Its final path, given here, is the store or inside it.
    Refused(PathBuf),
    Failed(PlatformError),
}

/// The final path of an open handle, through junctions, links, and short
/// names, in the `\\?\` form.
/// https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew
fn final_path(handle: HANDLE) -> Result<PathBuf, PlatformError> {
    let flags = GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0);
    let mut buffer = vec![0u16; MAX_PATH as usize];
    // A buffer too small returns the length needed, with the terminator; one
    // large enough returns the length written, without it. So a second call
    // with the length the first asked for always fits.
    for _ in 0..2 {
        // SAFETY: `handle` is open, and the call writes at most
        // `buffer.len()` characters into `buffer`.
        let length = unsafe { GetFinalPathNameByHandleW(handle, &mut buffer, flags) } as usize;
        if length == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        buffer.resize(length, 0);
    }
    Err(PlatformError::InvalidFormat("a final path"))
}

/// Whether a failed wait timed out rather than broke.
pub(super) fn is_timeout(error: &windows::core::Error) -> bool {
    error.code() == HRESULT::from_win32(WAIT_TIMEOUT.0)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::super::tests::store_with_junction;
    use super::*;

    #[test]
    fn a_lens_follows_only_on_its_volume_outside_the_bin_and_the_store() {
        let from = Path::new(r"C:\Users\me\Work");
        let store = Path::new(r"C:\Users\me\AppData\Roaming\DeskSpace\storage");
        let follows = |to: &str| follows_to(from, Path::new(to), store);
        assert!(follows(r"C:\Users\me\Projects"), "a rename");
        assert!(follows(r"C:\Archive\Work"), "a move on the same volume");
        assert!(!follows(r"C:\$Recycle.Bin\S-1-5-21\$R0ABC12"), "deleted");
        assert!(
            !follows(r"C:\Users\me\AppData\Roaming\DeskSpace\storage\Work"),
            "into the store"
        );
        assert!(!follows(r"D:\Work"), "onto another volume");
    }

    /// The 5 s path check finds a watched path that now leads into the store,
    /// as when a folder is renamed away and a junction put in its place.
    #[test]
    fn a_path_that_now_leads_into_the_store_is_refused() {
        let (scratch, link, final_store) = store_with_junction("moved");
        let folder = |path: &Path| Folder {
            path: path.to_path_buf(),
            store: final_store.clone(),
            root: None,
            listing: Arc::new(Mutex::new(None)),
            builder: None,
            open: None,
            retry_at: None,
        };
        let moved = folder(&link).moved();
        let plain = folder(&scratch).moved();
        let gone = folder(&scratch.join("gone")).moved();
        let _ = std::fs::remove_dir_all(&scratch);
        assert_eq!(moved, Some(FolderListing::Refused));
        assert_eq!(plain, None, "a folder outside the store is still fine");
        assert_eq!(gone, Some(FolderListing::Unavailable));
    }

    /// A browsed lens's shown subfolder is checked for the store on its own,
    /// because the handle is on the lens's own folder.
    #[test]
    fn a_browsed_subfolder_that_leads_into_the_store_is_refused() {
        let (scratch, link, final_store) = store_with_junction("browsed-moved");
        let folder = Folder {
            path: link,
            store: final_store,
            root: Some(scratch.clone()),
            listing: Arc::new(Mutex::new(None)),
            builder: None,
            open: None,
            retry_at: None,
        };
        assert_eq!(folder.moved(), Some(FolderListing::Refused));
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
