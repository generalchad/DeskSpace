//! File operations performed by the shell's `IFileOperation`, the engine
//! behind Explorer's own copy, paste, and delete.
//!
//! Using it keeps DeskSpace's behavior close to Explorer's: the standard
//! progress dialog, long-path and cross-volume handling, and an undo record.
//! Name clashes are resolved with `FOF_RENAMEONCOLLISION` ("name (2).ext")
//! rather than the shell's Replace dialog, because every space shares one
//! storage folder and replacing would overwrite a file another space shows.
//! Delete uses the Recycle Bin (`FOF_ALLOWUNDO`) with no prompt.
//! https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOCONFIRMMKDIR, FOF_RENAMEONCOLLISION, IFileOperation,
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{GUID, HRESULT, PCWSTR, Ref, Result as WinResult};

use crate::error::PlatformError;
use crate::win32::system::wide;

/// `CLSID_FileOperation`; windows-rs does not export it, so the documented GUID
/// is spelled out.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation
const CLSID_FILE_OPERATION: GUID = GUID::from_u128(0x3ad05575_8857_4850_9277_11b85bdb8e09);

/// Moves or copies `paths` into `destination`, returning where the files
/// landed. `owner` parents the shell's progress and conflict UI.
pub(crate) fn transfer(
    owner: Option<HWND>,
    paths: &[PathBuf],
    destination: &Path,
    move_items: bool,
) -> Result<Vec<PathBuf>, PlatformError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let created = Arc::new(Mutex::new(Vec::new()));
    let sink: IFileOperationProgressSink = CreatedItems {
        paths: created.clone(),
    }
    .into();
    // SAFETY: COM is initialized on this UI thread (`OleInitialize`).
    // `PerformOperations` pumps messages, so this must not be called while a
    // runtime or handler borrow is held; the caller releases both first.
    unsafe {
        let operation: IFileOperation = CoCreateInstance(&CLSID_FILE_OPERATION, None, CLSCTX_ALL)?;
        if let Some(owner) = owner {
            let _ = operation.SetOwnerWindow(owner);
        }
        operation.SetOperationFlags(FOF_RENAMEONCOLLISION | FOF_NOCONFIRMMKDIR | FOF_ALLOWUNDO)?;
        let destination = shell_item(destination)?;
        let mut queued = false;
        for path in paths {
            let Ok(source) = shell_item(path) else {
                tracing::warn!(path = %path.display(), "pasted path is not a shell item");
                continue;
            };
            let result = if move_items {
                operation.MoveItem(&source, &destination, PCWSTR::null(), None)
            } else {
                operation.CopyItem(&source, &destination, PCWSTR::null(), None)
            };
            match result {
                Ok(()) => queued = true,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "could not queue a file operation");
                }
            }
        }
        // An operation with nothing queued reports a failure, so skip it.
        if queued {
            let cookie = operation.Advise(Some(&sink))?;
            let outcome = operation.PerformOperations();
            let _ = operation.Unadvise(cookie);
            // Keep whatever was created even if the operation failed partway,
            // so the caller can still list the files that did land.
            if let Err(error) = outcome {
                tracing::warn!(%error, "the file operation failed");
            }
            if matches!(operation.GetAnyOperationsAborted(), Ok(aborted) if aborted.as_bool()) {
                tracing::warn!("the file operation was cancelled");
            }
        }
    }
    created
        .lock()
        .map(|paths| paths.clone())
        .map_err(|_| PlatformError::InvalidFormat("file operation result"))
}

/// Sends `paths` to the Recycle Bin, without a prompt, recording an undo.
/// Explorer's Delete key does the same.
pub(crate) fn delete(owner: Option<HWND>, paths: &[PathBuf]) -> Result<(), PlatformError> {
    if paths.is_empty() {
        return Ok(());
    }
    // SAFETY: COM is initialized on this UI thread; `PerformOperations` pumps
    // messages, so no runtime or handler borrow is held across it.
    unsafe {
        let operation: IFileOperation = CoCreateInstance(&CLSID_FILE_OPERATION, None, CLSCTX_ALL)?;
        if let Some(owner) = owner {
            let _ = operation.SetOwnerWindow(owner);
        }
        // ALLOWUNDO sends to the Recycle Bin; NOCONFIRMATION skips the prompt.
        operation.SetOperationFlags(FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOCONFIRMMKDIR)?;
        let mut queued = false;
        for path in paths {
            let Ok(item) = shell_item(path) else {
                tracing::warn!(path = %path.display(), "deleted path is not a shell item");
                continue;
            };
            match operation.DeleteItem(&item, None) {
                Ok(()) => queued = true,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "could not queue a delete");
                }
            }
        }
        // An operation with nothing queued reports a failure, so skip it.
        if queued {
            operation.PerformOperations()?;
            if matches!(operation.GetAnyOperationsAborted(), Ok(aborted) if aborted.as_bool()) {
                tracing::warn!("the delete was cancelled");
            }
        }
    }
    Ok(())
}

