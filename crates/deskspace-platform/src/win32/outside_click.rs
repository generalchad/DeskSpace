//! Reports mouse presses outside every space, so the app can clear an item
//! selection the way clicking elsewhere clears one in Explorer.
//!
//! Spaces are never activated, so they get no focus-loss message when the
//! user clicks the desktop or another app. A low-level mouse hook is the only
//! way to see those clicks. It runs for every mouse event in the session and
//! Windows skips it if this thread is slow to pump messages
//! (LowLevelHooksTimeout), so it is installed only while the app asks for
//! it, and the callback only posts the point to the controller, which does
//! the hit test.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc

use std::cell::Cell;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GetAncestor, GetClassNameW, HC_ACTION, HHOOK, MSLLHOOKSTRUCT,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_RBUTTONDOWN, WindowFromPoint,
};

use crate::win32::{desktop_shell, messages, runtime};

/// Window class of popup menus. Clicking one of DeskSpace's own menus (such
/// as an item's context menu) must not clear the selection it applies to.
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/about-window-classes#system-classes
const MENU_CLASS: &str = "#32768";

thread_local! {
    // Process state: the low-level mouse hook, while outside clicks are
    // watched. A hook procedure gets no context; `watch` installs it and
    // `stop` removes it.
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    // Process state: the controller window a press is reported to. A hook
    // procedure gets no context; `watch` sets it and `stop` clears it.
    static CONTROLLER: Cell<Option<HWND>> = const { Cell::new(None) };
}

/// Starts or stops reporting presses to `controller`. Idempotent.
pub(crate) fn watch(controller: HWND, enabled: bool) {
    if enabled == HOOK.get().is_some() {
        return;
    }
    if !enabled {
        stop();
        return;
    }
    CONTROLLER.set(Some(controller));
    // SAFETY: the hook procedure lives in this module for the life of the
    // process; low-level hooks run on the installing thread, which pumps
    // messages for the whole run.
    let hook = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), Some(HINSTANCE(module.0)), 0)
        })
    };
    match hook {
        Ok(hook) => HOOK.set(Some(hook)),
        Err(error) => tracing::warn!(%error, "could not watch for clicks outside spaces"),
    }
}

/// Removes the hook if it is installed.
/// Whether outside clicks are being watched now (the hook is installed).
pub(crate) fn is_watching() -> bool {
    HOOK.with(|hook| hook.get().is_some())
}

pub(crate) fn stop() {
    if let Some(hook) = HOOK.take() {
        // SAFETY: `hook` was returned by SetWindowsHookExW on this thread.
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let press = matches!(
        wparam.0 as u32,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
    );
    if code == HC_ACTION as i32 && press {
        // SAFETY: for HC_ACTION, lParam points at the event's MSLLHOOKSTRUCT.
        let point = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
        if let Some(controller) = CONTROLLER.get() {
            // SAFETY: posting to our own controller window.
            unsafe {
                let _ = PostMessageW(
                    Some(controller),
                    messages::OUTSIDE_PROBE,
                    WPARAM(point.x as isize as usize),
                    LPARAM(point.y as isize),
                );
            }
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
        String::from_utf16_lossy(&class[..length]) != MENU_CLASS
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
