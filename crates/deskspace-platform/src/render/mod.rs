pub mod d2d;
pub mod scene;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) use d2d::PaintFrame;
pub use d2d::{Painter, PreparedScene};
pub use scene::{ItemState, ItemVisual, SpaceScene, SpaceStyle};

use deskspace_core::geometry::Rect;

/// Icon size and top inset within an item cell, shared by the painter and the
/// app's check-box hit test.
pub const ITEM_ICON_SIZE: i32 = 32;
pub const ITEM_ICON_TOP: i32 = 10;
/// Explorer-style item check box, anchored at the icon's top-left corner.
pub const ITEM_CHECK_BOX_SIZE: i32 = 13;
pub const ITEM_CHECK_BOX_MARGIN: i32 = 1;
/// Slack around the drawn box that still counts as a click on it.
const ITEM_CHECK_BOX_SLACK: i32 = 2;

/// The check box's rectangle inside an item cell, in the same coordinate
/// system as the cell. Matches where the painter draws it.
pub fn item_check_box(cell: Rect) -> Rect {
    let icon_left = cell.x + (cell.width - ITEM_ICON_SIZE) / 2;
    Rect::new(
        icon_left + ITEM_CHECK_BOX_MARGIN,
        cell.y + ITEM_ICON_TOP + ITEM_CHECK_BOX_MARGIN,
        ITEM_CHECK_BOX_SIZE,
        ITEM_CHECK_BOX_SIZE,
    )
}

/// The region that counts as a click on the check box: the drawn box plus a
/// little slack, so a near miss still toggles it.
pub fn item_check_box_hit(cell: Rect) -> Rect {
    let drawn = item_check_box(cell);
    Rect::new(
        drawn.x - ITEM_CHECK_BOX_SLACK,
        drawn.y - ITEM_CHECK_BOX_SLACK,
        drawn.width + ITEM_CHECK_BOX_SLACK * 2,
        drawn.height + ITEM_CHECK_BOX_SLACK * 2,
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
        let checkbox = item_check_box(cell);
        assert_eq!(
            checkbox.x,
            100 + (96 - ITEM_ICON_SIZE) / 2 + ITEM_CHECK_BOX_MARGIN
        );
        assert_eq!(checkbox.y, 200 + ITEM_ICON_TOP + ITEM_CHECK_BOX_MARGIN);
        assert_eq!(checkbox.width, ITEM_CHECK_BOX_SIZE);
        assert_eq!(checkbox.height, ITEM_CHECK_BOX_SIZE);
    }
}
