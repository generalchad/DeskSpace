//! Reads one folder's entries for a lens. The watch worker calls this off the
//! UI thread, so a slow folder never freezes the spaces.
//!
//! `FindFirstFileExW` with `FindExInfoBasic` and `FIND_FIRST_EX_LARGE_FETCH`
//! reads names and attributes without the short 8.3 name, which is the fast
//! path for a large folder.
//! https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use windows::Win32::Foundation::{ERROR_NO_MORE_FILES, FILETIME, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM,
    FIND_FIRST_EX_LARGE_FETCH, FindClose, FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW,
    FindNextFileW, WIN32_FIND_DATAW,
};
use windows::core::PCWSTR;

use deskspace_core::item::ItemKind;
use deskspace_core::source::FolderEntry;

use crate::error::PlatformError;
use crate::win32::system::wide;

/// 100 ns steps in one second.
const TICKS_PER_SECOND: u64 = 10_000_000;
/// `FILETIME` steps from 1601-01-01 to 1970-01-01, the Unix epoch.
const UNIX_EPOCH_TICKS: u64 = 11_644_473_600 * TICKS_PER_SECOND;

/// Lists `path` as a lens sees it. `.` and `..` are left out; hidden and
/// system entries are listed with `hidden` set, and `SyncLens` decides whether
/// to show them.
pub(crate) fn list_folder(path: &Path) -> Result<Vec<FolderEntry>, PlatformError> {
    let pattern = wide(&path.join("*").to_string_lossy());
    let mut data = WIN32_FIND_DATAW::default();
    // SAFETY: `pattern` is NUL-terminated, `data` is a live output buffer, and
    // the returned handle is closed by `FindHandle`.
    let found = unsafe {
        FindFirstFileExW(
            PCWSTR(pattern.as_ptr()),
            FindExInfoBasic,
            &mut data as *mut _ as *mut core::ffi::c_void,
            FindExSearchNameMatch,
            None,
            FIND_FIRST_EX_LARGE_FETCH,
        )
    }?;
    let find = FindHandle(found);

    let mut entries = Vec::new();
    loop {
        let name = file_name(&data);
        if name.as_os_str() != OsStr::new(".") && name.as_os_str() != OsStr::new("..") {
            let size = (u64::from(data.nFileSizeHigh) << 32) | u64::from(data.nFileSizeLow);
            entries.push(entry_from(
                path,
                &name,
                data.dwFileAttributes,
                size,
                data.ftLastWriteTime,
            ));
        }
        // SAFETY: `find.0` is the live handle from `FindFirstFileExW`, and
        // `data` receives the next entry.
        match unsafe { FindNextFileW(find.0, &mut data) } {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(entries)
}

/// Closes a find handle exactly once, on every path out of `list_folder`.
struct FindHandle(HANDLE);

impl Drop for FindHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` is the live handle from `FindFirstFileExW`, and this
        // is the only place it is closed.
        unsafe {
            let _ = FindClose(self.0);
        }
    }
}

/// The entry for one `WIN32_FIND_DATAW` result. Pure, so every attribute and
/// timestamp rule is tested without a real folder.
fn entry_from(
    folder: &Path,
    name: &OsStr,
    attributes: u32,
    size: u64,
    last_write: FILETIME,
) -> FolderEntry {
    let is_folder = attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
    let path = folder.join(name);
    let kind = ItemKind::classify(&path, is_folder);
    FolderEntry {
        path,
        label: String::new(),
        kind,
        size: (!is_folder).then_some(size),
        modified: unix_seconds(last_write),
        hidden: attributes & (FILE_ATTRIBUTE_HIDDEN.0 | FILE_ATTRIBUTE_SYSTEM.0) != 0,
    }
}

/// A `FILETIME`'s seconds since the Unix epoch, or `None` when it is unset or
/// before the epoch.
fn unix_seconds(time: FILETIME) -> Option<u64> {
    let ticks = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    (ticks >= UNIX_EPOCH_TICKS).then(|| (ticks - UNIX_EPOCH_TICKS) / TICKS_PER_SECOND)
}

/// The NUL-terminated name in a find result, kept as `OsString` so a name that
/// is not valid Unicode survives unchanged.
fn file_name(data: &WIN32_FIND_DATAW) -> OsString {
    let length = data
        .cFileName
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(data.cFileName.len());
    OsString::from_wide(&data.cFileName[..length])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filetime(seconds: u64) -> FILETIME {
        let ticks = UNIX_EPOCH_TICKS + seconds * TICKS_PER_SECOND;
        FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        }
    }

    #[test]
    fn a_file_carries_its_name_size_and_modification_time() {
        let entry = entry_from(
            Path::new("C:/folder"),
            OsStr::new("report.txt"),
            0,
            42,
            filetime(1_700_000_000),
        );
        assert_eq!(entry.path, Path::new("C:/folder/report.txt"));
        assert_eq!(entry.kind, ItemKind::File);
        assert_eq!(entry.size, Some(42));
        assert_eq!(entry.modified, Some(1_700_000_000));
        assert!(!entry.hidden);
        assert_eq!(entry.label, "", "the item falls back to the file stem");
    }

    #[test]
    fn a_folder_has_no_size() {
        let entry = entry_from(
            Path::new("C:/folder"),
            OsStr::new("sub"),
            FILE_ATTRIBUTE_DIRECTORY.0,
            4096,
            filetime(1_700_000_000),
        );
        assert_eq!(entry.kind, ItemKind::Folder);
        assert_eq!(entry.size, None);
    }

    #[test]
    fn hidden_and_system_files_are_flagged() {
        for attributes in [FILE_ATTRIBUTE_HIDDEN.0, FILE_ATTRIBUTE_SYSTEM.0] {
            let entry = entry_from(
                Path::new("C:/folder"),
                OsStr::new("secret.txt"),
                attributes,
                1,
                filetime(0),
            );
            assert!(entry.hidden, "{attributes:#x} should be hidden");
        }
    }

    #[test]
    fn an_unset_or_pre_epoch_time_has_no_modified_value() {
        for ticks in [0, UNIX_EPOCH_TICKS - 1] {
            let time = FILETIME {
                dwLowDateTime: ticks as u32,
                dwHighDateTime: (ticks >> 32) as u32,
            };
            assert_eq!(unix_seconds(time), None);
        }
        assert_eq!(unix_seconds(filetime(0)), Some(0));
    }

    #[test]
    fn a_shortcut_keeps_its_extensions() {
        let entry = entry_from(
            Path::new("C:/folder"),
            OsStr::new("tool.lnk"),
            0,
            1,
            filetime(0),
        );
        assert_eq!(entry.kind, ItemKind::Shortcut);
    }
}
