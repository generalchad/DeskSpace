//! A layered popup window that floats beside or above a space window for an
//! animation effect: per-pixel alpha, click-through, never activated, and
//! owned by the space window so it stays above it and closes with it. It is
//! placed directly above the space when made (`place_above_owner`) and again
//! whenever it shows (`insert_after`).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDPREV, GWL_EXSTYLE, GWL_STYLE, GetWindow,
    GetWindowLongW, HWND_TOP, HWND_TOPMOST, IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, ULW_ALPHA, UpdateLayeredWindow,
    WINDOW_EX_STYLE, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::win32::space_window;
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceOverlay");

/// An owned layered window and its top-down 32-bpp DIB section.
pub(crate) struct LayeredOverlay {
    hwnd: HWND,
    /// The space window the overlay floats above.
    owner: HWND,
    origin: POINT,
    size: SIZE,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    /// The DIB section's pixels, `size.cx * size.cy` premultiplied BGRA,
    /// owned by `bitmap`.
    bits: *mut u32,
}

impl LayeredOverlay {
    /// Creates an overlay owned by `owner` at `rect` (screen coordinates).
    /// Returns `None` for a desktop-hosted space, a child window with nothing
    /// to float above, an empty rect, or when Windows refuses the window or its
    /// bitmap.
    pub(crate) fn create(owner: HWND, rect: Rect) -> Option<Self> {
        // SAFETY: a style query on the live space window.
        let style = unsafe { GetWindowLongW(owner, GWL_STYLE) } as u32;
        if style & WS_CHILD.0 != 0 || rect.is_empty() {
            return None;
        }
        let origin = POINT {
            x: rect.x,
            y: rect.y,
        };
        let size = SIZE {
            cx: rect.width,
            cy: rect.height,
        };
        // SAFETY: plain window and GDI object creation; every object made
        // here is released in `Drop`, or right here if a later step fails.
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).ok()?.0);
            window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc))).ok()?;
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS_NAME,
                PCWSTR::null(),
                WS_POPUP,
                origin.x,
                origin.y,
                size.cx,
                size.cy,
                Some(owner),
                None,
                Some(instance),
                None,
            )
            .ok()?;
            place_above_owner(hwnd, owner);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.cx,
                    // Negative: top-down rows, matching pixel buffers.
                    biHeight: -size.cy,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let Ok(bitmap) = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)
            else {
                let _ = DestroyWindow(hwnd);
                return None;
            };
            let memory = CreateCompatibleDC(None);
            let previous = SelectObject(memory, bitmap.into());
            Some(Self {
                hwnd,
                owner,
                origin,
                size,
                memory,
                bitmap,
                previous,
                bits: bits.cast(),
            })
        }
    }

    /// Follows the space to `rect`; the overlay moves at its next `present`.
    /// Returns false for a rect of another size, which needs a new overlay.
    pub(crate) fn follow(&mut self, rect: Rect) -> bool {
        let size = SIZE {
            cx: rect.width,
            cy: rect.height,
        };
        if size != self.size {
            return false;
        }
        self.origin = POINT {
            x: rect.x,
            y: rect.y,
        };
        true
    }

    pub(crate) fn pixel_size(&self) -> (u32, u32) {
        (self.size.cx as u32, self.size.cy as u32)
    }

    pub(crate) fn pixels(&mut self) -> &mut [u32] {
        let (width, height) = self.pixel_size();
        // SAFETY: `bits` points at the DIB section's `width * height`
        // pixels, which `bitmap` owns and keeps alive while `self` lives;
        // nothing else touches them.
        unsafe { std::slice::from_raw_parts_mut(self.bits, (width * height) as usize) }
    }

    /// Uploads the pixel buffer to the layered window and shows it directly
    /// above its owner if hidden.
    pub(crate) fn present(&self) {
        // Per-pixel alpha from premultiplied source pixels.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: our own layered window and memory DC; UpdateLayeredWindow
        // copies the pixels.
        unsafe {
            if let Err(error) = UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&self.origin),
                Some(&self.size),
                Some(self.memory),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            ) {
                tracing::debug!(%error, "overlay could not be drawn");
                return;
            }
            if !IsWindowVisible(self.hwnd).as_bool() {
                self.show();
            }
        }
    }

    /// Shows the overlay directly above its space, under every app above it.
    ///
    /// Owning keeps the overlay above the space, but no lower: shown as
    /// made, it sat at the top of the z-order, so the flames burned over
    /// every app in front of the space (seen 2026-10-19).
    fn show(&self) {
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
        let (after, flags) = match insert_after(self.hwnd, space_window::window_above(self.owner)) {
            Some(after) => (Some(after), flags),
            None => (None, flags | SWP_NOZORDER),
        };
        // SAFETY: our own window, on the thread that made it; its procedure
        // is `DefWindowProcW`, so the messages SetWindowPos sends do not
        // reach the app.
        unsafe {
            let _ = SetWindowPos(self.hwnd, after, 0, 0, 0, 0, flags);
        }
    }
}

