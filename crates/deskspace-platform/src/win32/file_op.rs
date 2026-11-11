//! File operations performed by the shell's `IFileOperation`, the engine
//! behind Explorer's own copy, paste, and delete.
//!
//! Using it keeps DeskSpace's behavior close to Explorer's: the standard
//! progress dialog, long-path and cross-volume handling, and an undo record.
//! A caller picks how name clashes are resolved (`OnCollision`): storage
//! renames them ("name (2).ext") because every space shares one folder and
//! replacing would overwrite a file another space shows, while a lens's real
//! folder gets the shell's Replace or Skip dialog.
//! Delete uses the Recycle Bin (`FOF_ALLOWUNDO`) with no prompt.
//! https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation
//!
//! DeskSpace never holds the foreground, so the shell opens its progress and
//! conflict window behind the app that holds it. Every operation here therefore
//! runs under a `RaiseOperationWindows`, which brings that window to the
//! front.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{HWND, LPARAM, PROPERTYKEY, TRUE};
use windows::Win32::Storage::FileSystem::GetLongPathNameW;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, BHID_SFUIObject, CMF_NORMAL, CMINVOKECOMMANDINFO, FILEOPERATION_FLAGS,
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOCONFIRMMKDIR, FOF_RENAMEONCOLLISION,
    FOLDERID_RecycleBinFolder, IContextMenu, IEnumShellItems, IFileOperation,
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem, IShellItem2,
    KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI, SHCreateItemFromParsingName, SHGetKnownFolderItem,
    SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, EnumWindows, GWL_EXSTYLE, GetClassNameW, GetWindowLongW,
    GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible, KillTimer, SW_SHOWNORMAL,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowPos, WINDOW_EX_STYLE, WS_EX_TOPMOST,
};
use windows::core::{BOOL, GUID, HRESULT, Interface, PCWSTR, Ref, Result as WinResult, s};

use crate::error::PlatformError;
use crate::win32::shortcuts::display_name;
use crate::win32::system::wide;

/// `CLSID_FileOperation`; windows-rs does not export it, so the documented GUID
/// is spelled out.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation
const CLSID_FILE_OPERATION: GUID = GUID::from_u128(0x3ad05575_8857_4850_9277_11b85bdb8e09);

/// `System.Recycle.DeletedFrom` (`SCID_OriginalLocation`, `PID_DISPLACED_FROM`):
/// the folder from which a Recycle Bin item was deleted.
/// https://learn.microsoft.com/en-us/windows/win32/properties/props-system-recycle-deletedfrom
const PKEY_RECYCLE_DELETED_FROM: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
    pid: 2,
};

/// `System.Recycle.DateDeleted` (`SCID_DateDeleted`, `PID_DISPLACED_DATE`):
/// when a Recycle Bin item was deleted, as a UTC `FILETIME`.
/// https://learn.microsoft.com/en-us/windows/win32/properties/props-system-recycle-datedeleted
const PKEY_RECYCLE_DATE_DELETED: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
    pid: 3,
};

/// Seconds between the Windows `FILETIME` epoch (1601-01-01) and `UNIX_EPOCH`
/// (1970-01-01).
const WINDOWS_TO_UNIX_EPOCH_SECS: u64 = 11_644_473_600;

/// The class of the window the shell shows for a file operation: its progress,
/// and the "Replace or Skip Files" and error prompts that window turns into.
/// Windows documents the window only as an interface, which names no class,
/// so this is the name `GetClassNameW` gives for a live one (Windows 11). A
/// Windows that names it otherwise leaves it where the shell put it.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ioperationsprogressdialog
const OPERATION_STATUS_CLASS: &str = "OperationStatusWindow";

/// How often a running operation looks for its window. A prompt nobody can
/// see reads as a frozen DeskSpace, so it should come forward before the
/// user wonders. One look measured about half a millisecond with 500
/// top-level windows open, so ten a second leave the UI thread its budget.
const RAISE_INTERVAL_MS: u32 = 100;

/// Whether a top-level window is the shell's file-operation window for an
/// operation this process started. The process id keeps Explorer's own, and
/// every other app's, where their user left them.
fn is_our_operation_window(class: &str, window_pid: u32, our_pid: u32) -> bool {
    window_pid == our_pid && class == OPERATION_STATUS_CLASS
}

