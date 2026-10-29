//! Small drawing primitives: filled rounded rectangles, clipped text, the
//! color and rectangle conversions, and the pixel format every target and
//! bitmap uses.

use super::*;

/// 32-bit BGRA with premultiplied alpha: every render target, bitmap, and
/// surface the painters use.
/// https://learn.microsoft.com/en-us/windows/win32/direct2d/supported-pixel-formats-and-alpha-modes
pub(super) const PREMULTIPLIED_BGRA: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

/// 96 DPI makes one DIP one physical pixel, matching the model's pixel
/// coordinates.
pub(super) const PIXEL_DPI: f32 = 96.0;

/// A bitmap's properties: `PREMULTIPLIED_BGRA` at `PIXEL_DPI`.
pub(super) fn bitmap_properties() -> D2D1_BITMAP_PROPERTIES {
    D2D1_BITMAP_PROPERTIES {
        pixelFormat: PREMULTIPLIED_BGRA,
        dpiX: PIXEL_DPI,
        dpiY: PIXEL_DPI,
    }
}

/// A render target's properties: `PREMULTIPLIED_BGRA` at `PIXEL_DPI`, on
/// whatever hardware or software Direct2D picks.
pub(super) fn render_target_properties() -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: PREMULTIPLIED_BGRA,
        dpiX: PIXEL_DPI,
        dpiY: PIXEL_DPI,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

/// `icon` uploaded to the GPU through `target`.
pub(super) fn upload_icon(target: &ID2D1RenderTarget, icon: &Icon) -> Option<ID2D1Bitmap> {
    // SAFETY: `Icon::new` guarantees `pixels` holds width * height BGRA
    // pixels, so `width * 4` is the row pitch; D2D copies the data.
    unsafe {
        target
            .CreateBitmap(
                D2D_SIZE_U {
                    width: icon.width(),
                    height: icon.height(),
                },
                Some(icon.pixels().as_ptr() as *const core::ffi::c_void),
                icon.width() * 4,
                &bitmap_properties(),
            )
            .ok()
    }
}

pub(super) unsafe fn fill_rounded(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    radius: f32,
    color: Color,
) {
    if color.a == 0 {
        return;
    }
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        brush.SetColor(&color_f(color));
        if radius > 0.0 {
            target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
            );
        } else {
            target.FillRectangle(&rect, brush);
        }
    }
}

/// Draws clipped text, first offset by one pixel in `shadow` when given.
pub(super) unsafe fn draw_text(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    text: &[u16],
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    color: Color,
    shadow: Option<Color>,
) {
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if let Some(shadow) = shadow {
            brush.SetColor(&color_f(shadow));
            target.DrawText(
                text,
                format,
                &rect_f(
                    rect.left + 1.0,
                    rect.top + 1.0,
                    rect.right + 1.0,
                    rect.bottom + 1.0,
                ),
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        brush.SetColor(&color_f(color));
        target.DrawText(
            text,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
}

pub(super) fn color_f(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r as f32 / 255.0,
        g: color.g as f32 / 255.0,
        b: color.b as f32 / 255.0,
        a: color.a as f32 / 255.0,
    }
}

pub(super) fn rect_f(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

pub(super) fn to_rect_f(rect: Rect) -> D2D_RECT_F {
    rect_f(
        rect.x as f32,
        rect.y as f32,
        rect.right() as f32,
        rect.bottom() as f32,
    )
}

pub(super) fn inset(rect: D2D_RECT_F, amount: f32) -> D2D_RECT_F {
    rect_f(
        rect.left + amount,
        rect.top + amount,
        rect.right - amount,
        rect.bottom - amount,
    )
}

/// The notice bar's gap from the item area's edges.
const NOTICE_MARGIN: f32 = 8.0;

/// Where a space's notice bar sits in its item `area`: along the bottom, a
/// margin in from each edge, two title bars high so a path can wrap to a
/// second line, and never above the area's top.
pub(super) fn notice_rect(area: D2D_RECT_F, title_height: f32) -> D2D_RECT_F {
    let bar = inset(area, NOTICE_MARGIN);
    rect_f(
        bar.left,
        (bar.bottom - 2.0 * title_height).max(bar.top),
        bar.right,
        bar.bottom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_sits_along_the_bottom_within_the_area() {
        let bar = notice_rect(rect_f(0.0, 32.0, 300.0, 232.0), 32.0);
        assert_eq!(
            (bar.left, bar.top, bar.right, bar.bottom),
            (8.0, 160.0, 292.0, 224.0)
        );
        let short = notice_rect(rect_f(0.0, 32.0, 300.0, 72.0), 32.0);
        assert_eq!((short.top, short.bottom), (40.0, 64.0));
    }

    #[test]
    fn color_conversion_is_straight_alpha() {
        let converted = color_f(Color::rgba(255, 0, 51, 128));
        assert_eq!(converted.r, 1.0);
        assert_eq!(converted.g, 0.0);
        assert!((converted.b - 0.2).abs() < 1e-6);
        assert!((converted.a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn inset_shrinks_every_edge() {
        let shrunk = inset(rect_f(0.0, 0.0, 10.0, 10.0), 2.0);
        assert_eq!(
            (shrunk.left, shrunk.top, shrunk.right, shrunk.bottom),
            (2.0, 2.0, 8.0, 8.0)
        );
    }
}
