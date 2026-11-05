//! A space's mouse messages, turned into `SpaceEvent`s.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::System::SystemServices::{MK_CONTROL, MK_SHIFT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetParent, IDC_ARROW, IDC_HAND, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
    LoadCursorW, SetCursor, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
    WM_RBUTTONUP,
};

use crate::input::{MouseButton, SpaceEvent};
use crate::ui::CursorShape;
use crate::win32::system::{hiword, loword};

use super::{is_hosted, space_event, with_state};

/// Sets the pointer shown over the space. The app calls this while handling
/// a mouse event on the space, so the cursor is over it (or captured by it)
/// and is updated at once. WM_SETCURSOR is not sent while the mouse is
/// captured, so waiting for it would leave the old shape during a drag.
pub(crate) fn set_cursor(hwnd: HWND, shape: CursorShape) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let changed = unsafe {
        with_state(hwnd, |state| {
            std::mem::replace(&mut state.cursor, shape) != shape
        })
    };
    if changed == Some(true) {
        apply_cursor(shape);
    }
}

pub(super) fn apply_cursor(shape: CursorShape) {
    let id = match shape {
        CursorShape::Arrow => IDC_ARROW,
        CursorShape::SizeWestEast => IDC_SIZEWE,
        CursorShape::SizeNorthSouth => IDC_SIZENS,
        CursorShape::SizeNorthWestSouthEast => IDC_SIZENWSE,
        CursorShape::SizeNorthEastSouthWest => IDC_SIZENESW,
        CursorShape::Hand => IDC_HAND,
    };
    // SAFETY: stock cursors are shared system resources and are never freed.
    unsafe {
        if let Ok(cursor) = LoadCursorW(None, id) {
            SetCursor(Some(cursor));
        }
    }
}

/// Mouse-button bits of the `wParam` key state on mouse messages. Defined here
/// to avoid pulling in `Win32_System_SystemServices` for three constants.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;
const MK_ANY_BUTTON: usize = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

/// Converts a client point to the space's desktop coordinates: screen
/// coordinates for top-level spaces, the host's client coordinates for hosted
/// ones.
pub(super) unsafe fn to_desktop(window: HWND, lparam: LPARAM, hosted: bool) -> (i32, i32) {
    let mut point = [POINT {
        x: loword(lparam.0),
        y: hiword(lparam.0),
    }];
    // SAFETY: `window` is live; `point` is a valid one-element buffer.
    unsafe {
        let target = if hosted { GetParent(window).ok() } else { None };
        MapWindowPoints(Some(window), target, &mut point);
    }
    (point[0].x, point[0].y)
}

/// Ctrl and Shift as a mouse message reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Modifiers {
    ctrl: bool,
    shift: bool,
}

/// Reads the `MK_*` flags that every mouse message carries in the low word of
/// wParam (for the wheel messages, GET_KEYSTATE_WPARAM).
///
/// `GetKeyState` cannot be used here: it reports the key state as of the last
/// keyboard message this thread read, and a space is never activated, so its
/// thread never sees Ctrl or Shift go down. That is why Ctrl+wheel did nothing
/// over a space.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
fn modifiers(wparam: WPARAM) -> Modifiers {
    let keys = (wparam.0 & 0xFFFF) as u32;
    Modifiers {
        ctrl: keys & MK_CONTROL.0 != 0,
        shift: keys & MK_SHIFT.0 != 0,
    }
}

fn button_for(message: u32) -> MouseButton {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

pub(super) unsafe fn on_mouse_move(window: HWND, lparam: LPARAM) {
    // SAFETY: `window` is a live space window; the state borrows are short.
    unsafe {
        let start_tracking = with_state(window, |state| {
            !std::mem::replace(&mut state.tracking_leave, true)
        });
        if start_tracking == Some(true) {
            // Ask for WM_MOUSELEAVE so hover highlights clear when the cursor
            // leaves the space.
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackmouseevent
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
        }
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(window, SpaceEvent::MouseMove { x, y });
    }
}

pub(super) unsafe fn on_mouse_leave(window: HWND) {
    // SAFETY: `window` is a live space window; the state borrow is short.
    unsafe {
        with_state(window, |state| {
            state.tracking_leave = false;
            state.cursor = CursorShape::Arrow;
        });
        space_event(window, SpaceEvent::MouseLeave);
    }
}

pub(super) unsafe fn on_mouse_wheel(window: HWND, wparam: WPARAM, lparam: LPARAM, tilt: bool) {
    // Unlike other mouse messages, the wheel position is in screen
    // coordinates, and the delta is the signed high word of wParam
    // (GET_WHEEL_DELTA_WPARAM). A tilt wheel's positive delta is to the right.
    // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
    // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousehwheel
    let delta = hiword(wparam.0 as isize);
    let mut point = [POINT {
        x: loword(lparam.0),
        y: hiword(lparam.0),
    }];
    let Modifiers { ctrl, shift } = modifiers(wparam);
    // SAFETY: `window` is a live space window and `point` a valid one-element
    // buffer.
    unsafe {
        if is_hosted(window) {
            MapWindowPoints(None, GetParent(window).ok(), &mut point);
        }
        space_event(
            window,
            SpaceEvent::Wheel {
                x: point[0].x,
                y: point[0].y,
                delta,
                ctrl,
                shift,
                tilt,
            },
        );
    }
}

pub(super) unsafe fn on_button_down(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    let Modifiers { ctrl, shift } = modifiers(wparam);
    // SAFETY: `window` is a live space window.
    unsafe {
        // Capture so the matching button-up arrives even if the cursor leaves
        // the window mid-drag.
        // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input#mouse-capture
        SetCapture(window);
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(
            window,
            SpaceEvent::MouseDown {
                x,
                y,
                button: button_for(message),
                ctrl,
                shift,
            },
        );
    }
}

pub(super) unsafe fn on_button_up(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    // SAFETY: `window` is a live space window. ReleaseCapture is not tied to
    // the window, so it is sound even if the handler destroyed it.
    unsafe {
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(
            window,
            SpaceEvent::MouseUp {
                x,
                y,
                button: button_for(message),
            },
        );
        if wparam.0 & MK_ANY_BUTTON == 0 {
            let _ = ReleaseCapture();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_come_from_the_mouse_message() {
        let wheel_with_ctrl = WPARAM((120usize << 16) | MK_CONTROL.0 as usize);
        assert_eq!(
            modifiers(wheel_with_ctrl),
            Modifiers {
                ctrl: true,
                shift: false
            }
        );
        let click_with_both = WPARAM((MK_CONTROL.0 | MK_SHIFT.0) as usize);
        assert_eq!(
            modifiers(click_with_both),
            Modifiers {
                ctrl: true,
                shift: true
            }
        );
        let wheel_down_alone = WPARAM((-120i16 as u16 as usize) << 16);
        assert_eq!(
            modifiers(wheel_down_alone),
            Modifiers {
                ctrl: false,
                shift: false
            }
        );
    }
}
