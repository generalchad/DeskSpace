//! Every private window message DeskSpace posts to its own windows (the
//! controller, unless one says otherwise), in one place so no two share a
//! value. The values are fixed: `SHOW_SPACES` crosses processes (a second
//! launch posts it), and none is ever renumbered.
//! Payload encodings stay beside the code that posts and decodes them.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-app

use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// The tray icon's callback (`NOTIFYICONDATAW::uCallbackMessage`).
pub(crate) const TRAY_CALLBACK: u32 = WM_APP + 1;

/// A press outside every space: wParam is x, lParam y (screen coordinates).
pub(crate) const OUTSIDE_PROBE: u32 = WM_APP + 2;

/// An editing key during inline rename: wParam is a kind code and lParam the
/// character for `KIND_CHAR` (`keyboard::decode`).
pub(crate) const EDIT_KEY: u32 = WM_APP + 3;

/// Delete while an item is selected.
pub(crate) const DELETE_KEY: u32 = WM_APP + 4;

/// A clipboard shortcut; wParam is one of the `keyboard::ACTION_*` codes.
pub(crate) const CLIPBOARD_KEY: u32 = WM_APP + 5;

/// F2 while an item is selected or a space is current.
pub(crate) const RENAME_KEY: u32 = WM_APP + 6;

/// Ctrl+A while a space is current.
pub(crate) const SELECT_ALL_KEY: u32 = WM_APP + 7;

/// Posted by a second launch to ask this instance to show its spaces.
pub(crate) const SHOW_SPACES: u32 = WM_APP + 8;

/// Alt+Enter while an item is selected or a space is current.
pub(crate) const PROPERTIES_KEY: u32 = WM_APP + 9;

/// Posted by a worker thread through `Ui::waker`; wParam is the caller's id.
pub(crate) const WAKE: u32 = WM_APP + 10;

/// A watched selection key; wParam is one of the `keyboard::SEL_*` codes.
pub(crate) const SELECTION_KEY: u32 = WM_APP + 11;

/// A typed character for type-to-select; wParam is the character.
pub(crate) const TYPE_KEY: u32 = WM_APP + 12;

/// An arrow key; wParam packs the direction and the Shift (extend) and Ctrl
/// (focus) modifiers (`keyboard::nav_key`).
pub(crate) const NAV_KEY: u32 = WM_APP + 13;

/// A watched folder's contents changed (`Ui::watch_folder`).
pub(crate) const FOLDER_CHANGE: u32 = WM_APP + 14;

/// To an animating space window, after each composition (`frame_clock`).
pub(crate) const FRAME: u32 = WM_APP + 15;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every message above. A new one must be added here too, so the checks
    /// below cover it.
    const ALL: [u32; 15] = [
        TRAY_CALLBACK,
        OUTSIDE_PROBE,
        EDIT_KEY,
        DELETE_KEY,
        CLIPBOARD_KEY,
        RENAME_KEY,
        SELECT_ALL_KEY,
        SHOW_SPACES,
        PROPERTIES_KEY,
        WAKE,
        SELECTION_KEY,
        TYPE_KEY,
        NAV_KEY,
        FOLDER_CHANGE,
        FRAME,
    ];

    #[test]
    fn message_ids_are_unique() {
        let mut ids = ALL.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len(), "two messages share a value");
    }

    #[test]
    fn message_ids_are_in_the_private_range() {
        // WM_APP through 0xBFFF is reserved for an application's own messages.
        for id in ALL {
            assert!((WM_APP..=0xBFFF).contains(&id), "{id:#x} is outside WM_APP");
        }
    }
}