/// Brings the shell's window for a file operation to the front for as long as
/// this lives, so hold one across every call that can show it.
///
/// A thread timer looks every `RAISE_INTERVAL_MS`: the operation pumps this
/// thread's messages while it works, and a thread timer has no window and a
/// system-chosen id, so it takes none of the app's timer ids on the
/// controller.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer
struct RaiseOperationWindows {
    /// Zero when Windows refused the timer; the operation then runs as it
    /// did before, with its window wherever the shell put it.
    timer: usize,
}

impl RaiseOperationWindows {
    fn arm() -> Self {
        // SAFETY: `raise_on_timer` matches TIMERPROC and lives forever.
        let timer = unsafe { SetTimer(None, 0, RAISE_INTERVAL_MS, Some(raise_on_timer)) };
        if timer == 0 {
            tracing::warn!("no timer to bring the file operation's window forward");
        }
        Self { timer }
    }
}

impl Drop for RaiseOperationWindows {
    fn drop(&mut self) {
        if self.timer != 0 {
            // SAFETY: kills the thread timer `arm` made, on the thread that
            // made it: a guard is a local of the function that armed it.
            unsafe {
                let _ = KillTimer(None, self.timer);
            }
        }
    }
}

unsafe extern "system" fn raise_on_timer(_: HWND, _: u32, _: usize, _: u32) {
    raise_operation_windows();
}

/// Makes each of this process's shown file-operation windows topmost, once.
///
/// It looks at every top-level window, not this thread's: the shell runs the
/// window on a thread of its own. It skips a hidden one, because the shell
/// makes the window at once and shows it only when an operation runs long or
/// needs an answer, so showing it here would flash a progress box over every
/// quick delete. Topmost is the one way forward: without the foreground,
/// `SetForegroundWindow` only flashes a taskbar button.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows
fn raise_operation_windows() {
    // SAFETY: `raise_if_ours` matches WNDENUMPROC and uses no context.
    unsafe {
        let _ = EnumWindows(Some(raise_if_ours), LPARAM(0));
    }
}

unsafe extern "system" fn raise_if_ours(window: HWND, _: LPARAM) -> BOOL {
    let mut window_pid = 0;
    let mut class = [0u16; 64];
    // SAFETY: `window` is a live handle from EnumWindows, and each read
    // writes only to a local sized for it.
    let (length, shown, style) = unsafe {
        GetWindowThreadProcessId(window, Some(&mut window_pid));
        (
            GetClassNameW(window, &mut class).max(0) as usize,
            IsWindowVisible(window).as_bool(),
            WINDOW_EX_STYLE(GetWindowLongW(window, GWL_EXSTYLE) as u32),
        )
    };
    let class = String::from_utf16_lossy(&class[..length.min(class.len())]);
    // SAFETY: reading this process's id has no preconditions.
    let ours = is_our_operation_window(&class, window_pid, unsafe { GetCurrentProcessId() });
    // A window already topmost was raised by an earlier look, so each one is
    // raised once and never fights the user for its place afterwards.
    if ours && shown && !style.contains(WS_EX_TOPMOST) {
        // SAFETY: the window belongs to this process. No size, place, or
        // activation changes, so no space gains the focus and the foreground
        // app keeps its own.
        let raised = unsafe {
            SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        // `debug` for the failure too: it would repeat on every look.
        match raised {
            Ok(()) => tracing::debug!("brought the file operation's window to the front"),
            Err(error) => tracing::debug!(%error, "could not raise the file operation's window"),
        }
    }
    TRUE
}

/// What `transfer` does when a file with the same name is already in the
/// destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnCollision {
    /// Give the newcomer a free name ("name (2).ext"). Storage uses this,
    /// because every space shares the folder.
    Rename,
    /// Show the shell's Replace or Skip dialog, as Explorer does in a
    /// user's own folder.
    Ask,
}

