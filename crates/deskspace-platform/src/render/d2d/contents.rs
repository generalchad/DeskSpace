//! Renders a space's title text and items once, as separate sprites for Smoke
//! (`render::contents`). The tint, title-bar fill, and border are left to the
//! window; this is only what flies in on the smoke's current.

use super::bitmap::to_pixels;
use super::graphics::graphics;
use super::*;
use crate::render::contents::{Contents, Sprite};
use crate::render::snapshot::Snapshot;

/// `prepared`'s title text and items at `width` by `height` window pixels, or
/// `None` when the device or the bitmap cannot be made. Needs COM on this
/// thread (for WIC).
pub(crate) fn capture(prepared: &PreparedScene, width: u32, height: u32) -> Option<Contents> {
    if width == 0 || height == 0 {
        return None;
    }
    let scene = prepared.scene();
    let title_height = scene.title_height.max(0) as u32;
    let mut sprites = Vec::new();
    if scene.chrome && title_height > 0 {
        sprites.push(Sprite {
            target: Rect::new(0, 0, width as i32, (title_height).min(height) as i32),
            title: true,
        });
    }
    if !scene.folded {
        sprites.extend(
            scene
                .items
                .iter()
                .filter(|item| !item.rect.is_empty())
                .map(|item| Sprite {
                    target: item.rect,
                    title: false,
                }),
        );
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
        target.BeginDraw();
        target.Clear(Some(&color_f(Color::TRANSPARENT)));
        if scene.chrome && title_height > 0 {
            painter.draw_title(ctx, prepared, width as f32, title_height as f32);
        }
        if !scene.folded {
            // The window clips items below the title bar; do the same here.
            let content_top = if scene.chrome {
                title_height as f32
            } else {
                0.0
            };
            target.PushAxisAlignedClip(
                &rect_f(0.0, content_top, width as f32, height as f32),
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
            painter.draw_items(ctx, prepared);
            target.PopAxisAlignedClip();
        }
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
        Some(Contents::new(Snapshot::new(width, height, pixels), sprites))
    }
}
