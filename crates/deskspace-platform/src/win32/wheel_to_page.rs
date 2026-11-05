//! Drop-downs and sliders that hand the mouse wheel to their page.
//!
//! A combo box turns each wheel notch into an arrow key, and a trackbar moves
//! its thumb, so on a page that scrolls, a scroll that passed over one changed
//! its value: with the Debug window's typography box, a font changed on every
//! pass. Subclassed, the control passes the wheel to its parent, the page,
//! which scrolls. An open drop-down's list still scrolls itself, since its
//! own window gets the wheel; the keyboard still changes either value.
//! https://learn.microsoft.com/en-us/windows/win32/controls/subclassing-overview

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CB_GETDROPPEDSTATE, GetParent, SendMessageW, WM_MOUSEWHEEL, WM_NCDESTROY,
};

/// The subclass's id on a control (`SetWindowSubclass`).
const SUBCLASS_ID: usize = 1;

/// Which control is subclassed, kept in the subclass's reference data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum WheelControl {
    /// A combo box, which keeps the wheel while its list is open.
    DropDown = 1,
    /// A trackbar.
    Slider = 2,
}

/// Makes the live `control` pass the wheel to its parent.
pub(crate) fn pass_wheel_to_page(control: HWND, kind: WheelControl) {
    // SAFETY: `control` is a live control of ours; the subclass removes
    // itself in WM_NCDESTROY.
    unsafe {
        let _ = SetWindowSubclass(control, Some(subclass), SUBCLASS_ID, kind as usize);
    }
}

/// Whether `control`, subclassed as `kind`, is a drop-down whose list is open.
unsafe fn is_open(control: HWND, kind: usize) -> bool {
    // SAFETY: CB_GETDROPPEDSTATE goes only to a combo box.
    kind == WheelControl::DropDown as usize
        && unsafe { SendMessageW(control, CB_GETDROPPEDSTATE, None, None) }.0 != 0
}

unsafe extern "system" fn subclass(
    control: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    kind: usize,
) -> LRESULT {
    // SAFETY: comctl32 calls this for our live control with the message's own
    // arguments, which go on unchanged to the parent or the next procedure.
    unsafe {
        match message {
            WM_MOUSEWHEEL if !is_open(control, kind) => {
                if let Ok(page) = GetParent(control) {
                    SendMessageW(page, message, Some(wparam), Some(lparam));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(control, Some(subclass), id);
                DefSubclassProc(control, message, wparam, lparam)
            }
            _ => DefSubclassProc(control, message, wparam, lparam),
        }
    }
}
