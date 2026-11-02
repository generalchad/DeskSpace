//! Shared access to the shell's desktop view.
//!
//! The desktop's `IShellView` is reached through the shell's window enumerator
//! (`IShellWindows`), the top-level-browser service, and the browser's active
//! view. Several shell features need it (clearing the selection, reading the
//! view's display flags), so the lookup and its cached enumerator live here.
//! https://devblogs.microsoft.com/oldnewthing/20130318-00/?p=4933

use std::cell::RefCell;

use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, IServiceProvider};
use windows::Win32::System::Variant::{VARIANT, VT_I4, VariantInit};
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellView, IShellWindows, SID_STopLevelBrowser, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    ShellWindows,
};
use windows::core::Interface;

thread_local! {
    // Process state: the cached shell-window enumerator, shared by every lookup
    // on the OLE thread. Created on first use and released by `release` before
    // OLE is uninitialized.
    static SHELL_WINDOWS: RefCell<Option<IShellWindows>> = const { RefCell::new(None) };
}

/// `CSIDL_DESKTOP`: the virtual folder `FindWindowSW` wants for the desktop.
const CSIDL_DESKTOP: i32 = 0;

/// The shell view that draws the desktop's icons, or `None` if the shell is
/// not available.
pub(crate) fn desktop_shell_view() -> Option<IShellView> {
    if let Some(view) = shell_windows().and_then(|windows| view_from(&windows)) {
        return Some(view);
    }
    // The cached enumerator is a proxy into Explorer, so it dies when
    // Explorer restarts and every later lookup would fail. Retry once with a
    // fresh one.
    release();
    shell_windows().and_then(|windows| view_from(&windows))
}

fn view_from(windows: &IShellWindows) -> Option<IShellView> {
    let mut window = 0i32;
    let location = i4_variant(CSIDL_DESKTOP);
    // The second location is reserved and must be VT_EMPTY.
    let empty = empty_variant();
    // SAFETY: the variants are initialised; the returned dispatch is owned and
    // released when it drops.
    let dispatch = unsafe {
        windows.FindWindowSW(
            &location,
            &empty,
            SWC_DESKTOP,
            &mut window,
            SWFO_NEEDDISPATCH,
        )
    }
    .ok()?;
    // The desktop exposes its browser through the top-level-browser service.
    let browser = dispatch
        .cast::<IServiceProvider>()
        .ok()
        .and_then(|provider| {
            // SAFETY: querying a documented service from a live provider.
            unsafe { provider.QueryService::<IShellBrowser>(&SID_STopLevelBrowser) }.ok()
        })
        .or_else(|| dispatch.cast::<IShellBrowser>().ok())?;
    // SAFETY: querying the active view of a live browser.
    unsafe { browser.QueryActiveShellView() }.ok()
}

/// Releases the cached shell-window enumerator; call before `OleUninitialize`.
pub(crate) fn release() {
    SHELL_WINDOWS.with(|cell| cell.borrow_mut().take());
}

fn shell_windows() -> Option<IShellWindows> {
    SHELL_WINDOWS.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            // SAFETY: creating the shell's window enumerator on the OLE thread.
            *slot = unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) }
                .map_err(|error| tracing::debug!(%error, "no shell windows enumerator"))
                .ok();
        }
        slot.clone()
    })
}

fn empty_variant() -> VARIANT {
    // SAFETY: VariantInit zeroes an uninitialised VARIANT.
    unsafe { VariantInit() }
}

fn i4_variant(value: i32) -> VARIANT {
    let mut variant = empty_variant();
    // SAFETY: tagging the union as a 4-byte integer and writing it. The
    // explicit deref writes through the ManuallyDrop instead of dropping it.
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_I4;
        inner.Anonymous.lVal = value;
    }
    variant
}
