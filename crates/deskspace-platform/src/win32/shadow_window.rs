//! A space's shadow on screen (ROADMAP D15). DWM clips a window's visual
//! tree to the window's rect, and a shadow lies outside it, so a space that
//! casts one gets a window of its own. It is layered with per-pixel alpha,
//! click-through, and never activated, and it sits directly below its
//! space. It is not owned by the space, because Windows keeps an owned
//! window above its owner. `space_window::sync_shadow` keeps it in line
//! with its space whenever the space moves, resizes, shows, hides, changes
//! place in the z-order, is restyled, or plays a frame.
//!
//! The pixels (`render::shadow`) depend only on the look and the space's
//! size. A move or a fade only tells Windows where the window is and how
//! opaque.

use std::cell::Cell;

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDNEXT, GWL_EXSTYLE, GetWindow,
    GetWindowLongW, HWND_NOTOPMOST, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WINDOW_EX_STYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::render::shadow::{ShadowLook, ShadowPieces};
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceShadow");

thread_local! {
    /// The debug window's switch: every shadow's edge drawn, so its window
    /// can be seen. Session only (ROADMAP D14).
    static OUTLINED: Cell<bool> = const { Cell::new(false) };
}

/// Outlines every shadow, or stops; each shadow follows at its next sync.
pub(crate) fn set_outlined(on: bool) {
    OUTLINED.with(|cell| cell.set(on));
}

pub(crate) fn outlined() -> bool {
    OUTLINED.with(Cell::get)
}

/// How a shadow shows now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShadowPlacement {
    pub look: ShadowLook,
    /// The space window's rect, in screen coordinates.
    pub space: Rect,
    /// The whole shadow's opacity, 0 to 255.
    pub alpha: u8,
}

/// The DIB section a shadow is drawn into. It is kept at least as big as
/// the last shadow, so a space growing frame by frame rarely makes a new one.
struct Canvas {
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    /// `size.cx * size.cy` premultiplied BGRA pixels, owned by `bitmap`.
    bits: *mut u32,
    size: SIZE,
}

pub(crate) struct ShadowWindow {
    hwnd: HWND,
    memory: HDC,
    canvas: Option<Canvas>,
    pieces: Option<ShadowPieces>,
    /// What the canvas holds: the look, the space's size, and the outline.
    drawn: Option<(ShadowLook, (i32, i32), bool)>,
    /// What Windows shows, while it shows anything.
    shown: Option<ShadowPlacement>,
}

