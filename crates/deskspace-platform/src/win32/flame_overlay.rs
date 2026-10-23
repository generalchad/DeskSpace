//! Burn's flames on screen (ROADMAP D12, amended 2026-10-18). They rise past
//! the space's frame, and the space's own window carries the blur over its
//! whole rect, so they are drawn on a window of their own: layered with
//! per-pixel alpha, click-through, never activated, and owned by the space
//! window, so it dies with it, and placed directly above it when made
//! (`place_above_owner`). It lives as long as one Burn plays
//! (`space_animation`).

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
    GetWindowLongW, HWND_TOP, HWND_TOPMOST, IsWindowVisible, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WINDOW_EX_STYLE, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::render::burn::{self, BurnField, FlameBase};
use crate::render::flames::{self, MARGIN_SIDE, MARGIN_TOP, SPACING, Tongue};
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceFlames");

pub(crate) struct FlameOverlay {
    hwnd: HWND,
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
            place_above_owner(hwnd, owner);
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
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
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
            tracing::debug!(%error, "the flames could not be placed above their space");
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

    #[test]
    fn the_overlay_starts_directly_above_its_owner() {
        // SAFETY: windows made on this thread and destroyed before it ends.
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("Static"),
                w!("flame test owner"),
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
            let flames = FlameOverlay::create(owner, Rect::new(-30000, -30000, 80, 60), 3)
                .expect("an overlay window");
            assert_eq!(
                GetWindow(owner, GW_HWNDPREV).ok(),
                Some(flames.hwnd),
                "the flames sit directly above the space they burn"
            );
            drop(flames);
            let _ = DestroyWindow(owner);
        }
    }
}
