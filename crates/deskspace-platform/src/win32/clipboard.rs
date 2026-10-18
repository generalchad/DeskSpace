//! The shell file clipboard (`CF_HDROP`), so items can be cut, copied, and
//! pasted with Explorer and other applications.
//!
//! Copy and cut write a `CF_HDROP` file list plus a "Preferred DropEffect"
//! (`MOVE` for cut), the same formats Explorer uses. Real handles are handed
//! to the clipboard, so the data outlives this process and no `IDataObject`
//! has to be implemented.
//! https://learn.microsoft.com/en-us/windows/win32/shell/clipboard#cf_hdrop
//!
//! Plain text (`CF_UNICODETEXT`) is written too, for the debug window's
//! console and diagnostics.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, OleGetClipboard,
};
use windows::Win32::UI::Shell::{DROPFILES, DragQueryFileW, HDROP};
use windows::core::{BOOL, PCWSTR};

use crate::error::PlatformError;
use crate::input::ShellItem;
use crate::win32::system::wide;

/// Files on the clipboard and whether they were cut (to be moved) or copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardFiles {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

/// Replaces the clipboard with `paths`, marked as cut (`MOVE`) or copied.
pub(crate) fn set_file_list(paths: &[PathBuf], cut: bool) -> Result<(), PlatformError> {
    if paths.is_empty() {
        return clear();
    }
    let buffer = dropfiles_buffer(paths);
    let effect = if cut {
        DROPEFFECT_MOVE
    } else {
        DROPEFFECT_COPY
    };
    // SAFETY: the clipboard is opened and closed around the writes; each global
    // handle is owned by us until `SetClipboardData` takes it (or it is freed
    // on failure).
    unsafe {
        open_clipboard()?;
        let result = (|| {
            EmptyClipboard()?;
            set_bytes(CF_HDROP.0 as u32, &buffer)?;
            set_bytes(preferred_effect_format(), &effect.0.to_le_bytes())
        })();
        let _ = CloseClipboard();
        result
    }
}

/// The files on the clipboard, or `None` when it holds none.
pub(crate) fn file_list() -> Option<ClipboardFiles> {
    // SAFETY: the clipboard is opened and closed around the reads; handles
    // returned by `GetClipboardData` are owned by the clipboard, not freed here.
    unsafe {
        if open_clipboard().is_err() {
            return None;
        }
        let result = read_files();
        let _ = CloseClipboard();
        result
    }
}

/// The shell items on the clipboard (files, folders, or virtual items copied
/// in Explorer), or an empty list when it holds none.
/// https://learn.microsoft.com/en-us/windows/win32/api/ole2/nf-ole2-olegetclipboard
pub(crate) fn shell_items() -> Vec<ShellItem> {
    // SAFETY: OLE is initialized on the UI thread; the returned data object is
    // released when `data` drops.
    let Ok(data) = with_clipboard_retry(|| unsafe { OleGetClipboard() }) else {
        return Vec::new();
    };
    super::drop_target::shell_items(&data).unwrap_or_default()
}

/// Replaces the clipboard with `text` as `CF_UNICODETEXT`, owned by `owner`.
/// https://learn.microsoft.com/en-us/windows/win32/dataxchg/standard-clipboard-formats
pub(crate) fn set_text(owner: HWND, text: &str) -> Result<(), PlatformError> {
    let bytes: Vec<u8> = wide(text)
        .into_iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    // SAFETY: opening the clipboard for our own live window.
    with_clipboard_retry(|| unsafe { OpenClipboard(Some(owner)) })?;
    // SAFETY: the clipboard is open until the close below; the global block
    // is ours until `SetClipboardData` takes it.
    unsafe {
        let result = (|| {
            EmptyClipboard()?;
            set_bytes(CF_UNICODETEXT.0 as u32, &bytes)
        })();
        let _ = CloseClipboard();
        result
    }
}

/// Empties the clipboard.
pub(crate) fn clear() -> Result<(), PlatformError> {
    // SAFETY: open, empty, and close, with no borrowed state.
    unsafe {
        open_clipboard()?;
        let result = EmptyClipboard();
        let _ = CloseClipboard();
        Ok(result?)
    }
}

/// How many times to retry opening the clipboard before giving up.
const OPEN_ATTEMPTS: usize = 5;

/// Opens the clipboard, retrying briefly: another process (often Explorer) can
/// hold it at the moment a change notification arrives.
fn open_clipboard() -> Result<(), PlatformError> {
    // SAFETY: opening the clipboard with no owner window.
    with_clipboard_retry(|| unsafe { OpenClipboard(None) }).map_err(PlatformError::from)
}