/// The flags `transfer` sets: both keep `FOF_ALLOWUNDO`, so Ctrl+Z in
/// Explorer undoes the operation.
fn operation_flags(on_collision: OnCollision) -> FILEOPERATION_FLAGS {
    let base = FOF_NOCONFIRMMKDIR | FOF_ALLOWUNDO;
    match on_collision {
        OnCollision::Rename => base | FOF_RENAMEONCOLLISION,
        OnCollision::Ask => base,
    }
}

/// Moves or copies `paths` into `destination`, returning where the files
/// landed. `owner` parents the shell's progress and conflict UI.
pub(crate) fn transfer(
    owner: Option<HWND>,
    paths: &[PathBuf],
    destination: &Path,
    move_items: bool,
    on_collision: OnCollision,
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
        operation.SetOperationFlags(operation_flags(on_collision))?;
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
            let outcome = {
                let _in_front = RaiseOperationWindows::arm();
                operation.PerformOperations()
            };
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

/// Deletes `paths` with the shell's file operation, without a prompt. Unless
/// `permanent` is set they go to the Recycle Bin and the operation records an
/// undo, as Explorer's Delete key does; `permanent` deletes them for good, as
/// Shift+Delete does.
pub(crate) fn delete(
    owner: Option<HWND>,
    paths: &[PathBuf],
    permanent: bool,
) -> Result<(), PlatformError> {
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
        // ALLOWUNDO sends to the Recycle Bin; without it the files are removed
        // for good. NOCONFIRMATION skips the prompt.
        let flags = if permanent {
            FOF_NOCONFIRMATION | FOF_NOCONFIRMMKDIR
        } else {
            FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOCONFIRMMKDIR
        };
        operation.SetOperationFlags(flags)?;
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
            let _in_front = RaiseOperationWindows::arm();
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
        let _in_front = RaiseOperationWindows::arm();
        operation.PerformOperations()?;
        if matches!(operation.GetAnyOperationsAborted(), Ok(aborted) if aborted.as_bool()) {
            tracing::warn!("the rename was cancelled");
        }
    }
    Ok(())
}

/// Finds the Recycle Bin item whose original location is `original` and that
/// was deleted at or after `since` (the newest if several), restores it with
/// the shell's `"undelete"` verb, and returns the restored path. Returns
/// `None` when no matching item is still in the Recycle Bin.
/// https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid (FOLDERID_RecycleBinFolder)
pub(crate) fn restore_recycled(
    owner: Option<HWND>,
    original: &Path,
    since: SystemTime,
) -> Option<PathBuf> {
    // Recycle Bin deletion timestamps are stored at whole-second granularity by
    // the shell's property store, so allow a small slack before `since` (which
    // the caller captures right before `delete`).
    let cutoff = filetime_ticks(since.checked_sub(Duration::from_secs(2)).unwrap_or(since));
    let target = long_path(original);
    // SAFETY: COM is initialized on this thread; every interface and string is
    // released when it goes out of scope.
    unsafe {
        let bin =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)
                .ok()?;
        let items = bin
            .BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems)
            .ok()?;
        let mut newest: Option<(u64, IShellItem)> = None;
        loop {
            let mut batch = [None];
            let mut fetched = 0u32;
            if items.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = batch[0].take() else {
                continue;
            };
            let Some((origin, deleted_at)) = recycled_origin_and_time(&item) else {
                continue;
            };
            if deleted_at >= cutoff
                && same_path(&long_path(&origin), &target)
                && newest.as_ref().is_none_or(|(best, _)| deleted_at >= *best)
            {
                newest = Some((deleted_at, item));
            }
        }
        let (_, item) = newest?;
        invoke_undelete(owner, &item)?;
    }
    original.exists().then(|| original.to_path_buf())
}

