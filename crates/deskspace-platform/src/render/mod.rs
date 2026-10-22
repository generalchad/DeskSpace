pub mod burn;
pub mod d2d;
pub mod flames;
pub mod scene;
pub mod transition;
pub mod zoom_bar;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) use d2d::{BorderStroke, PaintFrame};
pub use d2d::{Painter, PreparedScene};
pub use scene::{DropInto, ItemState, ItemVisual, SpaceScene, SpaceStyle};
pub use transition::{Geometry, Mask, Play, Transition, TransitionFrame, Way};
pub use zoom_bar::ZoomBar;

use deskspace_core::geometry::Rect;

/// Icon size and top inset within an item cell, at the 96-DPI / 100 % base.
/// 48 px is Windows' default desktop icon size ("Medium icons" at 100 % DPI),
/// so the default icon size setting is 1:1 with Explorer's desktop.
pub const ITEM_ICON_SIZE: i32 = 48;
pub const ITEM_ICON_TOP: i32 = 8;
/// Gap between the icon and its label at the base size.
const ITEM_LABEL_GAP: i32 = 6;
/// Explorer-style item check box, anchored at the icon's top-left corner.
pub const ITEM_CHECK_BOX_SIZE: i32 = 13;
pub const ITEM_CHECK_BOX_MARGIN: i32 = 1;
/// Slack around the drawn box that still counts as a click on it.
const ITEM_CHECK_BOX_SLACK: i32 = 2;

/// Item sizes scaled for DPI and the icon-size setting. The painter and the
/// app's hit tests share these so they always agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMetrics {
    pub icon_size: i32,
    pub icon_top: i32,
    /// Gap between the icon and its label.
    pub label_gap: i32,
    pub check_box_size: i32,
    pub check_box_margin: i32,
}

impl ItemMetrics {
    /// The metrics for a display/icon factor (`1.0` is the 96-DPI base).
    pub fn for_scale(scale: f32) -> Self {
        let scale = scale.max(0.1);
        let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
        Self {
            icon_size: scaled(ITEM_ICON_SIZE),
            icon_top: scaled(ITEM_ICON_TOP),
            label_gap: scaled(ITEM_LABEL_GAP),
            check_box_size: scaled(ITEM_CHECK_BOX_SIZE),
            check_box_margin: scaled(ITEM_CHECK_BOX_MARGIN),
        }
    }
}

/// The check box's rectangle inside an item cell, in the same coordinate
/// system as the cell. Matches where the painter draws it.
pub fn item_check_box(cell: Rect, metrics: ItemMetrics) -> Rect {
    let icon_left = cell.x + (cell.width - metrics.icon_size) / 2;
    Rect::new(
        icon_left + metrics.check_box_margin,
        cell.y + metrics.icon_top + metrics.check_box_margin,
        metrics.check_box_size,
        metrics.check_box_size,
    )
}

/// The region that counts as a click on the check box: the drawn box plus a
/// little slack, so a near miss still toggles it.
pub fn item_check_box_hit(cell: Rect, metrics: ItemMetrics) -> Rect {
    let drawn = item_check_box(cell, metrics);
    let slack = (ITEM_CHECK_BOX_SLACK as f32
        * (metrics.check_box_size as f32 / ITEM_CHECK_BOX_SIZE as f32))
        .round() as i32;
    let slack = slack.max(ITEM_CHECK_BOX_SLACK);
    Rect::new(
        drawn.x - slack,
        drawn.y - slack,
        drawn.width + slack * 2,
        drawn.height + slack * 2,
    )
}

/// A decoded icon: premultiplied BGRA pixels, cheap to clone. The pixels are
/// owned here rather than borrowed from an `HICON`, so a scene stays valid
/// after `IconCache` evicts the icon it came from. Painters cache GPU bitmaps
/// by `id`, which is never reused within a process.
#[derive(Clone)]
pub struct Icon(Arc<IconImage>);

struct IconImage {
    id: u64,
    width: u32,
    height: u32,
    pixels: Box<[u8]>,
}

impl Icon {
    /// `pixels` holds `width * height` premultiplied BGRA pixels, top-down.
    pub(crate) fn new(width: u32, height: u32, pixels: Box<[u8]>) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self(Arc::new(IconImage {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels,
        }))
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
}

impl PartialEq for Icon {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Icon {}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icon")
            .field("id", &self.id())
            .field("width", &self.width())
            .field("height", &self.height())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_box_sits_at_the_icon_corner() {
        let cell = Rect::new(100, 200, 96, 104);
        let metrics = ItemMetrics::for_scale(1.0);
        let checkbox = item_check_box(cell, metrics);
        assert_eq!(
            checkbox.x,
            100 + (96 - ITEM_ICON_SIZE) / 2 + ITEM_CHECK_BOX_MARGIN
        );
        assert_eq!(checkbox.y, 200 + ITEM_ICON_TOP + ITEM_CHECK_BOX_MARGIN);
        assert_eq!(checkbox.width, ITEM_CHECK_BOX_SIZE);
        assert_eq!(checkbox.height, ITEM_CHECK_BOX_SIZE);
    }

    #[test]
    fn metrics_and_check_box_scale_together() {
        let cell = Rect::new(0, 0, 144, 156);
        let metrics = ItemMetrics::for_scale(1.5);
        assert_eq!(metrics.icon_size, ITEM_ICON_SIZE * 3 / 2);
        assert_eq!(metrics.check_box_size, 20);
        let checkbox = item_check_box(cell, metrics);
        assert_eq!(checkbox.width, metrics.check_box_size);
        // The hit region is always at least the drawn box.
        let hit = item_check_box_hit(cell, metrics);
        assert!(hit.width >= checkbox.width);
    }
}
