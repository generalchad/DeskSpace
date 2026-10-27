//! The title bar's drawing: the icon, the title text or rename box, and the
//! lock glyph at the right.

use deskspace_core::space::DEFAULT_TITLE_HEIGHT;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use super::edit::draw_edit_title;
use super::geometry::{color_f, draw_text, rect_f};
use super::{DrawContext, Painter, PreparedScene};

/// The title text's inset from the left edge, at 96 DPI, when the space has
/// no icon.
const TITLE_INSET: f32 = 12.0;
/// The gap between the title icon and the text, at 96 DPI: 6 px at the 28 px
/// title, so it scales with the bar like the icon does.
const TITLE_ICON_GAP: f32 = 6.0;
/// Segoe Fluent Icons / Segoe MDL2 Assets "Lock" glyph.
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const LOCK_GLYPH: &[u16] = &[0xE72E];
/// The lock glyph's alpha, and its gap from the title bar's right edge.
const LOCK_ALPHA: u8 = 170;
const LOCK_MARGIN: f32 = 4.0;

/// Where a title bar's parts go, in the space's coordinates. Pure, so the
/// layout is tested apart from the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TitleLayout {
    /// The title text's box, or the rename box's.
    text: D2D_RECT_F,
    /// The lock glyph's square at the right, when the space is locked.
    lock: Option<D2D_RECT_F>,
}

impl TitleLayout {
    /// The layout of a title bar `width` by `height`, with a title icon
    /// ending at `icon_right` (`None` when the space has none).
    fn new(width: f32, height: f32, locked: bool, icon_right: Option<f32>) -> Self {
        let lock_width = if locked { height } else { 0.0 };
        let left = match icon_right {
            Some(right) => right + TITLE_ICON_GAP * (height / DEFAULT_TITLE_HEIGHT as f32),
            None => TITLE_INSET,
        };
        let text_right = (width - TITLE_INSET - lock_width).max(left);
        let lock_right = width - LOCK_MARGIN;
        Self {
            text: rect_f(left, 0.0, text_right, height),
            lock: locked.then(|| rect_f(lock_right - lock_width, 0.0, lock_right, height)),
        }
    }
}

/// Draws the title bar's icon (once its image is ready), the title text or
/// rename box, and the lock glyph.
///
/// # Safety
/// Call between `BeginDraw` and `EndDraw` on `ctx.target`.
pub(super) unsafe fn draw_title(
    painter: &mut Painter,
    ctx: DrawContext,
    prepared: &PreparedScene,
    width: f32,
    title_height: f32,
) {
    let DrawContext {
        graphics,
        target,
        brush,
    } = ctx;
    let scene = &prepared.scene;
    let title_icon = scene.title_icon.as_ref();
    let bitmap = title_icon
        .and_then(|title| title.image.as_ref())
        .and_then(|icon| painter.bitmap_for(target, icon));
    let layout = TitleLayout::new(
        width,
        title_height,
        scene.locked,
        title_icon.map(|title| title.rect.right() as f32),
    );
    let text = layout.text;
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if let (Some(bitmap), Some(title)) = (bitmap, title_icon) {
            let icon = title.rect;
            target.DrawBitmap(
                &bitmap,
                Some(&rect_f(
                    icon.x as f32,
                    icon.y as f32,
                    icon.right() as f32,
                    icon.bottom() as f32,
                )),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        if scene.editing.is_some() {
            draw_edit_title(ctx, prepared, text.left, text.right, title_height);
        } else {
            draw_text(
                target,
                brush,
                &prepared.title,
                &graphics.title_format,
                text,
                scene.title_text,
                scene.text_shadow,
            );
        }
        if let (Some(lock), Some(format)) = (layout.lock, &graphics.glyph_format) {
            brush.SetColor(&color_f(scene.title_text.fade(LOCK_ALPHA)));
            target.DrawText(
                LOCK_GLYPH,
                format,
                &lock,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unlocked_title_spans_the_bar_inside_its_insets() {
        let layout = TitleLayout::new(240.0, 28.0, false, None);
        assert_eq!(
            layout.text,
            rect_f(TITLE_INSET, 0.0, 240.0 - TITLE_INSET, 28.0)
        );
        assert_eq!(layout.lock, None);
    }

    #[test]
    fn an_icon_pushes_the_title_past_its_gap() {
        // A 10 px inset and a 16 px icon: the text starts at 10 + 16 + 6.
        let layout = TitleLayout::new(240.0, 28.0, false, Some(26.0));
        assert_eq!(layout.text.left, 32.0);
        // A 42 px bar scales the gap like the icon.
        let scaled = TitleLayout::new(240.0, 42.0, false, Some(39.0));
        assert_eq!(scaled.text.left, 48.0);
    }

    #[test]
    fn a_locked_title_leaves_a_square_for_the_lock_at_the_right() {
        let layout = TitleLayout::new(240.0, 28.0, true, None);
        let lock = layout.lock.expect("a lock");
        assert_eq!(lock.right, 240.0 - LOCK_MARGIN);
        assert_eq!(lock.right - lock.left, 28.0, "square");
        assert_eq!(layout.text.right, 240.0 - TITLE_INSET - 28.0);
    }

    #[test]
    fn a_narrow_title_never_turns_inside_out() {
        let layout = TitleLayout::new(20.0, 28.0, true, None);
        assert!(layout.text.right >= layout.text.left);
    }
}