/// Reads a Recycle Bin item's original full path and deletion timestamp (in
/// `FILETIME` 100-ns ticks).
unsafe fn recycled_origin_and_time(item: &IShellItem) -> Option<(PathBuf, u64)> {
    let item2: IShellItem2 = item.cast().ok()?;
    // SAFETY: `GetString` returns a CoTaskMem-allocated wide string that is
    // copied and freed below; `GetFileTime` writes a plain `FILETIME`.
    let (folder, deleted) = unsafe {
        let raw = item2.GetString(&PKEY_RECYCLE_DELETED_FROM).ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        let folder = text.filter(|text| !text.is_empty()).map(PathBuf::from)?;
        let deleted = item2.GetFileTime(&PKEY_RECYCLE_DATE_DELETED).ok()?;
        (folder, deleted)
    };
    let ticks = (u64::from(deleted.dwHighDateTime) << 32) | u64::from(deleted.dwLowDateTime);
    let name = display_name(item, SIGDN_NORMALDISPLAY)?;
    let mut origin = folder.join(&name);
    // When Explorer's "Hide extensions for known file types" is on,
    // `SIGDN_NORMALDISPLAY` omits the extension while the `$R` backing file in
    // `SIGDN_FILESYSPATH` keeps it.
    if origin.extension().is_none()
        && let Some(ext) = filesystem_path(item)
            .as_deref()
            .and_then(Path::extension)
            .map(ToOwned::to_owned)
    {
        origin.set_extension(ext);
    }
    Some((origin, ticks))
}

