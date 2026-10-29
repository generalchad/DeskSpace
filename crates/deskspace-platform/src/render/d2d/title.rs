//! The title bar's drawing: the icon, the title text or rename box, and the
//! lock glyph at the right.

use deskspace_core::space::DEFAULT_TITLE_HEIGHT;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXT_METRICS, IDWriteFactory, IDWriteTextFormat,
};

use super::edit::draw_edit_title;
use super::geometry::{color_f, draw_text, rect_f};
use super::{DrawContext, Painter, PreparedScene};

/// The ellipsis a title cut from its start begins with.
const ELLIPSIS: u16 = 0x2026;

/// A title cut from its start to fit a width, kept until either changes.
pub(super) struct TitleCut {
    full: Vec<u16>,
    width: f32,
    text: Vec<u16>,
}

/// `text` when it `fits`, else `…` and the longest end of `text` that fits.
/// Pure: the caller measures, so the search is tested without DirectWrite.
fn keep_end(text: &[u16], mut fits: impl FnMut(&[u16]) -> bool) -> Vec<u16> {
    if fits(text) {
        return text.to_vec();
    }
    let cut = |start: usize| {
        let mut kept = Vec::with_capacity(text.len() - start + 1);
        kept.push(ELLIPSIS);
        kept.extend_from_slice(&text[start..]);
        kept
    };
    // Keeping less never makes the text wider, so the first start that fits
    // is found by bisection.
    let (mut low, mut high) = (1, text.len());
    while low < high {
        let middle = (low + high) / 2;
        if fits(&cut(middle)) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let mut start = low;
    // Never keep the second half of a surrogate pair without its first.
    while start < text.len() && (0xDC00..=0xDFFF).contains(&text[start]) {
        start += 1;
    }
    cut(start)
}

/// The width `text` takes in `format` on one line, or `None` when DirectWrite
/// cannot lay it out.
fn text_width(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &[u16],
    height: f32,
) -> Option<f32> {
    // SAFETY: `text` and `format` are live for the call, and the layout is
    // only measured.
    unsafe {
        let layout = dwrite
            .CreateTextLayout(text, format, f32::MAX, height)
            .ok()?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        layout.GetMetrics(&mut metrics).ok()?;
        Some(metrics.widthIncludingTrailingWhitespace)
    }
}

impl Painter {
    /// `title` cut from its start to fit `width`. It is measured once per
    /// title and width, then kept, so a repaint lays nothing out.
    fn title_keeping_end<'a>(
        &'a mut self,
        dwrite: &IDWriteFactory,
        format: &IDWriteTextFormat,
        title: &'a [u16],
        width: f32,
        height: f32,
    ) -> &'a [u16] {
        let current = self
            .title_cut
            .as_ref()
            .is_some_and(|cut| cut.full == title && cut.width == width);
        if !current {
            let text = keep_end(title, |text| {
                text_width(dwrite, format, text, height).is_none_or(|used| used <= width)
            });
            self.title_cut = Some(TitleCut {
                full: title.to_vec(),
                width,
                text,
            });
        }
        self.title_cut
            .as_ref()
            .map_or(title, |cut| cut.text.as_slice())
    }
}

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
            // The edit format has no ellipsis trimming, so it measures the
            // whole text in the title's font.
            let title = if scene.title_keeps_end {
                painter.title_keeping_end(
                    &graphics.dwrite,
                    &graphics.edit_format,
                    &prepared.title,
                    text.right - text.left,
                    title_height,
                )
            } else {
                &prepared.title
            };
            draw_text(
                target,
                brush,
                title,
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

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    /// Cuts `text` to `room` characters with a measure of one unit each.
    fn cut_to(text: &str, room: usize) -> String {
        String::from_utf16(&keep_end(&wide(text), |kept| kept.len() <= room)).unwrap()
    }

    #[test]
    fn a_title_that_fits_is_kept_whole() {
        assert_eq!(cut_to("Downloads", 9), "Downloads");
    }

    #[test]
    fn a_long_title_keeps_its_end() {
        assert_eq!(cut_to("Downloads › 2026 › Invoices", 12), "… › Invoices");
        assert_eq!(cut_to("Downloads › Invoices", 1), "…");
        assert_eq!(cut_to("Downloads", 0), "…", "nothing fits but the ellipsis");
    }

    #[test]
    fn a_cut_never_splits_a_surrogate_pair() {
        // "𝄞" is a surrogate pair; a room that would keep only its second
        // half drops the whole character instead.
        assert_eq!(cut_to("ab𝄞cd", 4), "…cd");
        assert_eq!(cut_to("ab𝄞cd", 5), "…𝄞cd");
    }

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
