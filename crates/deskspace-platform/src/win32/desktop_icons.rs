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

use windows::Win32::Foundation::{HWND, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::Controls::{LVM_CANCELEDITLABEL, LVM_GETEDITCONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetClassNameW, GetParent, SMTO_ABORTIFHUNG, SW_HIDE, SW_SHOW,
    SendMessageTimeoutW, ShowWindow, WM_GETTEXT,
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

// Process state: whether DeskSpace wants Explorer's icon layer hidden, so a
// re-hide is skipped while takeover is off. `hide` sets it and `restore`
// clears it; `rehide_if_active` only reads it.
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
    let command = if visible { SW_SHOW } else { SW_HIDE };
    // SAFETY: ShowWindow hides or shows Explorer's own listview, which
    // belongs to Explorer, not to this process.
    for_each_listview(|list| unsafe {
        let _ = ShowWindow(list, command);
    });
}

/// How long a message to Explorer's listview may take before it counts as
/// hung, in milliseconds.
const EXPLORER_TIMEOUT_MS: u32 = 200;

/// The label Explorer is renaming in a hidden desktop listview, if any, after
/// cancelling that rename. Explorer starts one after New › Folder (or any
/// New entry) and after its own Rename, but with its icons hidden the box is
/// invisible, so DeskSpace shows its own rename instead. Cancelling keeps the
/// item under the name it has.
///
/// The edit-control and cancel messages carry no pointers, so they work
/// across processes, and the system marshals `WM_GETTEXT`; `GetWindowTextW`
/// would read another process's caption instead of the edit's text.
/// <https://learn.microsoft.com/windows/win32/controls/lvm-geteditcontrol>
pub fn take_explorer_rename() -> Option<String> {
    let mut label = None;
    for_each_listview(|list| {
        if label.is_some() {
            return;
        }
        let Some(edit) = send_to_explorer(list, LVM_GETEDITCONTROL, 0, 0)
            .map(|result| HWND(result as *mut core::ffi::c_void))
            .filter(|edit| !edit.is_invalid())
        else {
            return;
        };
        // MAX_PATH: a desktop item's name, with its extension.
        let mut buffer = [0u16; 260];
        let length = send_to_explorer(edit, WM_GETTEXT, buffer.len(), buffer.as_mut_ptr() as isize)
            .unwrap_or(0)
            .min(buffer.len());
        label = Some(String::from_utf16_lossy(&buffer[..length]));
        let _ = send_to_explorer(list, LVM_CANCELEDITLABEL, 0, 0);
    });
    label.filter(|label| !label.is_empty())
}

/// Sends a message to one of Explorer's windows, giving up if Explorer is
/// hung. The result, or `None` when the send failed or timed out.
fn send_to_explorer(window: HWND, message: u32, wparam: usize, lparam: isize) -> Option<usize> {
    let mut result = 0;
    // SAFETY: `window` is a window from the enumeration (a stale handle only
    // fails the send); any pointer in `lparam` is the caller's live buffer,
    // sized by `wparam`, for the duration of this synchronous call.
    let sent = unsafe {
        SendMessageTimeoutW(
            window,
            message,
            WPARAM(wparam),
            LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            EXPLORER_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    (sent.0 != 0).then_some(result)
}

/// Calls `visit` with every desktop icon listview, under the desktop shell's
/// `SHELLDLL_DefView`s.
fn for_each_listview(mut visit: impl FnMut(HWND)) {
    let mut visit: &mut dyn FnMut(HWND) = &mut visit;
    // SAFETY: `visit_top_level` matches the `WNDENUMPROC` signature and only
    // reads the handles EnumWindows passes it. `lParam` points at `visit`,
    // which outlives the synchronous enumeration.
    unsafe {
        let _ = EnumWindows(
            Some(visit_top_level),
            LPARAM(&mut visit as *mut &mut dyn FnMut(HWND) as isize),
        );
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
    // SAFETY: `window` is a live descendant handle; the class and ancestor
    // reads only read. `param` is the visitor `for_each_listview` passed,
    // alive for the whole enumeration.
    unsafe {
        if class_is(window, LIST_VIEW) && has_def_view_ancestor(window) {
            let visit = &mut *(param.0 as *mut &mut dyn FnMut(HWND));
            visit(window);
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
