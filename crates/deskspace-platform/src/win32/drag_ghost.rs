//! The floating drag image shown while an item is rearranged inside its
//! space. That drag is tracked by the app with mouse capture, not by OLE, so
//! the shell's drag-image manager is not involved; this window plays its part.
//!
//! The window is layered with per-pixel alpha (`UpdateLayeredWindow`) and
//! `WS_EX_TRANSPARENT`, so it is click-through and never takes the pointer
//! from the space that holds the capture. It is created on first use, reused,
//! and destroyed with the other UI windows.

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::win32::drag_image::{self, DragImage};
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceDragGhost");

pub(crate) struct DragGhost {
    hwnd: HWND,
    hotspot: Point,
}

impl DragGhost {
    pub(crate) fn create() -> Result<Self, PlatformError> {
        // SAFETY: plain window creation with a class whose procedure is
        // DefWindowProcW; the window carries no state.
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None)?.0);
            ensure_class()?;
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOPMOST,
                CLASS_NAME,
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )?;
            Ok(Self {
                hwnd,
                hotspot: Point::ZERO,
            })
        }
    }

    /// Shows `image` with its hotspot at `at` (screen coordinates).
    pub(crate) fn show(&mut self, image: &DragImage, at: Point) {
        self.hotspot = image.hotspot;
        let Some(bitmap) = drag_image::dib(&image.icon, true) else {
            return;
        };
        let origin = self.origin(at);
        let size = SIZE {
            cx: image.width(),
            cy: image.height(),
        };
        // Per-pixel alpha from premultiplied source pixels.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: the memory DC and bitmap are created, used, and released
        // here; UpdateLayeredWindow copies the pixels, so both can be freed.
        unsafe {
            let screen = GetDC(None);
            let memory = CreateCompatibleDC(Some(screen));
            let previous = SelectObject(memory, bitmap.into());
            let updated = UpdateLayeredWindow(
                self.hwnd,
                Some(screen),
                Some(&origin),
                Some(&size),
                Some(memory),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            SelectObject(memory, previous);
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            let _ = DeleteObject(bitmap.into());
            if let Err(error) = updated {
                tracing::debug!(%error, "drag ghost could not be drawn");
                return;
            }
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    /// Moves the shown image so its hotspot is at `at`.
    pub(crate) fn move_to(&self, at: Point) {
        let origin = self.origin(at);
        // SAFETY: repositioning our own live window; layered content is kept.
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                origin.x,
                origin.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub(crate) fn hide(&self) {
        // SAFETY: hiding our own live window.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn origin(&self, at: Point) -> POINT {
        POINT {
            x: at.x - self.hotspot.x,
            y: at.y - self.hotspot.y,
        }
    }
}

impl Drop for DragGhost {
    fn drop(&mut self) {
        // SAFETY: the window was created by `create` on this thread.
        unsafe {
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

fn ensure_class() -> Result<(), PlatformError> {
    window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)))
}