/// Renames `path` to `new_name` (a file name, not a path) in place, recording
/// an undo. The shell shows its conflict prompt if the name is taken.
pub(crate) fn rename(
    owner: Option<HWND>,
    path: &Path,
    new_name: &str,
) -> Result<(), PlatformError> {
    let wide_name = wide(new_name);
    // SAFETY: COM is initialized on this UI thread; `PerformOperations` pumps
    // messages, so no runtime or handler borrow is held across it.
    unsafe {
        let operation: IFileOperation = CoCreateInstance(&CLSID_FILE_OPERATION, None, CLSCTX_ALL)?;
        if let Some(owner) = owner {
            let _ = operation.SetOwnerWindow(owner);
        }
        operation.SetOperationFlags(FOF_ALLOWUNDO)?;
        let item = shell_item(path)?;
        operation.RenameItem(&item, PCWSTR(wide_name.as_ptr()), None)?;
        operation.PerformOperations()?;
        if matches!(operation.GetAnyOperationsAborted(), Ok(aborted) if aborted.as_bool()) {
            tracing::warn!("the rename was cancelled");
        }
    }
    Ok(())
}

fn shell_item(path: &Path) -> Result<IShellItem, PlatformError> {
    let wide = wide(&path.to_string_lossy().replace('/', "\\"));
    // SAFETY: `wide` is NUL-terminated and outlives the parse.
    Ok(unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)? })
}

fn filesystem_path(item: &IShellItem) -> Option<PathBuf> {
    // SAFETY: the returned string is copied then freed with the shell's
    // documented allocator.
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let text = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
        text.filter(|text| !text.is_empty()).map(PathBuf::from)
    }
}

/// Collects the destination of every item the operation creates.
#[windows_core::implement(IFileOperationProgressSink)]
struct CreatedItems {
    paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl CreatedItems {
    fn record(&self, item: Ref<IShellItem>) {
        let Some(path) = item.as_ref().and_then(filesystem_path) else {
            return;
        };
        if let Ok(mut paths) = self.paths.lock() {
            paths.push(path);
        }
    }
}

impl IFileOperationProgressSink_Impl for CreatedItems_Impl {
    fn PostCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        hrcopy: HRESULT,
        psinewlycreated: Ref<IShellItem>,
    ) -> WinResult<()> {
        if hrcopy.is_ok() {
            self.record(psinewlycreated);
        }
        Ok(())
    }

    fn PostMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        hrmove: HRESULT,
        psinewlycreated: Ref<IShellItem>,
    ) -> WinResult<()> {
        if hrmove.is_ok() {
            self.record(psinewlycreated);
        }
        Ok(())
    }

    fn StartOperations(&self) -> WinResult<()> {
        Ok(())
    }

    fn FinishOperations(&self, _hrresult: HRESULT) -> WinResult<()> {
        Ok(())
    }

    fn PreRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PostRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _hrrename: HRESULT,
        _psinewlycreated: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PreMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PreCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PreDeleteItem(&self, _dwflags: u32, _psiitem: Ref<IShellItem>) -> WinResult<()> {
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<IShellItem>,
        _hrdelete: HRESULT,
        _psinewlycreated: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PreNewItem(
        &self,
        _dwflags: u32,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }

    fn PostNewItem(
        &self,
        _dwflags: u32,
        _psidestinationfolder: Ref<IShellItem>,
        _psznewname: &PCWSTR,
        _psztemplatename: &PCWSTR,
        _dwfileattributes: u32,
        _hrnew: HRESULT,
        _psinewitem: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }

    fn UpdateProgress(&self, _iworktotal: u32, _iworksofar: u32) -> WinResult<()> {
        Ok(())
    }

    fn ResetTimer(&self) -> WinResult<()> {
        Ok(())
    }

    fn PauseTimer(&self) -> WinResult<()> {
        Ok(())
    }

    fn ResumeTimer(&self) -> WinResult<()> {
        Ok(())
    }
}