impl ShadowWindow {
    /// A hidden shadow window, or `None` when Windows refuses it.
    pub(crate) fn create() -> Option<Self> {
        // SAFETY: plain window and DC creation; both are released in `Drop`,
        // or right here if the second fails.
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).ok()?.0);
            window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc))).ok()?;
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("DeskSpace shadow"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance),
                None,
            )
            .ok()?;
            let memory = CreateCompatibleDC(None);
            if memory.is_invalid() {
                let _ = DestroyWindow(hwnd);
                return None;
            }
            Some(Self {
                hwnd,
                memory,
                canvas: None,
                pieces: None,
                drawn: None,
                shown: None,
            })
        }
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Shows the shadow as `placement` says, directly below `space`'s
    /// window, or hides it for `None`. Draws only when the look or the
    /// space's size changed.
    pub(crate) fn sync(&mut self, space: HWND, placement: Option<ShadowPlacement>) {
        let Some(placement) = placement.filter(|placement| {
            placement.alpha > 0 && placement.space.width > 0 && placement.space.height > 0
        }) else {
            self.hide();
            return;
        };
        let outline = outlined();
        let size = (placement.space.width, placement.space.height);
        let drawn = Some((placement.look, size, outline));
        if self.drawn != drawn {
            if !self.draw(placement.look, size, outline) {
                self.hide();
                return;
            }
            self.drawn = drawn;
            self.shown = None;
        }
        if self.shown != Some(placement) {
            if !self.update(placement) {
                self.hide();
                return;
            }
            self.shown = Some(placement);
        }
        self.place_below(space);
    }

    fn hide(&mut self) {
        if self.shown.take().is_some() {
            // SAFETY: our own window; its procedure is `DefWindowProcW`.
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    /// Draws the shadow of a space `size` with `look` into the canvas.
    fn draw(&mut self, look: ShadowLook, size: (i32, i32), outline: bool) -> bool {
        let rect = look.rect(Rect::new(0, 0, size.0, size.1));
        let Some((bits, stride, rows)) = self
            .canvas_for(SIZE {
                cx: rect.width,
                cy: rect.height,
            })
            .map(|canvas| {
                (
                    canvas.bits,
                    canvas.size.cx as usize,
                    canvas.size.cy as usize,
                )
            })
        else {
            return false;
        };
        // SAFETY: `bits` points at the DIB section's `stride * rows` pixels,
        // which the canvas's bitmap owns and keeps alive while `self` holds
        // it; nothing else touches them.
        let pixels = unsafe { std::slice::from_raw_parts_mut(bits, stride * rows) };
        if self
            .pieces
            .as_ref()
            .is_none_or(|pieces| pieces.look() != look)
        {
            self.pieces = Some(ShadowPieces::new(look));
        }
        if let Some(pieces) = &mut self.pieces {
            pieces.paint(size, pixels, stride, outline);
        }
        true
    }

    /// The canvas, made bigger first if it cannot hold `size`.
    fn canvas_for(&mut self, size: SIZE) -> Option<&Canvas> {
        let fits = self
            .canvas
            .as_ref()
            .is_some_and(|canvas| canvas.size.cx >= size.cx && canvas.size.cy >= size.cy);
        if !fits {
            // A quarter more, so a growing space does not need a new one
            // every frame.
            let grown = |wanted: i32, had: i32| (wanted + wanted / 4).max(had);
            let had = self
                .canvas
                .as_ref()
                .map_or(SIZE::default(), |canvas| canvas.size);
            let size = SIZE {
                cx: grown(size.cx, had.cx),
                cy: grown(size.cy, had.cy),
            };
            self.release_canvas();
            self.canvas = Some(new_canvas(self.memory, size)?);
        }
        self.canvas.as_ref()
    }

    fn release_canvas(&mut self) {
        if let Some(canvas) = self.canvas.take() {
            // SAFETY: the bitmap was selected into `memory` by `new_canvas`
            // and is released once.
            unsafe {
                SelectObject(self.memory, canvas.previous);
                let _ = DeleteObject(canvas.bitmap.into());
            }
        }
    }

    /// Tells Windows where the shadow is and how opaque, from the canvas.
    fn update(&self, placement: ShadowPlacement) -> bool {
        let rect = placement.look.rect(placement.space);
        // Per-pixel alpha from premultiplied source pixels, faded as a
        // whole by the constant alpha.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: placement.alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: our own layered window and memory DC, which holds a canvas
        // at least `rect`'s size; UpdateLayeredWindow copies the pixels.
        let updated = unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&POINT {
                    x: rect.x,
                    y: rect.y,
                }),
                Some(&SIZE {
                    cx: rect.width,
                    cy: rect.height,
                }),
                Some(self.memory),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        };
        if let Err(error) = &updated {
            tracing::debug!(%error, "shadow could not be drawn");
        }
        updated.is_ok()
    }

    /// Shows the shadow directly below `space`, topmost when it is, unless
    /// it is there already.
    fn place_below(&self, space: HWND) {
        // SAFETY: plain z-order and style queries, and SetWindowPos on our
        // own window, whose procedure is `DefWindowProcW`, so nothing it
        // sends reaches the app or the space.
        unsafe {
            let topmost = |window| {
                WINDOW_EX_STYLE(GetWindowLongW(window, GWL_EXSTYLE) as u32).contains(WS_EX_TOPMOST)
            };
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            if topmost(space) != topmost(self.hwnd) {
                let band = if topmost(space) {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                let _ = SetWindowPos(self.hwnd, Some(band), 0, 0, 0, 0, flags);
            }
            let below = GetWindow(space, GW_HWNDNEXT).ok() == Some(self.hwnd);
            if below && IsWindowVisible(self.hwnd).as_bool() {
                return;
            }
            let _ = SetWindowPos(self.hwnd, Some(space), 0, 0, 0, 0, flags | SWP_SHOWWINDOW);
        }
    }
}

/// A top-down DIB section `size` selected into `memory`.
fn new_canvas(memory: HDC, size: SIZE) -> Option<Canvas> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size.cx,
            // Negative: top-down rows, as `ShadowPieces::paint` writes them.
            biHeight: -size.cy,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    // SAFETY: a plain DIB section, released by `release_canvas`.
    unsafe {
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let previous = SelectObject(memory, bitmap.into());
        Some(Canvas {
            bitmap,
            previous,
            bits: bits.cast(),
            size,
        })
    }
}

impl Drop for ShadowWindow {
    fn drop(&mut self) {
        self.release_canvas();
        // SAFETY: made by `create` on this thread and released once.
        unsafe {
            let _ = DeleteDC(self.memory);
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
