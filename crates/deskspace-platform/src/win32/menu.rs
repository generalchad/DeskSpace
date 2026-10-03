use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MENU_ITEM_FLAGS, MF_CHECKED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_NULL,
};
use windows::core::PCWSTR;

use crate::ui::MenuEntry;
use crate::win32::system::wide;

/// Shows `entries` as a popup menu and returns the chosen id. Ids must be
/// non-zero: TrackPopupMenuEx returns 0 for "dismissed".
pub(crate) fn track(owner: HWND, entries: &[MenuEntry], at: Point) -> Option<u32> {
    // SAFETY: the menu is created, used, and destroyed within this call;
    // labels outlive the AppendMenuW calls that copy them.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for entry in entries {
            match entry {
                MenuEntry::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
                MenuEntry::Item {
                    id,
                    label,
                    checked,
                    enabled,
                } => {
                    let mut flags: MENU_ITEM_FLAGS = MF_STRING;
                    if *checked {
                        flags |= MF_CHECKED;
                    }
                    if !*enabled {
                        flags |= MF_GRAYED;
                    }
                    let text = wide(label);
                    let _ = AppendMenuW(menu, flags, *id as usize, PCWSTR(text.as_ptr()));
                }
            }
        }

        // A popup owned by a background window does not close when the user
        // clicks elsewhere unless the owner is foreground first, and the
        // trailing WM_NULL makes the next click work.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackpopupmenu#remarks
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0,
            at.x,
            at.y,
            owner,
            None,
        );
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        (chosen.0 != 0).then_some(chosen.0 as u32)
    }
}
