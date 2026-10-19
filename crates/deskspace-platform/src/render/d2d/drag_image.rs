//! Renders a group of items into one bitmap: the drag image.

use super::*;

/// Renders the items `ids` of `scene` together into one premultiplied bitmap,
/// keeping their relative positions and scaling the whole group by `scale`
/// about `origin` (scene coordinates). Used for drag images, so a group drag
/// shows every selected item laid out as Explorer shows it. Needs COM on this
/// thread (for WIC).
pub(crate) fn render_items(
    scene: &SpaceScene,
    ids: &[u64],
    origin: Rect,
    scale: f32,
) -> Option<Icon> {
    let width = ((origin.width as f32 * scale).ceil() as u32).max(1);
    let height = ((origin.height as f32 * scale).ceil() as u32).max(1);
    let graphics = graphics().ok()?;
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
            .CreateWicBitmapRenderTarget(&pixels, &render_target_properties())
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
        let mut scene = scene.clone();
        scene.show_check_boxes = false;

        target.BeginDraw();
        target.Clear(Some(&color_f(Color::TRANSPARENT)));
        // Map scene coordinates into the canvas: p' = (p - origin) * scale.
        // https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-transforms-overview
        target.SetTransform(&Matrix3x2 {
            M31: -(origin.x as f32) * scale,
            M32: -(origin.y as f32) * scale,
            ..Matrix3x2::scale(scale, scale)
        });
        for id in ids {
            let Some(index) = scene.items.iter().position(|item| item.id == *id) else {
                continue;
            };
            let mut item = scene.items[index].clone();
            // The drag image is the item itself, without the hover highlight,
            // cut ghosting, or a check box that only appears while pointing.
            item.state = ItemState::Normal;
            item.hovered = false;
            item.checked = false;
            item.cut = false;
            let label: Vec<u16> = item.label.encode_utf16().collect();
            let icon = item
                .icon
                .as_ref()
                .and_then(|icon| upload_icon(&target, icon));
            draw_item(ctx, &scene, &item, &label, icon.as_ref(), None);
        }
        target.EndDraw(None, None).ok()?;
        drop(target);
        to_icon(&graphics, &pixels)
    }
}
