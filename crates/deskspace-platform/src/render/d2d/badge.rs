//! Draws the debug badge onto a copy of the application icon.

use super::*;

/// Segoe Fluent Icons / Segoe MDL2 Assets "Bug" glyph.
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const BUG_GLYPH: &[u16] = &[0xEBE8];
/// The disc's diameter, as a share of the icon's side.
const DISC_SHARE: f32 = 0.6;
/// The bug's em size, as a share of the disc's diameter.
const GLYPH_SHARE: f32 = 0.75;
/// A deep amber, so the white bug keeps its contrast.
const DISC_COLOR: Color = Color::rgb(0xD9, 0x77, 0x06);
const GLYPH_COLOR: Color = Color::rgb(0xFF, 0xFF, 0xFF);
/// A dark ring that separates the disc from the icon and any taskbar.
const RING_COLOR: Color = Color::rgba(0x00, 0x00, 0x00, 0xA0);

/// `icon` with a white bug on an amber disc in its bottom-right corner, as
/// premultiplied BGRA. Needs COM on this thread (for WIC).
pub(crate) fn debug_badge(icon: &Icon) -> Option<Icon> {
    let (width, height) = (icon.width(), icon.height());
    let side = width.min(height) as f32;
    let graphics = graphics().ok()?;
    let ring = (side / 32.0).max(1.0);
    let diameter = side * DISC_SHARE;
    let radius = diameter / 2.0 - ring / 2.0;
    let center = Vector2 {
        X: width as f32 - diameter / 2.0,
        Y: height as f32 - diameter / 2.0,
    };
    let disc = D2D1_ELLIPSE {
        point: center,
        radiusX: radius,
        radiusY: radius,
    };
    // SAFETY: the WIC bitmap is created from `icon`'s own pixels with the
    // matching stride and format; it and its render target live for this
    // call only, drawing happens between BeginDraw and EndDraw, and the
    // pixels are copied out afterwards.
    unsafe {
        let pixels = graphics
            .wic
            .CreateBitmapFromMemory(
                width,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                width * 4,
                icon.pixels(),
            )
            .ok()?;
        let target = graphics
            .d2d
            .CreateWicBitmapRenderTarget(&pixels, &render_target_properties())
            .ok()?;
        target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        // Transparent background: ClearType needs an opaque one.
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        let brush = target
            .CreateSolidColorBrush(&color_f(DISC_COLOR), None)
            .ok()?;

        target.BeginDraw();
        target.FillEllipse(&disc, &brush);
        brush.SetColor(&color_f(RING_COLOR));
        target.DrawEllipse(&disc, &brush, ring, None);
        if let Some(format) = &graphics.glyph_format {
            // The shared glyph format has a fixed em size, so the glyph is
            // scaled about the disc's center to fit it instead.
            let scale = diameter * GLYPH_SHARE / format.GetFontSize();
            target.SetTransform(&Matrix3x2::scale_around(scale, scale, center));
            brush.SetColor(&color_f(GLYPH_COLOR));
            let half = diameter / 2.0;
            target.DrawText(
                BUG_GLYPH,
                format,
                &rect_f(
                    center.X - half,
                    center.Y - half,
                    center.X + half,
                    center.Y + half,
                ),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        target.EndDraw(None, None).ok()?;
        to_icon(&graphics, &pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    fn pixel(icon: &Icon, x: u32, y: u32) -> &[u8] {
        let at = ((y * icon.width() + x) * 4) as usize;
        &icon.pixels()[at..at + 4]
    }

    #[test]
    fn the_badge_fills_the_bottom_right_corner_only() {
        // SAFETY: initialising COM for this test thread, as WIC needs.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let side = 32;
        let clear = Icon::new(side, side, vec![0; (side * side * 4) as usize].into());

        let badged = debug_badge(&clear).expect("badge");

        assert_eq!((badged.width(), badged.height()), (side, side));
        assert_eq!(pixel(&badged, 2, 2)[3], 0, "the top-left stays clear");
        let disc_edge = pixel(&badged, side - 18, side - 10);
        assert_eq!(disc_edge[3], 255, "the disc is opaque near its edge");
    }
}
