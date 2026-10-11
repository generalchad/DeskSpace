//! Hiding and restoring Explorer's desktop icon layer for the desktop
//! takeover (ROADMAP D8).
//!
//! Explorer draws the desktop icons in a `SysListView32` inside a
//! `SHELLDLL_DefView` (one per monitor). DeskSpace hides only the listview,
//! never the `SHELLDLL_DefView`, so the window still exists: `find_host` keeps
//! recognizing the wallpaper `WorkerW`, and the shell view that `shell_view`
//! and `display_settings` query through COM is unaffected.
//!
//! The lookup is stateless: `hide` and `restore` re-enumerate every time, so a
//! listview Explorer recreates after a restart is found and restored rather
//! than a stale handle being shown. `HIDDEN` records only whether hiding is
//! currently wanted, so `rehide_if_active` is a no-op while takeover is off.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, SW_HIDE, SW_SHOW, ShowWindow,
};
use windows::core::{BOOL, PCWSTR, w};

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
    let visible = param.0 != 0;
    // SAFETY: `window` is a live handle supplied by EnumWindows; the child
    // lookups only read it, and `ShowWindow` hides or shows a listview that
    // belongs to Explorer, not to this process.
    unsafe {
        if let Ok(def_view) =
            FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), PCWSTR::null())
        {
            if !def_view.is_invalid() {
                if let Ok(view) =
                    FindWindowExW(Some(def_view), None, w!("SysListView32"), PCWSTR::null())
                {
                    if !view.is_invalid() {
                        let command = if visible { SW_SHOW } else { SW_HIDE };
                        let _ = ShowWindow(view, command);
                    }
                }
            }
        }
    }
    TRUE
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
