//! Hiding and restoring Explorer's desktop icon layer for the desktop
//! takeover (ROADMAP D8).
//!
//! Explorer draws the desktop icons in a `SysListView32` inside a
//! `SHELLDLL_DefView`. The view lives in the desktop shell (`Progman`, or a
//! wallpaper `WorkerW`), sometimes deeply nested, and there may be more than
//! one. DeskSpace walks the shell's whole window subtree and hides every
//! `SysListView32` that sits under a `SHELLDLL_DefView`, never the
//! `SHELLDLL_DefView` itself, so the window keeps existing: `find_host` keeps
//! recognizing the wallpaper `WorkerW`, and the shell view that `shell_view`
//! and `display_settings` query through COM is unaffected. Only the desktop
//! shell's subtree is walked, because an Explorer window also contains a
//! `SHELLDLL_DefView`; touching its list would hide the contents of an open
//! folder.
//!
//! The lookup is stateless: `hide` and `restore` re-enumerate every time, so a
//! listview Explorer recreates after a restart is found and restored rather
//! than a stale handle being shown. `HIDDEN` records only whether hiding is
//! currently wanted, so `rehide_if_active` is a no-op while takeover is off.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetParent, SW_HIDE, SW_SHOW, ShowWindow,
};
use windows::core::BOOL;

/// The desktop shell classes: Explorer hosts the icon view under `Progman` or a
/// wallpaper `WorkerW`, one of which may nest the other.
const PROGMAN: &str = "Progman";
const WORKER_W: &str = "WorkerW";
/// Explorer's shell view class; the `SysListView32` beneath it is the desktop
/// icon grid.
const DEF_VIEW: &str = "SHELLDLL_DefView";
const LIST_VIEW: &str = "SysListView32";

/// Whether DeskSpace is currently hiding Explorer's icon layer. Set by `hide`
/// and cleared by `restore`, so a re-hide is skipped when takeover is off.
static HIDDEN: AtomicBool = AtomicBool::new(false);

/// Hides the desktop listview of every monitor. Idempotent.
pub fn hide() {
    HIDDEN.store(true, Ordering::SeqCst);
    set_listviews_visible(false);
}

/// Shows every desktop listview again. Idempotent, and safe to call when
/// nothing is hidden (each startup calls it so a crash cannot leave the icons
/// gone).
pub fn restore() {
    HIDDEN.store(false, Ordering::SeqCst);
    set_listviews_visible(true);
}

/// Re-hides the listviews if takeover is active. Called after an Explorer
/// restart (`TaskbarCreated`) and on a low-frequency health timer, because
/// Explorer recreates its listview and would otherwise show its icons again.
/// A no-op while takeover is off.
pub fn rehide_if_active() {
    if HIDDEN.load(Ordering::SeqCst) {
        set_listviews_visible(false);
    }
}

fn set_listviews_visible(visible: bool) {
    // SAFETY: `visit_top_level` matches the `WNDENUMPROC` signature and only
    // reads the handles EnumWindows passes it. The `visible` flag rides in
    // `lParam`.
    unsafe {
        let _ = EnumWindows(Some(visit_top_level), LPARAM(visible as isize));
    }
}

unsafe extern "system" fn visit_top_level(window: HWND, param: LPARAM) -> BOOL {
    // Only the desktop shell hosts the icon view. An Explorer window is a
    // top-level of another class and also contains a `SHELLDLL_DefView`, so
    // recursing into every top-level window could hide an open folder's list.
    // SAFETY: `window` is a live handle from EnumWindows; the class read only
    // reads, and EnumChildWindows passes this subtree's live handles on.
    unsafe {
        if class_is(window, PROGMAN) || class_is(window, WORKER_W) {
            let _ = EnumChildWindows(Some(window), Some(visit_child), param);
        }
    }
    TRUE
}

unsafe extern "system" fn visit_child(window: HWND, param: LPARAM) -> BOOL {
    let visible = param.0 != 0;
    // SAFETY: `window` is a live descendant handle; the class and ancestor
    // reads only read, and ShowWindow hides or shows Explorer's own listview,
    // which belongs to Explorer, not to this process.
    unsafe {
        if class_is(window, LIST_VIEW) && has_def_view_ancestor(window) {
            let command = if visible { SW_SHOW } else { SW_HIDE };
            let _ = ShowWindow(window, command);
        }
    }
    TRUE
}

/// Whether any ancestor of `window` is a `SHELLDLL_DefView`, which marks the
/// listview as the desktop icon grid rather than some other list.
unsafe fn has_def_view_ancestor(window: HWND) -> bool {
    let mut current = window;
    // SAFETY: every `current` is a live window from the enumeration or a live
    // parent handle; GetParent and the class read only read.
    unsafe {
        while let Ok(parent) = GetParent(current) {
            if parent.is_invalid() {
                return false;
            }
            if class_is(parent, DEF_VIEW) {
                return true;
            }
            current = parent;
        }
    }
    false
}

/// Whether `window`'s class is exactly `name`.
unsafe fn class_is(window: HWND, name: &str) -> bool {
    let mut buffer = [0u16; 64];
    // SAFETY: `buffer` is writable and its length is passed to the call.
    let length = unsafe { GetClassNameW(window, &mut buffer) } as usize;
    if length == 0 || length > buffer.len() {
        return false;
    }
    String::from_utf16_lossy(&buffer[..length]) == name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rehide_without_hide_does_not_claim_active() {
        // `HIDDEN` is process-global, so only assert the default bookkeeping
        // rather than touching the real desktop.
        assert!(!HIDDEN.load(Ordering::SeqCst));
        rehide_if_active();
        assert!(!HIDDEN.load(Ordering::SeqCst));
    }
}
