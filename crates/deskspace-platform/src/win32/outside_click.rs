//! Reports mouse presses outside every space, so the app can clear an item
//! selection the way clicking elsewhere clears one in Explorer.
//!
//! Spaces are never activated, so they get no focus-loss message when the
//! user clicks the desktop or another app. A low-level mouse hook is the only
//! way to see those clicks. It runs for every mouse event in the session, so
//! it is installed only while the app asks for it, and it runs on
//! `hook_thread`'s thread, not the UI thread: Windows drops a hook whose
//! thread is slow to answer (LowLevelHooksTimeout). The callback only posts
//! the point to the controller, which does the hit test on the UI thread.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GetAncestor, GetClassNameW, HC_ACTION, MSLLHOOKSTRUCT, PostMessageW,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WindowFromPoint,
};

use crate::win32::hook_thread::{self, Hook};
use crate::win32::{desktop_shell, messages, runtime};

/// Window class of popup menus. Clicking one of DeskSpace's own menus (such
/// as an item's context menu) must not clear the selection it applies to.
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/about-window-classes#system-classes
const MENU_CLASS: &str = "#32768";

// Process state: the controller window a press is reported to, as the
// handle's value, or 0. A hook procedure gets no context; `watch` sets it,
// and it stays set, since only a held hook reads it.
static CONTROLLER: AtomicIsize = AtomicIsize::new(0);

/// Starts or stops reporting presses to `controller`. Idempotent.
pub(crate) fn watch(controller: HWND, enabled: bool) {
    if enabled {
        CONTROLLER.store(controller.0 as isize, Ordering::SeqCst);
    }
    hook_thread::set(Hook::Mouse, enabled);
}

/// Whether outside clicks are being watched now (the hook is installed).
pub(crate) fn is_watching() -> bool {
    hook_thread::holds(Hook::Mouse)
}

/// Removes the hook if it is installed.
pub(crate) fn stop() {
    hook_thread::set(Hook::Mouse, false);
}

/// The hook procedure, which `hook_thread` installs and Windows calls on that
/// thread.
pub(crate) unsafe extern "system" fn hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let press = matches!(
        wparam.0 as u32,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
    );
    let controller = CONTROLLER.load(Ordering::SeqCst);
    if code == HC_ACTION as i32 && press && controller != 0 {
        // SAFETY: for HC_ACTION, lParam points at the event's MSLLHOOKSTRUCT.
        let point = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
        // SAFETY: posting copies plain values into our controller's queue,
        // from any thread; a window that is gone makes it fail, nothing more.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(controller as *mut core::ffi::c_void)),
                messages::OUTSIDE_PROBE,
                WPARAM(point.x as isize as usize),
                LPARAM(point.y as isize),
            );
        }
    }
    // SAFETY: passing the event on unchanged, as every hook must.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Whether a press at `point` (screen coordinates) landed outside every
/// space window, the desktop layer, and any popup menu.
pub(crate) fn is_outside(point: POINT) -> bool {
    // SAFETY: plain window queries; the class buffer is sized for the call.
    unsafe {
        let window = WindowFromPoint(point);
        if window.is_invalid() {
            return true;
        }
        // Hosted spaces are children of the desktop, so test the window
        // under the point itself as well as its top-level ancestor. A hosted
        // layer counts too: otherwise its root, the desktop, made the press
        // clear the selection before a Ctrl+click on the layer could add to
        // it, since the posted probe is handled before the click.
        let root = GetAncestor(window, GA_ROOT);
        if runtime::is_surface_window(window) || runtime::is_surface_window(root) {
            return false;
        }
        let mut class = [0u16; 16];
        let length = GetClassNameW(root, &mut class) as usize;
        if String::from_utf16_lossy(&class[..length]) == MENU_CLASS {
            return false;
        }
        // A press on the icon picker keeps the rename that opened it (D16).
        !crate::win32::icon_picker::is_picker(root)
    }
}

/// Whether a press at `point` (screen coordinates) landed on the desktop
/// background or its icon list, as opposed to another application. A press on
/// another app keeps the item selection, as it does on the desktop.
pub(crate) fn is_desktop(point: POINT) -> bool {
    // SAFETY: plain window queries.
    unsafe {
        let window = WindowFromPoint(point);
        if window.is_invalid() {
            return true;
        }
        // Desktop icons live in a child list view, so the top-level ancestor is
        // what identifies the desktop.
        let root = GetAncestor(window, GA_ROOT);
        desktop_shell::is_desktop_window(root) || desktop_shell::is_desktop_window(window)
    }
}
