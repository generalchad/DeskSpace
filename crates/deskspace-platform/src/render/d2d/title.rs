//! The title bar's drawing: the icon, the title text or rename box, and the
//! lock glyph at the right.

use deskspace_core::space::DEFAULT_TITLE_HEIGHT;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_CLUSTER_METRICS, DWRITE_MEASURING_MODE_NATURAL, IDWriteFactory, IDWriteTextFormat,
    IDWriteTextLayout,
};

use super::edit::draw_edit_title;
use super::geometry::{color_f, draw_text, rect_f};
use super::{DrawContext, Painter, PreparedScene};

/// The ellipsis a title cut from its start begins with.
const ELLIPSIS: &[u16] = &[0x2026];

/// One DirectWrite cluster of a title: the UTF-16 units it spans and its
/// width. A cluster is what a cut may not split: a surrogate pair, a letter
/// with its accents, an emoji sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Cluster {
    length: usize,
    width: f32,
}

/// A title measured once, and its last cut. A width change recuts it from
/// the kept clusters without DirectWrite; only a new title measures again.
pub(super) struct TitleCut {
    full: Vec<u16>,
    clusters: Vec<Cluster>,
    ellipsis: f32,
    width: f32,
    text: Vec<u16>,
}

/// Where the kept end of a title of `clusters` starts, in UTF-16 units, so
/// it fits `room` after an ellipsis `ellipsis` wide: 0 when the whole title
/// fits, and the title's length (the ellipsis alone) when no cluster does.
/// Pure, so the cut is tested without DirectWrite.
fn keep_end_start(clusters: &[Cluster], ellipsis: f32, room: f32) -> usize {
    if clusters.iter().map(|cluster| cluster.width).sum::<f32>() <= room {
        return 0;
    }
    let mut start: usize = clusters.iter().map(|cluster| cluster.length).sum();
    let mut kept = ellipsis;
    for cluster in clusters.iter().rev() {
        if kept + cluster.width > room {
            break;
        }
        kept += cluster.width;
        start -= cluster.length;
    }
    start
}

/// `title` from `start` on, after an ellipsis; the whole title at 0.
fn cut_title(title: &[u16], start: usize) -> Vec<u16> {
    if start == 0 {
        return title.to_vec();
    }
    [ELLIPSIS, &title[start..]].concat()
}

/// `text` laid out on one line in `format`, wide enough never to trim.
fn one_line(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &[u16],
    height: f32,
) -> Option<IDWriteTextLayout> {
    // SAFETY: `text` and `format` are live for the call.
    unsafe { dwrite.CreateTextLayout(text, format, f32::MAX, height) }.ok()
}

/// The clusters of `text` in `format`, or `None` when DirectWrite cannot lay
/// it out.
fn clusters(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &[u16],
    height: f32,
) -> Option<Vec<Cluster>> {
    let layout = one_line(dwrite, format, text, height)?;
    let mut count = 0u32;
    // SAFETY: a null array asks only for the count; the call then fails with
    // "not sufficient buffer", which is expected.
    unsafe {
        let _ = layout.GetClusterMetrics(None, &mut count);
    }
    let mut metrics = vec![DWRITE_CLUSTER_METRICS::default(); count as usize];
    // SAFETY: the slice is sized to the count just reported.
    unsafe { layout.GetClusterMetrics(Some(&mut metrics), &mut count) }.ok()?;
    Some(
        metrics
            .iter()
            .map(|cluster| Cluster {
                length: usize::from(cluster.length),
                width: cluster.width,
            })
            .collect(),
    )
}

impl Painter {
    /// `title` cut from its start to fit `width`. A title is measured once,
    /// in its own `format` (laid out wide, so its trimming never applies),
    /// and recut by arithmetic as the width changes, so a resize lays
    /// nothing out. A title DirectWrite cannot measure is drawn whole, and
    /// the format trims it at the end as before.
    fn title_keeping_end<'a>(
        &'a mut self,
        dwrite: &IDWriteFactory,
        format: &IDWriteTextFormat,
        title: &'a [u16],
        width: f32,
        height: f32,
    ) -> &'a [u16] {
        if self.title_cut.as_ref().is_none_or(|cut| cut.full != title) {
            let measured = clusters(dwrite, format, title, height).zip(
                clusters(dwrite, format, ELLIPSIS, height)
                    .map(|marks| marks.iter().map(|mark| mark.width).sum()),
            );
            let (clusters, ellipsis) = measured.unwrap_or_default();
            self.title_cut = Some(TitleCut {
                full: title.to_vec(),
                clusters,
                ellipsis,
                width: f32::NAN,
                text: Vec::new(),
            });
        }
        let Some(cut) = self.title_cut.as_mut() else {
            return title;
        };
        if cut.width != width {
            cut.width = width;
            cut.text = cut_title(title, keep_end_start(&cut.clusters, cut.ellipsis, width));
        }
        &cut.text
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
            let title = if scene.title_keeps_end {
                painter.title_keeping_end(
                    &graphics.dwrite,
                    &graphics.title_format,
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

    /// Cuts `text` to `room` units with a measure of one unit per character
    /// and per ellipsis, each `char` being one cluster.
    fn cut_to(text: &str, room: f32) -> String {
        let title: Vec<u16> = text.encode_utf16().collect();
        let clusters: Vec<Cluster> = text
            .chars()
            .map(|ch| Cluster {
                length: ch.len_utf16(),
                width: 1.0,
            })
            .collect();
        String::from_utf16(&cut_title(&title, keep_end_start(&clusters, 1.0, room))).unwrap()
    }

    #[test]
    fn a_title_that_fits_is_kept_whole() {
        assert_eq!(cut_to("Downloads", 9.0), "Downloads");
        assert_eq!(cut_to("", 0.0), "", "an empty title");
    }

    #[test]
    fn a_long_title_keeps_its_end() {
        assert_eq!(cut_to("Downloads › 2026 › Invoices", 12.0), "… › Invoices");
        assert_eq!(cut_to("Downloads › Invoices", 1.0), "…");
        assert_eq!(
            cut_to("Downloads", 0.0),
            "…",
            "nothing fits but the ellipsis"
        );
    }

    #[test]
    fn a_cut_never_splits_a_cluster() {
        // "𝄞" is a surrogate pair, and "é" written as "e" and a combining
        // accent is one cluster of two units: a cut keeps either whole.
        assert_eq!(cut_to("ab𝄞cd", 4.0), "…𝄞cd");
        assert_eq!(cut_to("ab𝄞cd", 3.0), "…cd");
        let accented = [
            Cluster {
                length: 1,
                width: 1.0,
            },
            Cluster {
                length: 2,
                width: 1.0,
            },
            Cluster {
                length: 1,
                width: 1.0,
            },
        ];
        let title: Vec<u16> = "ae\u{301}x".encode_utf16().collect();
        // A half-wide ellipsis leaves room for "é" and "x" but not "a".
        assert_eq!(
            cut_title(&title, keep_end_start(&accented, 0.5, 2.6)),
            "…e\u{301}x".encode_utf16().collect::<Vec<_>>()
        );
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
