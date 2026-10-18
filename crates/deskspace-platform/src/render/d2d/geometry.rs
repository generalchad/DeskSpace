//! Small drawing primitives: filled rounded rectangles, clipped text,
//! and the color and rectangle conversions.

use super::*;

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

#[cfg(test)]
mod tests {
    use super::*;

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
