//! The GDI primitives the picker and the settings window paint with.

use super::layout::PickerLayout;
use super::*;

fn rect_to_win(rect: Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}

pub(super) unsafe fn fill_solid(hdc: HDC, rect: Rect, color: Color) {
    // SAFETY: the brush is deleted before returning; FillRect leaves it
    // selected in no DC.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let win = rect_to_win(rect);
        let _ = FillRect(hdc, &win, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

pub(super) unsafe fn frame(hdc: HDC, rect: Rect, color: Color) {
    // SAFETY: a one-pixel outline drawn with a temporary brush, deleted before
    // returning.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let win = rect_to_win(rect);
        let _ = FrameRect(hdc, &win, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Draws a color preview: a slightly rounded fill with a one-pixel border.
/// Shared by the picker's swatches and the settings window's color fields.
pub(crate) unsafe fn draw_color_box(
    hdc: HDC,
    rect: Rect,
    color: Color,
    radius: i32,
    border: Color,
) {
    // SAFETY: the brush and pen are selected out and deleted before returning.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let pen = CreatePen(PS_SOLID, 1, colorref(border));
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Draws a hollow rounded outline of `width` just inside `rect`.
unsafe fn draw_ring(hdc: HDC, rect: Rect, color: Color, width: i32, radius: i32) {
    // SAFETY: the pen is selected out and deleted; the stock null brush fills
    // nothing.
    unsafe {
        let pen = CreatePen(PS_SOLID, width, colorref(color));
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

/// Draws one palette swatch: its color in a slightly rounded box, a defining
/// outline, and, when selected, an accent ring and a check mark.
pub(super) unsafe fn draw_swatch(
    hdc: HDC,
    rect: Rect,
    color: Color,
    selected: bool,
    colors: UiColors,
    layout: PickerLayout,
    glyph_font: HFONT,
) {
    // SAFETY: caller holds a live paint DC.
    unsafe {
        draw_color_box(hdc, rect, color, layout.swatch_radius, colors.divider);
        if selected {
            draw_ring(hdc, rect, colors.accent, 2, layout.swatch_radius);
            draw_check(hdc, rect, color, glyph_font);
        }
    }
}

/// Draws the selection check mark as an anti-aliased icon-font glyph, in black
/// or white to contrast with `color`.
unsafe fn draw_check(hdc: HDC, rect: Rect, color: Color, glyph_font: HFONT) {
    let ink = if color.luminance() < 0.5 {
        Color::rgb(255, 255, 255)
    } else {
        Color::rgb(0, 0, 0)
    };
    // SAFETY: `hdc` is a live paint DC.
    unsafe { draw_glyph(hdc, glyph_font, GLYPH_CHECK, rect_to_win(rect), ink) };
}

pub(super) unsafe fn draw_label(hdc: HDC, rect: Rect, text: &str, colors: UiColors) {
    // SAFETY: the text buffer and rectangle outlive the call.
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(colors.text));
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

pub(super) unsafe fn draw_hex(hdc: HDC, rect: Rect, color: Color, colors: UiColors) {
    // SAFETY: the text buffer and rectangle outlive the call.
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(colors.text));
        let text = format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        );
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        text_rect.left += 2;
        let _ = DrawTextW(hdc, &mut wide, &mut text_rect, DT_VCENTER | DT_SINGLELINE);
    }
}

pub(super) unsafe fn draw_button(
    hdc: HDC,
    rect: Rect,
    label: &str,
    colors: UiColors,
    accent: bool,
    hovered: bool,
) {
    let (fill, text) = if accent {
        (colors.accent, Color::rgb(255, 255, 255))
    } else if hovered {
        (colors.divider, colors.text)
    } else {
        (colors.command_bar, colors.text)
    };
    // SAFETY: the brush and pen are selected out and deleted before returning;
    // the text buffer lives for the call.
    unsafe {
        let brush = CreateSolidBrush(colorref(fill));
        let pen = CreatePen(PS_SOLID, 1, colorref(fill));
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            BUTTON_ROUND * 2,
            BUTTON_ROUND * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(text));
        let mut wide: Vec<u16> = label.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rect,
            windows::Win32::Graphics::Gdi::DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

/// Fills `dest` pixel by pixel from `color_at`, drawn through a top-down 32bpp
/// DIB so a smooth gradient costs one `BitBlt` rather than per-pixel drawing.
pub(super) unsafe fn fill_image(hdc: HDC, dest: Rect, color_at: impl Fn(i32, i32) -> Color) {
    let width = dest.width.max(1);
    let height = dest.height.max(1);
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
    // SAFETY: `info` is fully initialised; `bits` receives the DIB's pixels.
    unsafe {
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let memory = CreateCompatibleDC(Some(hdc));
        let Ok(bitmap) = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(memory);
            return;
        };
        if !bits.is_null() {
            let pixels = bits as *mut u32;
            for y in 0..height {
                for x in 0..width {
                    let color = color_at(x, y);
                    // BI_RGB stores bytes B, G, R, A, so little-endian u32 is
                    // A << 24 | R << 16 | G << 8 | B.
                    *pixels.add((y * width + x) as usize) = (color.a as u32) << 24
                        | (color.r as u32) << 16
                        | (color.g as u32) << 8
                        | color.b as u32;
                }
            }
        }
        let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
        let _ = BitBlt(
            hdc,
            dest.x,
            dest.y,
            width,
            height,
            Some(memory),
            0,
            0,
            SRCCOPY,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
    }
}