/// The `hwndInsertAfter` that puts `overlay` directly above its space, or
/// `None` when it is there already. `above_space` is the non-topmost window
/// directly above the space, if any (`space_window::window_above`).
fn insert_after(overlay: HWND, above_space: Option<HWND>) -> Option<HWND> {
    match above_space {
        Some(above) if above == overlay => None,
        Some(above) => Some(above),
        // Only topmost windows are above the space, so the top of the
        // non-topmost band is the slot directly above it.
        None => Some(HWND_TOP),
    }
}

impl Drop for LayeredOverlay {
    fn drop(&mut self) {
        // SAFETY: the objects were made by `create` on this thread and are
        // released once. The window may already be gone with its owner, in
        // which case DestroyWindow fails harmlessly.
        unsafe {
            SelectObject(self.memory, self.previous);
            let _ = DeleteDC(self.memory);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Places `window` directly above `owner`, in the owner's z-order band.
///
/// Ownership keeps an overlay above its owner but does not put it there:
/// Windows creates a new owned popup at the top of the non-topmost band, so
/// Burn's flames drew over every application (seen 2026-10-22). A topmost
/// owner (`--on-top`) needs a topmost overlay, or it would land below the
/// space it belongs to.
fn place_above_owner(window: HWND, owner: HWND) {
    // SAFETY: both are live windows, `owner` from `create` and `window` just
    // made on this thread. None of these calls sends a message.
    unsafe {
        let owner_topmost = is_topmost(owner);
        // `SetWindowPos` inserts `window` below the window named here
        // (`hWndInsertAfter` precedes it in the z-order), so the slot above
        // the owner is the window above it now.
        let above = GetWindow(owner, GW_HWNDPREV)
            .ok()
            .filter(|above| is_topmost(*above) == owner_topmost);
        let insert_after = match (above, owner_topmost) {
            (Some(above), _) => above,
            (None, true) => HWND_TOPMOST,
            (None, false) => HWND_TOP,
        };
        if let Err(error) = SetWindowPos(
            window,
            Some(insert_after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) {
            tracing::debug!(%error, "the overlay could not be placed above its space");
        }
    }
}

/// Whether `window` sits in the topmost band.
///
/// # Safety
/// `window` is a live window.
fn is_topmost(window: HWND) -> bool {
    // SAFETY: a plain style query on a live window.
    let style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
    WINDOW_EX_STYLE(style).contains(WS_EX_TOPMOST)
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwarding Windows' own arguments unchanged.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowPos, WINDOW_EX_STYLE, WS_POPUP,
    };
    use windows::core::w;

    use super::*;

    fn window(id: usize) -> HWND {
        HWND(id as *mut core::ffi::c_void)
    }

    #[test]
    fn the_flames_go_directly_above_their_space() {
        let (overlay, app) = (window(1), window(2));
        // An app directly above the space keeps the overlay under it.
        assert_eq!(insert_after(overlay, Some(app)), Some(app));
        // Already directly above the space: they stay put.
        assert_eq!(insert_after(overlay, Some(overlay)), None);
        // Nothing but topmost windows above the space.
        assert_eq!(insert_after(overlay, None), Some(HWND_TOP));
    }

    #[test]
    fn the_overlay_starts_directly_above_its_owner() {
        // SAFETY: windows made on this thread and destroyed before it ends.
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("Static"),
                w!("overlay test owner"),
                WS_POPUP,
                -30000,
                -30000,
                100,
                100,
                None,
                None,
                None,
                None,
            )
            .expect("an owner window");
            // A space is pinned at the bottom, where Windows' placement for a
            // new owned popup is most obviously wrong.
            let _ = SetWindowPos(
                owner,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let overlay = LayeredOverlay::create(owner, Rect::new(-30000, -30000, 80, 60))
                .expect("an overlay window");
            assert_eq!(
                GetWindow(owner, GW_HWNDPREV).ok(),
                Some(overlay.hwnd),
                "the overlay sits directly above the space it belongs to"
            );
            drop(overlay);
            let _ = DestroyWindow(owner);
        }
    }
}
