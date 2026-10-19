//! The GDI color box the settings window draws its color fields with.

use deskspace_core::geometry::Rect;
use deskspace_core::theme::Color;
use windows::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, HDC, HGDIOBJ, PS_SOLID, RoundRect, SelectObject,
};

use crate::win32::settings_window::colorref;

/// Draws a color preview: a slightly rounded fill with a one-pixel border.
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
