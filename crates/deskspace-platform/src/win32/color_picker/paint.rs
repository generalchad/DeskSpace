//! Paints the picker's panels and anti-aliased markers into its back buffer.

use super::draw::{draw_button, draw_hex, draw_label, draw_swatch, fill_image, fill_solid, frame};
use super::layout::{Panel, PickerLayout, Region};
use super::*;

/// Blends `over` onto `base` by `alpha` (`0.0..=1.0`), to draw a translucent
/// color over the checkerboard.
fn blend(base: Color, over: Color, alpha: f32) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |b: u8, o: u8| ((b as f32 * (1.0 - a)) + (o as f32 * a)).round() as u8;
    Color::rgb(
        mix(base.r, over.r),
        mix(base.g, over.g),
        mix(base.b, over.b),
    )
}

/// The checkerboard cell color under a point.
fn checker_color(x: i32, y: i32, colors: UiColors) -> Color {
    let (light, dark) = if colors.dark {
        (Color::rgb(0x60, 0x60, 0x60), Color::rgb(0x38, 0x38, 0x38))
    } else {
        (Color::rgb(0xFF, 0xFF, 0xFF), Color::rgb(0xC8, 0xC8, 0xC8))
    };
    if ((x / CHECKER) + (y / CHECKER)) % 2 == 0 {
        light
    } else {
        dark
    }
}

/// A device-independent back buffer. It makes painting flicker-free, and its
/// pixels are written directly for the marker circles, which need their own
/// anti-aliasing (GDI draws ellipses and lines without any).
pub(super) struct Buffer {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    size: (i32, i32),
}

