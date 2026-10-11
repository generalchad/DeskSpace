//! Notification-area (tray) icon owned by the controller window.
//! https://learn.microsoft.com/en-us/windows/win32/shell/notification-area

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetSystemMetrics, HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW,
    LoadImageW, SM_CXSMICON, SM_CYSMICON,
};
use windows::core::{PCWSTR, Result};

use crate::win32::controller;

const TRAY_ID: u32 = 1;

/// The icon loaded from the executable's resource, created once and destroyed
/// in `remove`. Zero when not created yet or when the stock icon is in use
/// (shared icons must not be passed to DestroyIcon).
static OWNED_ICON: AtomicIsize = AtomicIsize::new(0);

fn tray_icon() -> HICON {
    let existing = OWNED_ICON.load(Ordering::SeqCst);
    if existing != 0 {
        return HICON(existing as *mut core::ffi::c_void);
    }
    match load_app_icon() {
        Ok(handle) => {
            OWNED_ICON.store(handle.0 as isize, Ordering::SeqCst);
            handle
        }
        Err(error) => {
            tracing::debug!(%error, "falling back to the stock application icon");
            // SAFETY: loading a shared stock icon.
            unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default()
        }
    }
}

/// `MAKEINTRESOURCE(1)`: the Win32 resource APIs take an integer id in place
/// of a name, encoded as the raw value `1`. It is deliberately the value 1 and
/// not an aligned dangling pointer.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-makeintresourcew
const ICON_RESOURCE: PCWSTR = PCWSTR(std::ptr::without_provenance::<u16>(1));

/// Loads icon resource 1 (embedded by the build script) at the small-icon
/// size. It is not `LR_SHARED`, so the returned handle is ours to destroy in
/// `remove`.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-loadimagew
fn load_app_icon() -> Result<HICON> {
    // SAFETY: the module handle is this process, and resource 1 is the icon the
    // build script embedded; the returned handle is a fresh, unshared icon.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let handle = LoadImageW(
            Some(instance.into()),
            ICON_RESOURCE,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )?;
        Ok(HICON(handle.0))
    }
}

fn data(window: HWND, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: TRAY_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: controller::TRAY_CALLBACK,
        hIcon: tray_icon(),
        ..Default::default()
    };
    for (slot, unit) in data.szTip.iter_mut().zip(tooltip.encode_utf16().take(127)) {
        *slot = unit;
    }
    data
}

/// Adds the icon. Also called when Explorer restarts (`TaskbarCreated`),
/// because the new taskbar starts with no icons.
pub(crate) fn add(window: HWND, tooltip: &str) {
    let mut data = data(window, tooltip);
    // SAFETY: `data` is a fully initialised NOTIFYICONDATAW for our window.
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
            tracing::warn!("could not add the tray icon");
            return;
        }
        // Version 4 reports the click position in wParam and sends
        // NIN_SELECT / WM_CONTEXTMENU instead of raw mouse messages.
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

pub(crate) fn set_tooltip(window: HWND, tooltip: &str) {
    let data = data(window, tooltip);
    // SAFETY: `data` is fully initialised.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

pub(crate) fn remove(window: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: TRAY_ID,
        ..Default::default()
    };
    // SAFETY: identifies the icon added by `add`.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
    let handle = OWNED_ICON.swap(0, Ordering::SeqCst);
    if handle != 0 {
        // SAFETY: the icon was loaded from our executable's resource, the tray
        // no longer shows it, and the settings window is destroyed before this.
        unsafe {
            let _ = DestroyIcon(HICON(handle as *mut core::ffi::c_void));
        }
    }
}

/// The tray's icon handle, for other windows to reuse (the settings window).
/// It is the same handle, not a copy, and the caller must not destroy it.
pub(crate) fn shared_icon() -> HICON {
    tray_icon()
}
