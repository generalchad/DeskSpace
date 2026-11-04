//! Keeping space windows at the bottom of the z-order, above the desktop layer.

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GW_HWNDPREV, GWL_EXSTYLE, GetWindow, GetWindowLongW, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    WINDOW_EX_STYLE, WINDOWPOS, WS_EX_TOPMOST,
};

use crate::win32::runtime;
use crate::win32::shadow_window::ShadowWindow;

use super::with_state;

/// Moves a top-level space between its two placements: above every window
/// (`--on-top`), or at the bottom, where `on_window_pos_changing` pins it as
/// at creation. A desktop-hosted space has only one placement.
pub(crate) fn set_on_top(hwnd: HWND, on_top: bool) {
    // SAFETY: `hwnd` is a live space window; the state borrow is short and
    // ends before SetWindowPos, whose WM_WINDOWPOSCHANGING reads the state.
    unsafe {
        let hosted = with_state(hwnd, |state| {
            state.on_top = on_top;
            state.hosted
        });
        if hosted != Some(false) {
            return;
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        if on_top {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
        } else {
            let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags);
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, flags);
        }
    }
}

/// Sends `hwnd` to the bottom of its siblings, where the desktop layer
/// belongs: a top-level window lands where `on_window_pos_changing` pins it,
/// and a desktop-hosted one below the hosted spaces. A window kept above the
/// others (`--on-top`) stays there, since the bottom would drop its topmost
/// style.
pub(crate) fn sink(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the state borrow is short and
    // ends before SetWindowPos.
    unsafe {
        if with_state(hwnd, |state| state.on_top) != Some(false) {
            return;
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, flags);
    }
}

/// Keeps top-level spaces under every other window so they behave like part
/// of the desktop, and above the desktop layer (`insert_after`).
pub(super) unsafe fn on_window_pos_changing(window: HWND, lparam: LPARAM) {
    // SAFETY: for WM_WINDOWPOSCHANGING lParam is null or a live WINDOWPOS the
    // system lets us edit; the state borrow is short.
    unsafe {
        let position = lparam.0 as *mut WINDOWPOS;
        let pinned = with_state(window, |state| {
            let shadow = state.shadow.as_ref().map(ShadowWindow::hwnd);
            (!state.hosted && !state.on_top).then_some(shadow)
        })
        .flatten();
        let Some(shadow) = pinned else {
            return;
        };
        if position.is_null() || (*position).flags.contains(SWP_NOZORDER) {
            return;
        }
        let layer = runtime::desktop_layer_window();
        // The space's own shadow sits right under it, so the space is
        // directly above the layer when only its shadow is between them.
        let above_layer = layer.and_then(window_above).and_then(|above| {
            if Some(above) == shadow {
                window_above(above)
            } else {
                Some(above)
            }
        });
        match insert_after(window, layer, above_layer) {
            Some(after) => (*position).hwndInsertAfter = after,
            None => (*position).flags |= SWP_NOZORDER,
        }
    }
}

/// Where a pinned top-level window goes when its z-order changes, as the
/// `hwndInsertAfter` to use, or `None` to stay where it is. `above_layer` is
/// the non-topmost window directly above the desktop `layer`, if any.
///
/// The layer and the spaces all live at the bottom, and the layer spans the
/// whole desktop. When every one of them went to `HWND_BOTTOM`, the last to
/// move ended up lowest, so an outside z-order change (Show desktop, a task
/// switch) could leave a space under the layer: still visible through it,
/// but its clicks and drops went to the layer, which refuses drops.
/// Spaces therefore go directly above the layer, which alone takes the
/// bottom.
fn insert_after(window: HWND, layer: Option<HWND>, above_layer: Option<HWND>) -> Option<HWND> {
    match layer {
        None => Some(HWND_BOTTOM),
        Some(layer) if layer == window => Some(HWND_BOTTOM),
        Some(_) => match above_layer {
            Some(above) if above == window => None,
            Some(above) => Some(above),
            // Only topmost windows are above the layer, so the top of the
            // non-topmost band is the slot directly above it.
            None => Some(HWND_TOP),
        },
    }
}

/// The window directly above `window` in the z-order, unless it is topmost.
/// Then `HWND_TOP` names the same slot for a non-topmost window, without
/// positioning it relative to a topmost one.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos#remarks
pub(crate) fn window_above(window: HWND) -> Option<HWND> {
    // SAFETY: plain z-order and style queries; neither sends a message.
    unsafe {
        let above = GetWindow(window, GW_HWNDPREV).ok()?;
        let style = WINDOW_EX_STYLE(GetWindowLongW(above, GWL_EXSTYLE) as u32);
        (!style.contains(WS_EX_TOPMOST)).then_some(above)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: usize) -> HWND {
        HWND(id as *mut core::ffi::c_void)
    }

    #[test]
    fn without_a_layer_a_space_goes_to_the_bottom() {
        assert_eq!(insert_after(window(1), None, None), Some(HWND_BOTTOM));
    }

    #[test]
    fn the_layer_itself_goes_to_the_bottom() {
        let layer = window(9);
        assert_eq!(
            insert_after(layer, Some(layer), Some(window(1))),
            Some(HWND_BOTTOM)
        );
    }

    #[test]
    fn a_space_goes_directly_above_the_layer() {
        let (space, other, layer) = (window(1), window(2), window(9));
        assert_eq!(insert_after(space, Some(layer), Some(other)), Some(other));
        // Already directly above the layer: it stays put.
        assert_eq!(insert_after(space, Some(layer), Some(space)), None);
        // Nothing but topmost windows above the layer.
        assert_eq!(insert_after(space, Some(layer), None), Some(HWND_TOP));
    }
}