/// Retries a clipboard call briefly while another process holds the clipboard.
fn with_clipboard_retry<T, E>(mut op: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    let mut last = None;
    for _ in 0..OPEN_ATTEMPTS {
        match op() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
    Err(last.expect("at least one attempt"))
}

/// A `CF_HDROP` buffer: a `DROPFILES` header followed by the paths as
/// NUL-terminated UTF-16, with a final NUL ending the list.
pub(crate) fn dropfiles_buffer(paths: &[PathBuf]) -> Vec<u8> {
    let header = std::mem::size_of::<DROPFILES>();
    let mut bytes = vec![0u8; header];
    for path in paths {
        for unit in path.as_os_str().encode_wide() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes.extend_from_slice(&0u16.to_le_bytes());
    let drop = DROPFILES {
        pFiles: header as u32,
        fWide: BOOL(1),
        ..Default::default()
    };
    // SAFETY: `drop` is plain data copied over the zeroed header prefix.
    let raw =
        unsafe { std::slice::from_raw_parts((&drop as *const DROPFILES).cast::<u8>(), header) };
    bytes[..header].copy_from_slice(raw);
    bytes
}

/// Allocates `bytes` as a movable global block and gives it to the clipboard
/// under `format`. On success the clipboard owns the block; on failure it is
/// freed here.
unsafe fn set_bytes(format: u32, bytes: &[u8]) -> Result<(), PlatformError> {
    // SAFETY: `bytes.len()` is the requested size; a movable block is what the
    // clipboard accepts, and the caller owns it until it is handed over.
    let block = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?;
    // SAFETY: `block` is a live global block allocated just above.
    let pointer = unsafe { GlobalLock(block) };
    if pointer.is_null() {
        // SAFETY: the clipboard never received `block`, so it is still ours.
        let _ = unsafe { GlobalFree(Some(block)) };
        return Err(PlatformError::InvalidFormat("clipboard memory"));
    }
    // SAFETY: `block` is at least `bytes.len()` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
    }
    // SAFETY: `block` is locked and was fully written above.
    let _ = unsafe { GlobalUnlock(block) };
    // SAFETY: on success the clipboard owns `block`; on failure we still do.
    match unsafe { SetClipboardData(format, Some(HANDLE(block.0))) } {
        Ok(_) => Ok(()),
        Err(error) => {
            // SAFETY: `SetClipboardData` failed, so `block` is still ours.
            let _ = unsafe { GlobalFree(Some(block)) };
            Err(error.into())
        }
    }
}

fn preferred_effect_format() -> u32 {
    let name = wide(super::drop_target::PREFERRED_DROP_EFFECT);
    // SAFETY: `name` is NUL-terminated and outlives the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
}

unsafe fn read_files() -> Option<ClipboardFiles> {
    // SAFETY: the caller holds the clipboard open; the handle is owned by the
    // clipboard and remains valid until it is closed.
    let handle = unsafe { GetClipboardData(CF_HDROP.0 as u32) }.ok()?;
    if handle.0.is_null() {
        return None;
    }
    // SAFETY: the HDROP names a valid clipboard file list.
    let paths = unsafe { read_drop(HDROP(handle.0)) };
    // SAFETY: reading our own registered format from the open clipboard.
    let cut = unsafe { read_preferred_effect() }.is_some_and(|effect| effect == DROPEFFECT_MOVE);
    (!paths.is_empty()).then_some(ClipboardFiles { paths, cut })
}

unsafe fn read_drop(drop: HDROP) -> Vec<PathBuf> {
    // SAFETY: buffers are sized from DragQueryFileW's reported length plus NUL.
    unsafe {
        let count = DragQueryFileW(drop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = DragQueryFileW(drop, index, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            let copied = DragQueryFileW(drop, index, Some(&mut buffer)) as usize;
            paths.push(PathBuf::from(OsString::from_wide(&buffer[..copied])));
        }
        paths
    }
}

unsafe fn read_preferred_effect() -> Option<DROPEFFECT> {
    let format = preferred_effect_format();
    if format == 0 {
        return None;
    }
    // SAFETY: reading a registered format from the open clipboard.
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    if handle.0.is_null() {
        return None;
    }
    let block = HGLOBAL(handle.0);
    // SAFETY: the handle wraps a live global block while the clipboard is open.
    let pointer = unsafe { GlobalLock(block) };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the format holds a single DROPEFFECT (a u32).
    let effect = DROPEFFECT(unsafe { std::ptr::read_unaligned(pointer.cast::<u32>()) });
    // SAFETY: `block` is locked and the read above is complete.
    let _ = unsafe { GlobalUnlock(block) };
    Some(effect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropfiles_buffer_is_a_wide_double_null_terminated_list() {
        let paths = [PathBuf::from(r"C:\a.txt"), PathBuf::from(r"C:\b.txt")];
        let bytes = dropfiles_buffer(&paths);
        let header = std::mem::size_of::<DROPFILES>();

        assert_eq!(
            u32::from_le_bytes(bytes[..4].try_into().unwrap()),
            header as u32
        );
        // fWide is the final DWORD of the header.
        assert_eq!(
            i32::from_le_bytes(bytes[header - 4..header].try_into().unwrap()),
            1
        );

        let mut expected: Vec<u16> = Vec::new();
        for path in &paths {
            expected.extend(path.as_os_str().encode_wide());
            expected.push(0);
        }
        expected.push(0);
        let list: Vec<u16> = bytes[header..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        assert_eq!(list, expected);
    }

    #[test]
    fn clipboard_retry_recovers_from_transient_failures() {
        let mut attempts = 0;
        let result = with_clipboard_retry(|| {
            attempts += 1;
            if attempts < 3 { Err("busy") } else { Ok(42) }
        });
        assert_eq!(result, Ok(42));
        assert_eq!(attempts, 3);

        let mut attempts = 0;
        let result: Result<(), &str> = with_clipboard_retry(|| {
            attempts += 1;
            Err("held")
        });
        assert_eq!(result, Err("held"));
        assert_eq!(attempts, OPEN_ATTEMPTS);
    }
}
