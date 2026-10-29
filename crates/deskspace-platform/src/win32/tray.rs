//! Notification-area (tray) icon owned by the controller window.
//! https://learn.microsoft.com/en-us/windows/win32/shell/notification-area

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetSystemMetrics, HICON, IDI_APPLICATION, LoadIconW, SM_CXSMICON,
};

use crate::win32::{app_icon, messages};

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
    // SAFETY: a plain metric query.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) };
    match app_icon::load(size) {
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

fn data(window: HWND, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: TRAY_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: messages::TRAY_CALLBACK,
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
        // SAFETY: the icon was loaded from our executable's resource for the
        // tray alone, and the tray no longer shows it.
        unsafe {
            let _ = DestroyIcon(HICON(handle as *mut core::ffi::c_void));
        }
    }
}
