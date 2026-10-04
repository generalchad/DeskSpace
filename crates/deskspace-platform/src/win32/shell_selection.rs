//! Makes a click on a space behave like a click on the desktop.
//!
//! Spaces never take activation, so clicking one leaves the window in front
//! active and its selection highlighted, unlike a click on the real desktop.
//! To mirror native behavior the desktop is made the active window (which
//! deactivates whatever was in front) and its icon selection is cleared. A
//! selection in an Explorer window is deliberately left alone: native Windows
//! only changes a selection within the view that was clicked.

use windows::Win32::UI::Shell::SVSI_DESELECTOTHERS;
use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, SetForegroundWindow};

use crate::win32::shell_view;

/// Mirrors a click on the desktop: focus it, then clear its icon selection.
pub(crate) fn on_desktop_click() {
    focus_desktop();
    deselect_desktop();
}

/// Makes the shell's desktop window active. `GetShellWindow` is that window;
/// giving it the foreground deactivates the window in front, exactly as
/// clicking the desktop does. Spaces are never activated, so nothing else
/// would do this.
fn focus_desktop() {
    // SAFETY: both calls take plain window handles returned by the shell.
    unsafe {
        let desktop = GetShellWindow();
        if !desktop.is_invalid() && !SetForegroundWindow(desktop).as_bool() {
            tracing::debug!("could not focus the desktop on a space click");
        }
    }
}

/// Clears the desktop icon selection, as clicking empty desktop space does.
fn deselect_desktop() {
    let Some(view) = shell_view::desktop_shell_view() else {
        return;
    };
    // A null item with SVSI_DESELECTOTHERS deselects everything.
    // https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellview-selectitem
    // SAFETY: clearing the selection of a live view.
    if let Err(error) = unsafe { view.SelectItem(std::ptr::null(), SVSI_DESELECTOTHERS.0 as u32) } {
        tracing::debug!(%error, "could not clear the desktop selection");
    }
}