/// Runs the shell's `"undelete"` context-menu verb on `item`.
unsafe fn invoke_undelete(owner: Option<HWND>, item: &IShellItem) -> Option<()> {
    // SAFETY: `item` is a live shell item; the temporary popup menu is
    // destroyed before returning. `QueryContextMenu` is called first because
    // the Recycle Bin context-menu handler populates its verb table there.
    unsafe {
        let menu: IContextMenu = item.BindToHandler(None, &BHID_SFUIObject).ok()?;
        let popup = CreatePopupMenu().ok()?;
        let _ = menu.QueryContextMenu(popup, 0, 1, 0x7FFF, CMF_NORMAL);
        let info = CMINVOKECOMMANDINFO {
            cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
            fMask: SEE_MASK_FLAG_NO_UI,
            hwnd: owner.unwrap_or_default(),
            lpVerb: s!("undelete"),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        let result = {
            let _in_front = RaiseOperationWindows::arm();
            menu.InvokeCommand(&info)
        };
        let _ = DestroyMenu(popup);
        if let Err(error) = result {
            tracing::warn!(%error, "could not restore item from the Recycle Bin");
            return None;
        }
    }
    Some(())
}

/// Converts a `SystemTime` to Windows `FILETIME` 100-ns ticks since 1601-01-01.
fn filetime_ticks(time: SystemTime) -> u64 {
    let since_unix = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = since_unix
        .as_secs()
        .saturating_add(WINDOWS_TO_UNIX_EPOCH_SECS);
    secs.saturating_mul(10_000_000)
        .saturating_add(u64::from(since_unix.subsec_nanos() / 100))
}

/// Expands 8.3 short path segments (such as `TIMOTH~1` in `%TEMP%`) on `path`'s
/// existing parent folder so it compares equal to the long path the Recycle Bin
/// records in `System.Recycle.DeletedFrom`.
fn long_path(path: &Path) -> PathBuf {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    let wide_parent = wide(&parent.to_string_lossy().replace('/', "\\"));
    let mut buffer = vec![0u16; 1024];
    // SAFETY: `wide_parent` is NUL-terminated and `buffer` bounds the write.
    let len = unsafe { GetLongPathNameW(PCWSTR(wide_parent.as_ptr()), Some(&mut buffer)) } as usize;
    if len > 0 && len < buffer.len() {
        PathBuf::from(String::from_utf16_lossy(&buffer[..len])).join(name)
    } else {
        path.to_path_buf()
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{LRESULT, WPARAM};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, WINDOW_STYLE, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };
    use windows::core::w;

    use crate::win32::window_class::{self, WindowClass};

    fn has(flags: FILEOPERATION_FLAGS, flag: FILEOPERATION_FLAGS) -> bool {
        flags.0 & flag.0 == flag.0
    }

    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwards Windows' own arguments unchanged.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    /// A window of `class` far off screen that never takes the focus, so a
    /// test run shows nothing and steals nothing.
    fn off_screen_window(class: PCWSTR, style: WINDOW_STYLE) -> HWND {
        // SAFETY: `class` names a registered class and outlives the call.
        unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("file operation test"),
                style,
                -30000,
                -30000,
                100,
                100,
                None,
                None,
                None,
                None,
            )
        }
        .expect("a test window")
    }

    fn is_topmost(window: HWND) -> bool {
        // SAFETY: reads a style of a window this test made.
        let style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
        WINDOW_EX_STYLE(style).contains(WS_EX_TOPMOST)
    }

    #[test]
    fn only_this_processs_operation_window_is_ours() {
        assert!(is_our_operation_window(OPERATION_STATUS_CLASS, 7, 7));
        assert!(
            !is_our_operation_window(OPERATION_STATUS_CLASS, 8, 7),
            "Explorer's own copy window stays where its user left it"
        );
        assert!(!is_our_operation_window("DeskSpaceSpace", 7, 7));
        assert!(!is_our_operation_window("", 7, 7));
    }

    #[test]
    fn the_operation_class_compares_exactly() {
        for near in [
            "operationstatuswindow",
            "OperationStatusWindow2",
            "OperationStatus",
            " OperationStatusWindow",
        ] {
            assert!(!is_our_operation_window(near, 7, 7), "{near:?}");
        }
    }

    #[test]
    fn a_shown_operation_window_is_raised_and_a_hidden_one_is_left() {
        // A class of the shell's name, registered for this executable: the
        // shell's own belongs to its DLL, so the two never meet.
        let name = wide(OPERATION_STATUS_CLASS);
        let class = PCWSTR(name.as_ptr());
        window_class::register(WindowClass::new(class, Some(procedure))).expect("the class");
        let shown = off_screen_window(class, WS_POPUP | WS_VISIBLE);
        let hidden = off_screen_window(class, WS_POPUP);
        let bystander = off_screen_window(w!("Static"), WS_POPUP | WS_VISIBLE);

        raise_operation_windows();
        assert!(is_topmost(shown), "a shown prompt comes to the front");
        assert!(
            !is_topmost(hidden),
            "the shell has not shown this one, so neither do we"
        );
        // SAFETY: reads the visibility of a window this test made.
        assert!(!unsafe { IsWindowVisible(hidden) }.as_bool());
        assert!(!is_topmost(bystander), "another class is not ours to move");

        // SAFETY: destroys the windows this test made, on their own thread.
        unsafe {
            for window in [shown, hidden, bystander] {
                let _ = DestroyWindow(window);
            }
        }
    }

    #[test]
    fn the_raise_timer_ends_with_its_guard() {
        let guard = RaiseOperationWindows::arm();
        let timer = guard.timer;
        assert_ne!(timer, 0, "Windows gave a thread timer");
        drop(guard);
        // SAFETY: asks to kill a timer id on this thread; a dead id only fails.
        let killed_again = unsafe { KillTimer(None, timer) };
        assert!(killed_again.is_err(), "the guard already killed it");
    }

    #[test]
    fn recycles_and_restores_a_file() {
        // SAFETY: initializes COM for this test thread only.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let folder = std::env::temp_dir().join(format!(
            "deskspace-recycle-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).expect("create temp folder");
        let file = folder.join("restore-me.txt");
        std::fs::write(&file, "hello recycle bin").expect("write test file");

        let since = SystemTime::now();
        delete(None, std::slice::from_ref(&file), false).expect("recycle file");
        assert!(!file.exists(), "file moved to the Recycle Bin");

        let restored = restore_recycled(None, &file, since).expect("restored from Recycle Bin");
        assert_eq!(restored, file);
        assert_eq!(
            std::fs::read_to_string(&file).expect("read restored file"),
            "hello recycle bin"
        );
        assert_eq!(
            restore_recycled(None, &file, since),
            None,
            "already restored"
        );

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn rename_resolves_clashes_and_ask_leaves_them_to_the_shell() {
        let rename = operation_flags(OnCollision::Rename);
        let ask = operation_flags(OnCollision::Ask);
        assert!(has(rename, FOF_RENAMEONCOLLISION));
        assert!(!has(ask, FOF_RENAMEONCOLLISION));
    }

    #[test]
    fn both_keep_the_explorer_undo() {
        for on_collision in [OnCollision::Rename, OnCollision::Ask] {
            assert!(has(operation_flags(on_collision), FOF_ALLOWUNDO));
        }
    }
}
