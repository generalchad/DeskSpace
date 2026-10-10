//! The picture that follows the pointer while items are dragged.
//!
//! It is the selected items exactly as their space draws them (icons, labels)
//! at their grid positions, rendered offscreen as one bitmap and made slightly
//! translucent, like the group Explorer picks up. The translucency is baked
//! into the pixels, so the same bitmap looks identical wherever it is shown.
//!
//! One `DragImage` serves both drag visuals: the shell's drag-image manager
//! (`IDragSourceHelper`, for drags that leave a space) and the layered ghost
//! window (for rearranging inside a space). They want different alpha: the
//! layered window composites premultiplied pixels, while
//! `InitializeFromBitmap` premultiplies again itself, so it gets straight
//! alpha.

use deskspace_core::geometry::{Point, Rect};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, HBITMAP,
};

use crate::render::{Icon, SpaceScene, d2d};

/// Opacity of the drag image, as a fraction of 255.
const DRAG_IMAGE_ALPHA: u8 = 200;

/// Longest edge of a group drag image before it is scaled down, so a large
/// selection becomes a compact picture as Explorer's does while ordinary
/// selections stay at full size.
const MAX_DRAG_IMAGE: f32 = 512.0;

/// An item's drag picture and the point inside it that stays under the
/// pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragImage {
    pub icon: Icon,
    pub hotspot: Point,
}

impl DragImage {
    /// Renders the selected items `ids` of `scene` as one image, at their grid
    /// positions and scaled down to `MAX_DRAG_IMAGE`, like Explorer's group
    /// drag image. `ids` must include any item scrolled out of view so the
    /// image keeps their place in the grid. `grab` is where the pointer went
    /// down, in the scene's window coordinates; it stays under the pointer.
    /// `None` when none of `ids` is in the scene.
    pub fn of_selection(scene: &SpaceScene, ids: &[u64], grab: Point) -> Option<Self> {
        let origin = selection_bounds(scene, ids)?;
        let scale = group_scale(origin);
        let rendered = d2d::render_items(scene, ids, origin, scale)?;
        let mut pixels = rendered.pixels().to_vec().into_boxed_slice();
        fade(&mut pixels, DRAG_IMAGE_ALPHA);
        let icon = Icon::new(rendered.width(), rendered.height(), pixels);
        let size = Point::new(icon.width() as i32, icon.height() as i32);
        let offset = group_offset(grab, origin, scale);
        Some(Self {
            hotspot: hotspot(offset, size),
            icon,
        })
    }

    pub fn width(&self) -> i32 {
        self.icon.width() as i32
    }

    pub fn height(&self) -> i32 {
        self.icon.height() as i32
    }
}

/// The bounding box of the selected items present in `scene`, in scene
/// coordinates.
fn selection_bounds(scene: &SpaceScene, ids: &[u64]) -> Option<Rect> {
    scene
        .items
        .iter()
        .filter(|item| ids.contains(&item.id))
        .fold(None, |bounds: Option<Rect>, item| {
            Some(match bounds {
                Some(current) => current.union(item.rect),
                None => item.rect,
            })
        })
}

/// How much a `bounds`-sized group is scaled down to fit `MAX_DRAG_IMAGE`.
fn group_scale(bounds: Rect) -> f32 {
    let longest = bounds.width.max(bounds.height).max(1) as f32;
    (MAX_DRAG_IMAGE / longest).min(1.0)
}

/// Where the pointer sits inside the scaled group image.
fn group_offset(grab: Point, bounds: Rect, scale: f32) -> Point {
    Point::new(
        ((grab.x - bounds.x) as f32 * scale).round() as i32,
        ((grab.y - bounds.y) as f32 * scale).round() as i32,
    )
}

/// The grab offset inside the image, kept within it.
fn hotspot(offset: Point, size: Point) -> Point {
    Point::new(
        offset.x.clamp(0, (size.x - 1).max(0)),
        offset.y.clamp(0, (size.y - 1).max(0)),
    )
}

/// Scales premultiplied BGRA pixels by `alpha` / 255, making them uniformly
/// more transparent.
fn fade(pixels: &mut [u8], alpha: u8) {
    let alpha = alpha as u32;
    for channel in pixels.iter_mut() {
        *channel = ((*channel as u32 * alpha + 127) / 255) as u8;
    }
}

/// Undoes premultiplication of BGRA pixels in place.
fn unpremultiply(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }
}

/// A new top-down 32 bpp DIB section holding `icon`, with premultiplied or
/// straight alpha. The caller owns the bitmap.
pub(crate) fn dib(icon: &Icon, premultiplied: bool) -> Option<HBITMAP> {
    let width = icon.width() as i32;
    let height = icon.height() as i32;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative height: rows run top-down like `Icon` pixels.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    // SAFETY: `info` describes a width x height 32 bpp DIB; CreateDIBSection
    // allocates exactly `icon.pixels().len()` bytes behind `bits`, which are
    // written once before the bitmap is handed out.
    unsafe {
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            return None;
        }
        let target = std::slice::from_raw_parts_mut(bits.cast::<u8>(), icon.pixels().len());
        target.copy_from_slice(icon.pixels());
        if !premultiplied {
            unpremultiply(target);
        }
        Some(bitmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspot_keeps_the_grab_offset_inside_the_image() {
        let size = Point::new(96, 104);
        assert_eq!(hotspot(Point::new(40, 20), size), Point::new(40, 20));
        assert_eq!(hotspot(Point::new(-3, 200), size), Point::new(0, 103));
    }

    #[test]
    fn a_small_group_is_not_scaled_and_a_large_one_fits_the_cap() {
        assert_eq!(group_scale(Rect::new(0, 0, 96, 104)), 1.0);
        let scale = group_scale(Rect::new(0, 0, 1024, 128));
        assert!(scale < 1.0);
        assert!((1024.0 * scale - MAX_DRAG_IMAGE).abs() < 1.0);
    }

    #[test]
    fn the_group_offset_tracks_the_scaled_grab_point() {
        let bounds = Rect::new(100, 200, 400, 200);
        // No scaling: the grab point is relative to the group's top-left.
        assert_eq!(
            group_offset(Point::new(150, 260), bounds, 1.0),
            Point::new(50, 60)
        );
        // Half scale: the offset shrinks with the image.
        assert_eq!(
            group_offset(Point::new(150, 260), bounds, 0.5),
            Point::new(25, 30)
        );
    }

    #[test]
    fn fade_scales_every_premultiplied_channel() {
        let mut pixels = [255, 128, 0, 255];
        fade(&mut pixels, 200);
        assert_eq!(pixels, [200, 100, 0, 200]);
    }

    #[test]
    fn unpremultiply_restores_straight_color() {
        let mut pixels = [64, 32, 0, 128, 10, 20, 30, 255, 9, 9, 9, 0];
        unpremultiply(&mut pixels);
        assert_eq!(pixels, [128, 64, 0, 128, 10, 20, 30, 255, 9, 9, 9, 0]);
    }
}
