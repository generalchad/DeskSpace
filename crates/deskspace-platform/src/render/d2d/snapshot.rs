//! Renders a space's scene offscreen once, at the start of an animation, so
//! an effect can pull the real contents apart (`render::snapshot`). It draws
//! through the same painter as the window, so the snapshot is what the space
//! shows: tint, title, items, and border.

use super::bitmap::to_pixels;
use super::graphics::graphics;
use super::*;
use crate::render::snapshot::Snapshot;

/// `prepared` at `width` by `height` window pixels, with `frame`'s radius and
/// border, or `None` when the device or the bitmap cannot be made. Needs COM
/// on this thread (for WIC).
pub(crate) fn capture(
    prepared: &PreparedScene,
    frame: PaintFrame,
    width: u32,
    height: u32,
) -> Option<Snapshot> {
    if width == 0 || height == 0 {
        return None;
    }
    let graphics = graphics().ok()?;
    let properties = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    // SAFETY: the WIC bitmap and its render target live for this call only;
    // drawing happens between BeginDraw and EndDraw, and the pixels are copied
    // out afterwards.
    unsafe {
        let pixels = graphics
            .wic
            .CreateBitmap(
                width,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )
            .ok()?;
        let target = graphics
            .d2d
            .CreateWicBitmapRenderTarget(&pixels, &properties)
            .ok()?;
        target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        // Transparent background: ClearType needs an opaque one.
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        let brush = target
            .CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)
            .ok()?;
        let ctx = DrawContext {
            graphics: &graphics,
            target: &target,
            brush: &brush,
        };
        let mut painter = Painter::default();
        // A top-level space clears to transparent and draws its tint; the
        // snapshot keeps that body even for a hosted window, whose window
        // paints opaque.
        let frame = PaintFrame {
            opaque: false,
            ..frame
        };
        target.BeginDraw();
        target.Clear(Some(&color_f(Color::TRANSPARENT)));
        painter.draw_scene(ctx, prepared, frame, (width as f32, height as f32));
        target.EndDraw(None, None).ok()?;
        drop(target);
        let (captured_width, captured_height, bytes) = to_pixels(&graphics, &pixels)?;
        if (captured_width, captured_height) != (width, height) {
            return None;
        }
        let (chunks, _) = bytes.as_chunks::<4>();
        let pixels: Box<[u32]> = chunks
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect();
        Some(Snapshot::new(width, height, pixels))
    }
}
