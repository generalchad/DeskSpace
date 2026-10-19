//! Burn's flames on screen (ROADMAP D12, amended 2026-10-18). They rise past
//! the space's frame, and the space's own window carries the blur over its
//! whole rect, so they are drawn on a window of their own: layered with
//! per-pixel alpha, click-through, never activated, and owned by the space
//! window, which keeps it above the space and destroys it with it. It is
//! shown directly above the space (`insert_after`), and lives as long as one
//! Burn plays (`space_animation`).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_STYLE, GetWindowLongW, HWND_TOP,
    IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetWindowPos, ULW_ALPHA, UpdateLayeredWindow, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::render::burn::{self, BurnField, FlameBase};
use crate::render::flames::{self, MARGIN_SIDE, MARGIN_TOP, SPACING, Tongue};
use crate::win32::space_window;
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceFlames");

pub(crate) struct FlameOverlay {
    hwnd: HWND,
    /// The space window the flames burn on.
    owner: HWND,
    origin: POINT,
    size: SIZE,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    /// The DIB section's pixels, `size.cx * size.cy` premultiplied BGRA,
    /// owned by `bitmap`.
    bits: *mut u32,
    /// The same field the painter burns the space with, so the flames stand
    /// on its edge.
    field: BurnField,
    bases: Vec<FlameBase>,
    tongues: Vec<Tongue>,
    space_height: f32,
}

impl FlameOverlay {
    /// The flames for `space`'s window `owner` at `rect` (screen
    /// coordinates), burning with `detail`. `None` for a desktop-hosted
    /// space, a child window with nothing to float above, or when Windows
    /// refuses the window or its bitmap.
    pub(crate) fn create(owner: HWND, rect: Rect, detail: u8) -> Option<Self> {
        // SAFETY: a style query on the live space window.
        let style = unsafe { GetWindowLongW(owner, GWL_STYLE) } as u32;
        if style & WS_CHILD.0 != 0 || rect.is_empty() {
            return None;
        }
        let size = SIZE {
            cx: rect.width + 2 * MARGIN_SIDE as i32,
            cy: rect.height + MARGIN_TOP as i32,
        };
        let origin = POINT {
            x: rect.x - MARGIN_SIDE as i32,
            y: rect.y - MARGIN_TOP as i32,
        };
        let cols = (rect.width as f32 / burn::CELL as f32).ceil() as u32;
        let rows = (rect.height as f32 / burn::CELL as f32).ceil() as u32;
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
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.cx,
                    // Negative: top-down rows, as `flames::paint` writes them.
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
                field: BurnField::new(cols, rows, detail),
                bases: Vec::new(),
                tongues: Vec::new(),
                space_height: rect.height as f32,
            })
        }
    }

    /// Draws the flames for a burn at `front`, `time` seconds into any clock.
    pub(crate) fn draw(&mut self, front: f32, time: f32) {
        self.field.flame_bases(front, SPACING, &mut self.bases);
        flames::tongues(&self.bases, self.space_height, time, &mut self.tongues);
        let (width, height) = (self.size.cx as u32, self.size.cy as u32);
        // SAFETY: `bits` points at the DIB section's `width * height`
        // pixels, which `bitmap` owns and keeps alive while `self` lives;
        // nothing else touches them.
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(self.bits, (width * height) as usize) };
        flames::paint(pixels, width, height, &self.tongues, time);
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
                tracing::debug!(%error, "flames could not be drawn");
                return;
            }
            if !IsWindowVisible(self.hwnd).as_bool() {
                self.show();
            }
        }
    }

    /// Shows the flames directly above their space, under every app above it.
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

impl Drop for FlameOverlay {
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
    use super::*;

    fn window(id: usize) -> HWND {
        HWND(id as *mut core::ffi::c_void)
    }

    #[test]
    fn the_flames_go_directly_above_their_space() {
        let (overlay, app) = (window(1), window(2));
        // An app directly above the space keeps the flames under it.
        assert_eq!(insert_after(overlay, Some(app)), Some(app));
        // Already directly above the space: they stay put.
        assert_eq!(insert_after(overlay, Some(overlay)), None);
        // Nothing but topmost windows above the space.
        assert_eq!(insert_after(overlay, None), Some(HWND_TOP));
    }
}
