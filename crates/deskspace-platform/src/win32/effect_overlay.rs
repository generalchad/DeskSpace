//! What some effects draw beside the space (ROADMAP D12): Burn's flames
//! rise past the space's frame, and the space's own window carries the blur
//! over its whole rect, so they are drawn on a window of their own: layered
//! with per-pixel alpha, click-through, never activated, and owned by the
//! space window, which keeps it directly above the space and destroys it
//! with it. One lives as long as one play of such an effect
//! (`space_animation`). The pixels are shaded on the CPU by pure code in
//! `render/`; this module owns only the window and the effect's state.

use deskspace_core::animation::Effect;
use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_STYLE, GetWindowLongW, IsWindowVisible,
    SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WS_CHILD, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::render::burn::{self, BurnField, FlameBase};
use crate::render::flames::{self, Tongue};
use crate::render::{Mask, Play, TransitionFrame};
use crate::win32::window_class::{self, WindowClass};

const CLASS_NAME: PCWSTR = w!("DeskSpaceEffectOverlay");

/// The room an overlay keeps around the space, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Margins {
    side: u32,
    top: u32,
}

pub(crate) struct EffectOverlay {
    window: LayeredWindow,
    art: Art,
}

/// The effect an overlay draws, with what it keeps between frames.
enum Art {
    Flames(Flames),
}

struct Flames {
    /// The same field the painter burns the space with, so the flames stand
    /// on its edge.
    field: BurnField,
    bases: Vec<FlameBase>,
    tongues: Vec<Tongue>,
    space_height: f32,
}

impl EffectOverlay {
    /// The overlay `play` draws beside the space window `owner` at `rect`
    /// (screen coordinates). `None` for an effect with nothing beside the
    /// space, a desktop-hosted space (a child window, with nothing to float
    /// above), or when Windows refuses the window or its bitmap.
    pub(crate) fn create(owner: HWND, rect: Rect, play: &Play) -> Option<Self> {
        let (margins, art) = match play.effect {
            Effect::Burn => {
                let cols = (rect.width as f32 / burn::CELL as f32).ceil() as u32;
                let rows = (rect.height as f32 / burn::CELL as f32).ceil() as u32;
                let flames = Flames {
                    field: BurnField::new(cols, rows, play.detail),
                    bases: Vec::new(),
                    tongues: Vec::new(),
                    space_height: rect.height as f32,
                };
                let margins = Margins {
                    side: flames::MARGIN_SIDE,
                    top: flames::MARGIN_TOP,
                };
                (margins, Art::Flames(flames))
            }
            _ => return None,
        };
        let window = LayeredWindow::create(owner, rect, margins)?;
        Some(Self { window, art })
    }

    /// Draws `frame`'s part beside the space, `time` seconds into any clock.
    pub(crate) fn draw(&mut self, frame: &TransitionFrame, time: f32) {
        let (width, height) = self.window.pixel_size();
        let pixels = self.window.pixels();
        match (&mut self.art, frame.mask) {
            (Art::Flames(state), Mask::Burn { front, .. }) => {
                state
                    .field
                    .flame_bases(front, flames::SPACING, &mut state.bases);
                flames::tongues(&state.bases, state.space_height, time, &mut state.tongues);
                flames::paint(pixels, width, height, &state.tongues, time);
            }
            _ => return,
        }
        self.window.present();
    }
}

/// A layered, click-through window over the space and its margins, and the
/// DIB section it is drawn from.
struct LayeredWindow {
    hwnd: HWND,
    origin: POINT,
    size: SIZE,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    /// The DIB section's pixels, `size.cx * size.cy` premultiplied BGRA,
    /// owned by `bitmap`.
    bits: *mut u32,
}

impl LayeredWindow {
    fn create(owner: HWND, rect: Rect, margins: Margins) -> Option<Self> {
        // SAFETY: a style query on the live space window.
        let style = unsafe { GetWindowLongW(owner, GWL_STYLE) } as u32;
        if style & WS_CHILD.0 != 0 || rect.is_empty() {
            return None;
        }
        let size = SIZE {
            cx: rect.width + 2 * margins.side as i32,
            cy: rect.height + margins.top as i32,
        };
        let origin = POINT {
            x: rect.x - margins.side as i32,
            y: rect.y - margins.top as i32,
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
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.cx,
                    // Negative: top-down rows, as the shaders in `render/`
                    // write them.
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
            })
        }
    }

    fn pixel_size(&self) -> (u32, u32) {
        (self.size.cx as u32, self.size.cy as u32)
    }

    fn pixels(&mut self) -> &mut [u32] {
        let (width, height) = self.pixel_size();
        // SAFETY: `bits` points at the DIB section's `width * height`
        // pixels, which `bitmap` owns and keeps alive while `self` lives;
        // the `&mut self` borrow keeps anything else from touching them.
        unsafe { std::slice::from_raw_parts_mut(self.bits, (width * height) as usize) }
    }

    /// Puts the pixels on screen, showing the window the first time.
    fn present(&self) {
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
                tracing::debug!(%error, "an effect overlay could not be drawn");
                return;
            }
            if !IsWindowVisible(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
    }
}

impl Drop for LayeredWindow {
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