impl Buffer {
    /// Creates a top-down 32bpp back buffer of `size`, compatible with `reference`.
    unsafe fn new(reference: HDC, size: (i32, i32)) -> Option<Self> {
        let (width, height) = (size.0.max(1), size.1.max(1));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: `info` is fully initialised; `bits` receives the pixel
        // pointer of the created section.
        unsafe {
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dc = CreateCompatibleDC(Some(reference));
            let bitmap =
                CreateDIBSection(Some(reference), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                    .ok()?;
            if bits.is_null() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            Some(Self {
                dc,
                bitmap,
                previous,
                bits: bits as *mut u32,
                size: (width, height),
            })
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // SAFETY: the bitmap is ours and currently selected into this DC.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

pub(super) unsafe fn paint(window: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: BeginPaint/EndPaint bracket the paint; GDI drawing does not send
    // messages, so the state borrow inside is safe.
    unsafe {
        let hdc = BeginPaint(window, &mut ps);
        let _ = with_state(window, |state| {
            ensure_buffer(state, hdc);
            match state.buffer.as_ref() {
                Some(buffer) => {
                    paint_content(buffer.dc, state);
                    paint_markers(buffer, state);
                    let _ = BitBlt(
                        hdc,
                        0,
                        0,
                        buffer.size.0,
                        buffer.size.1,
                        Some(buffer.dc),
                        0,
                        0,
                        SRCCOPY,
                    );
                }
                // Without a back buffer, paint straight to the window; the
                // marker circles are skipped.
                None => paint_content(hdc, state),
            }
        });
        let _ = EndPaint(window, &ps);
    }
}

/// Creates `state.buffer` at the layout size when it is missing or stale.
unsafe fn ensure_buffer(state: &mut PickerState, reference: HDC) {
    if state
        .buffer
        .as_ref()
        .is_some_and(|buffer| buffer.size == state.layout.size)
    {
        return;
    }
    // SAFETY: creating a DIB section and a memory DC on this thread.
    state.buffer = unsafe { Buffer::new(reference, state.layout.size) };
}

/// Composites the anti-aliased marker circles into the back buffer's pixels,
/// over the already-drawn gradients.
unsafe fn paint_markers(buffer: &Buffer, state: &PickerState) {
    if state.panel != Panel::Custom {
        return;
    }
    // SAFETY: GdiFlush completes the batched drawing into the buffer so the
    // pixels written below start from the finished image.
    unsafe {
        let _ = GdiFlush();
    }
    let center = |rect: Rect, fx: f32, fy: f32| {
        (
            rect.x + (fx * (rect.width - 1) as f32).round() as i32,
            rect.y + (fy * (rect.height - 1) as f32).round() as i32,
        )
    };
    let square = state.layout.saturation;
    let (sx, sy) = center(square, state.saturation, 1.0 - state.value);
    let hue = state.layout.hue;
    let (hx, hy) = center(hue, state.hue / 360.0, 0.5);
    let alpha = state.layout.alpha;
    let (ax, ay) = center(alpha, state.alpha as f32 / 255.0, 0.5);
    // SAFETY: the buffer's pixels are owned by this thread and every index is
    // bounds-checked inside `composite_marker`.
    unsafe {
        composite_marker(
            buffer.bits,
            buffer.size,
            sx,
            sy,
            scaled_marker(state.layout, MARKER),
        );
        composite_marker(
            buffer.bits,
            buffer.size,
            hx,
            hy,
            scaled_marker(state.layout, STRIP_MARKER),
        );
        composite_marker(
            buffer.bits,
            buffer.size,
            ax,
            ay,
            scaled_marker(state.layout, STRIP_MARKER),
        );
    }
}

/// Composites a black-and-white bullseye marker centered at (`cx`, `cy`) over
/// the buffer's pixels, anti-aliased by per-pixel coverage (`GDI` draws
/// ellipses with no anti-aliasing at all).
unsafe fn composite_marker(bits: *mut u32, size: (i32, i32), cx: i32, cy: i32, radius: i32) {
    const BLACK: f32 = 2.0;
    const WHITE: f32 = 2.0;
    let (width, height) = size;
    let outer = radius as f32;
    let middle = outer - BLACK;
    let inner = middle - WHITE;
    let min_x = (cx - radius - 2).max(0);
    let max_x = (cx + radius + 2).min(width - 1);
    let min_y = (cy - radius - 2).max(0);
    let max_y = (cy + radius + 2).min(height - 1);
    // SAFETY: every index is within `0..width * height`.
    unsafe {
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x as f32 + 0.5) - (cx as f32 + 0.5);
                let dy = (y as f32 + 0.5) - (cy as f32 + 0.5);
                let distance = (dx * dx + dy * dy).sqrt();
                let black =
                    (coverage(distance, outer) - coverage(distance, middle)).clamp(0.0, 1.0);
                let white =
                    (coverage(distance, middle) - coverage(distance, inner)).clamp(0.0, 1.0);
                if black == 0.0 && white == 0.0 {
                    continue;
                }
                let index = (y * width + x) as usize;
                let pixel = *bits.add(index);
                let mut r = ((pixel >> 16) & 0xFF) as f32;
                let mut g = ((pixel >> 8) & 0xFF) as f32;
                let mut b = (pixel & 0xFF) as f32;
                r *= 1.0 - black;
                g *= 1.0 - black;
                b *= 1.0 - black;
                r += (255.0 - r) * white;
                g += (255.0 - g) * white;
                b += (255.0 - b) * white;
                *bits.add(index) =
                    ((r.round() as u32) << 16) | ((g.round() as u32) << 8) | b.round() as u32;
            }
        }
    }
}

/// Coverage of a half-open disc edge at `edge`: 1 fully inside, 0 fully
/// outside, a one-pixel-wide ramp across the boundary.
fn coverage(distance: f32, edge: f32) -> f32 {
    (edge + 0.5 - distance).clamp(0.0, 1.0)
}

unsafe fn paint_content(hdc: HDC, state: &PickerState) {
    let colors = UiColors::new(state.dark);
    let whole = Rect::new(0, 0, state.layout.size.0, state.layout.size.1);
    // SAFETY: `hdc` is a live paint DC; every GDI object created here is
    // selected out and deleted before this function returns.
    unsafe {
        fill_solid(hdc, whole, colors.body);
        let previous_font = SelectObject(hdc, HGDIOBJ(state.font.0));
        match state.panel {
            Panel::Palette => paint_palette(hdc, state, colors),
            Panel::Custom => paint_custom(hdc, state, colors),
        }
        let toggle = match state.panel {
            Panel::Palette => "Custom colors\u{2026}",
            Panel::Custom => "Colors",
        };
        draw_button(
            hdc,
            state.layout.toggle,
            toggle,
            colors,
            false,
            state.hover == Some(Region::Toggle),
        );
        draw_button(
            hdc,
            state.layout.cancel,
            "Cancel",
            colors,
            false,
            state.hover == Some(Region::Cancel),
        );
        draw_button(
            hdc,
            state.layout.ok,
            "OK",
            colors,
            true,
            state.hover == Some(Region::Ok),
        );
        SelectObject(hdc, previous_font);
    }
}

unsafe fn paint_palette(hdc: HDC, state: &PickerState, colors: UiColors) {
    // SAFETY: caller holds a live paint DC.
    unsafe {
        draw_label(hdc, state.layout.recent_label, "Recent colors", colors);
        for index in 0..state.recent.len().min(COLS) {
            let rect = state.layout.swatch_rect(state.layout.recent_grid, index);
            let selected = state.selected == Some(state.recent[index]);
            draw_swatch(
                hdc,
                rect,
                state.recent[index],
                selected,
                colors,
                state.layout,
                state.glyph_font,
            );
        }
        draw_label(hdc, state.layout.windows_label, "Windows colors", colors);
        for (index, color) in WINDOWS_COLORS.iter().enumerate() {
            let rect = state.layout.swatch_rect(state.layout.windows_grid, index);
            draw_swatch(
                hdc,
                rect,
                *color,
                state.selected == Some(*color),
                colors,
                state.layout,
                state.glyph_font,
            );
        }
    }
}

unsafe fn paint_custom(hdc: HDC, state: &PickerState, colors: UiColors) {
    let current = state.custom_color();
    let square = state.layout.saturation;
    let hue = state.hue;
    // SAFETY: caller holds a live paint DC.
    unsafe {
        fill_image(hdc, square, |x, y| {
            let s = x as f32 / (square.width - 1).max(1) as f32;
            let v = 1.0 - y as f32 / (square.height - 1).max(1) as f32;
            Hsv { h: hue, s, v }.to_color(255)
        });

        let hue_rect = state.layout.hue;
        fill_image(hdc, hue_rect, |x, _| {
            let h = x as f32 / (hue_rect.width - 1).max(1) as f32 * 360.0;
            Hsv { h, s: 1.0, v: 1.0 }.to_color(255)
        });

        let alpha_rect = state.layout.alpha;
        fill_image(hdc, alpha_rect, |x, y| {
            let a = x as f32 / (alpha_rect.width - 1).max(1) as f32;
            blend(checker_color(x, y, colors), current, a)
        });

        let preview = state.layout.preview;
        let preview_alpha = current.a as f32 / 255.0;
        fill_image(hdc, preview, |x, y| {
            blend(checker_color(x, y, colors), current, preview_alpha)
        });
        frame(hdc, preview, colors.divider);

        draw_hex(hdc, state.layout.hex, current, colors);
    }
}

/// The marker radius scaled with the rest of the layout.
fn scaled_marker(layout: PickerLayout, base: i32) -> i32 {
    let reference = COLS as i32 * SWATCH + (COLS as i32 - 1) * SWATCH_GAP + PAD * 2;
    (base as f32 * layout.size.0 as f32 / reference as f32).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_hits_both_ends() {
        let base = Color::rgb(0, 0, 0);
        let over = Color::rgb(255, 255, 255);
        assert_eq!(blend(base, over, 0.0), base);
        assert_eq!(blend(base, over, 1.0), over);
    }

    #[test]
    fn coverage_ramps_across_a_one_pixel_edge() {
        assert_eq!(coverage(0.0, 5.0), 1.0);
        assert_eq!(coverage(5.0, 5.0), 0.5);
        assert_eq!(coverage(5.5, 5.0), 0.0);
    }
}
