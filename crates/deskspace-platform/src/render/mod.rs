pub mod burn;
pub(crate) mod color_picker;
pub mod d2d;
pub mod dissolve;
pub mod flames;
pub(crate) mod glyph;
pub(crate) mod highlight;
pub(crate) mod math;
pub mod scene;
pub mod shadow;
pub mod transition;
pub mod zoom_bar;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) use d2d::{BorderStroke, PaintFrame};
pub use d2d::{Painter, PreparedScene};
pub use scene::{
    ColumnAlign, DetailsColumnVisual, DetailsView, DropInto, GroupHeaderVisual, ItemLayout,
    ItemState, ItemVisual, ShadowStyle, SpaceScene, SpaceStyle, StatusBar, StatusButtonVisual,
    TitleIcon,
};
pub use transition::{Content, Geometry, Mask, Play, Side, Transition, TransitionFrame, Way};
pub use zoom_bar::ZoomBar;

use deskspace_core::geometry::Rect;
use deskspace_core::theme::LABEL_SIZE_RANGE;

/// Icon size within an item cell, at the 96-DPI / 100 % base. 48 px is
/// Windows' default desktop icon size ("Medium icons" at 100 % DPI), so the
/// default icon size setting is 1:1 with Explorer's desktop.
pub const ITEM_ICON_SIZE: i32 = 48;
/// An icon cell's highlight around its icon and label, at the base size, as
/// measured on Explorer's desktop on Windows 11 at 100 % with 40 px icons:
/// the highlight starts 2 px above the icon, Explorer's 15 px label line
/// starts 3 px below the icon, and the highlight ends 2 px below that line.
/// A label line here is 16 px, with its baseline a pixel lower in the line,
/// so a 2 px gap puts the text on Explorer's rows and 2 px below the line
/// ends the highlight on Explorer's.
pub const ITEM_ICON_TOP: i32 = 2;
const ITEM_LABEL_GAP: i32 = 2;
const ITEM_LABEL_BOTTOM: i32 = 2;
/// One label line at 100 % label size, in pixels; the label font does not
/// grow with the display scale, so neither does this.
pub(crate) const ITEM_LABEL_LINE_HEIGHT: f32 = 16.0;
/// How far short of the next icon cell, to the right and below, a highlight
/// stops: Explorer's is 74 px wide on its 75 px step.
pub(crate) const ITEM_FRAME_GAP: i32 = 1;
/// How far the highlight and focus rectangle of a row or tile (every view
/// but the icon grids) sit inside its cell.
pub(crate) const ITEM_FRAME_INSET: i32 = 2;
/// Explorer-style item check box, anchored at the icon's top-left corner.
pub const ITEM_CHECK_BOX_SIZE: i32 = 13;
pub const ITEM_CHECK_BOX_MARGIN: i32 = 1;
/// Slack around the drawn box that still counts as a click on it.
const ITEM_CHECK_BOX_SLACK: i32 = 2;
/// Inline cell (Small icons, and List later) at the base size: the inset from
/// the cell's left edge and the gap after the icon. The check box sits in the
/// inset, the icon after it, and the label after the icon.
const INLINE_INSET: i32 = 4;
const INLINE_GAP: i32 = 4;

/// Item sizes scaled for DPI and the icon-size setting. The painter and the
/// app's hit tests share these so they always agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMetrics {
    pub icon_size: i32,
    /// From an icon cell's top, where its highlight starts, to the icon.
    pub icon_top: i32,
    /// Gap between the icon and its label.
    pub label_gap: i32,
    /// From the label's last line to the highlight's bottom edge.
    pub label_bottom: i32,
    pub check_box_size: i32,
    pub check_box_margin: i32,
    /// The inline cell's left inset and the gap after its icon.
    pub inline_inset: i32,
    pub inline_gap: i32,
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
            label_bottom: scaled(ITEM_LABEL_BOTTOM),
            check_box_size: scaled(ITEM_CHECK_BOX_SIZE),
            check_box_margin: scaled(ITEM_CHECK_BOX_MARGIN),
            inline_inset: scaled(INLINE_INSET),
            inline_gap: scaled(INLINE_GAP),
        }
    }

    /// How tall an icon cell's highlight is around a label of `label_lines`
    /// lines (one or two) at `label_size` percent (`LABEL_SIZE_RANGE`): the
    /// icon's inset, the icon, the label gap, the lines, and the space below
    /// them. It hugs the item, as Explorer's does, so a one-line name gets a
    /// shorter highlight than a two-line one.
    pub fn grid_frame_height(&self, label_lines: u8, label_size: u8) -> i32 {
        let size = label_size.clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as f32 / 100.0;
        let lines = label_lines.clamp(1, 2) as f32;
        let label = (lines * ITEM_LABEL_LINE_HEIGHT * size).ceil() as i32;
        self.icon_top + self.icon_size + self.label_gap + label + self.label_bottom
    }

    /// The height of a grid cell: the tallest highlight an item can have
    /// with `label_lines` lines, and the gap that keeps it off the row below.
    pub fn grid_cell_height(&self, label_lines: u8, label_size: u8) -> i32 {
        self.grid_frame_height(label_lines, label_size) + ITEM_FRAME_GAP
    }

    /// Where an icon cell `width` wide puts its icon's left edge, from the
    /// cell's: centered in the highlight, on a whole pixel.
    pub fn grid_icon_left(&self, width: i32) -> i32 {
        (width - ITEM_FRAME_GAP - self.icon_size).div_euclid(2)
    }
}

/// The check box's rectangle inside an item cell, in the same coordinate
/// system as the cell. Matches where the painter draws it.
pub fn item_check_box(cell: Rect, metrics: ItemMetrics, layout: ItemLayout) -> Rect {
    let (left, top) = match layout {
        ItemLayout::Grid => {
            let icon_left = cell.x + metrics.grid_icon_left(cell.width);
            (
                icon_left + metrics.check_box_margin,
                cell.y + metrics.icon_top + metrics.check_box_margin,
            )
        }
        ItemLayout::Inline | ItemLayout::Details | ItemLayout::Tiles | ItemLayout::Content => (
            cell.x + metrics.inline_inset + metrics.check_box_margin,
            cell.y + (cell.height - metrics.check_box_size) / 2,
        ),
    };
    Rect::new(left, top, metrics.check_box_size, metrics.check_box_size)
}

/// The region that counts as a click on the check box: the drawn box plus a
/// little slack, so a near miss still toggles it.
pub fn item_check_box_hit(cell: Rect, metrics: ItemMetrics, layout: ItemLayout) -> Rect {
    let drawn = item_check_box(cell, metrics, layout);
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
        // Process state: the next icon id. Every `IconImage` needs a unique id
        // without an owner to hold the counter, and it only ever increments.
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
        let checkbox = item_check_box(cell, metrics, ItemLayout::Grid);
        // The icon is centered in the highlight, which stops a pixel short
        // of the cell's right edge.
        assert_eq!(
            checkbox.x,
            100 + (95 - ITEM_ICON_SIZE) / 2 + ITEM_CHECK_BOX_MARGIN
        );
        assert_eq!(checkbox.y, 200 + ITEM_ICON_TOP + ITEM_CHECK_BOX_MARGIN);
        assert_eq!(checkbox.width, ITEM_CHECK_BOX_SIZE);
        assert_eq!(checkbox.height, ITEM_CHECK_BOX_SIZE);
    }

    #[test]
    fn an_inline_check_box_sits_at_the_left_edge() {
        let cell = Rect::new(100, 200, 200, 22);
        let metrics = ItemMetrics::for_scale(1.0);
        let checkbox = item_check_box(cell, metrics, ItemLayout::Inline);
        assert_eq!(
            checkbox.x,
            100 + metrics.inline_inset + ITEM_CHECK_BOX_MARGIN
        );
        assert_eq!(checkbox.y, 200 + (22 - ITEM_CHECK_BOX_SIZE) / 2);
        assert_eq!(checkbox.width, ITEM_CHECK_BOX_SIZE);
        let hit = item_check_box_hit(cell, metrics, ItemLayout::Inline);
        assert_eq!(hit.center(), checkbox.center());
    }

    #[test]
    fn metrics_and_check_box_scale_together() {
        let cell = Rect::new(0, 0, 144, 156);
        let metrics = ItemMetrics::for_scale(1.5);
        assert_eq!(metrics.icon_size, ITEM_ICON_SIZE * 3 / 2);
        assert_eq!(metrics.check_box_size, 20);
        let checkbox = item_check_box(cell, metrics, ItemLayout::Grid);
        assert_eq!(checkbox.width, metrics.check_box_size);
        // The hit region is always at least the drawn box.
        let hit = item_check_box_hit(cell, metrics, ItemLayout::Grid);
        assert!(hit.width >= checkbox.width);
    }

    #[test]
    fn a_one_line_highlight_is_as_tall_as_explorers() {
        // Measured on Explorer's desktop at 100 % with a 40 px icon: the
        // highlight of a one-line name is 62 px tall.
        let metrics = ItemMetrics::for_scale(40.0 / ITEM_ICON_SIZE as f32);
        assert_eq!(metrics.icon_size, 40);
        assert_eq!(metrics.grid_frame_height(1, 100), 62);
        assert_eq!(metrics.grid_frame_height(2, 100), 62 + 16);
        assert_eq!(metrics.grid_frame_height(9, 100), 62 + 16, "two at most");
    }

    #[test]
    fn a_grid_cell_holds_its_tallest_highlight_and_the_gap_below_it() {
        let metrics = ItemMetrics::for_scale(1.0);
        for (lines, size) in [(1, 100), (2, 100), (2, 150)] {
            assert_eq!(
                metrics.grid_cell_height(lines, size),
                metrics.grid_frame_height(lines, size) + ITEM_FRAME_GAP
            );
        }
        assert!(metrics.grid_cell_height(1, 100) < metrics.grid_cell_height(2, 100));
        assert!(metrics.grid_cell_height(2, 100) < metrics.grid_cell_height(2, 150));
    }

    #[test]
    fn the_icon_is_centered_in_the_highlight() {
        let metrics = ItemMetrics::for_scale(1.0);
        // A 75 px cell has a 74 px highlight, 13 px each side of the icon.
        assert_eq!(metrics.grid_icon_left(75), 13);
        assert_eq!(metrics.grid_icon_left(76), 13);
        // A cell narrower than its icon still centers it.
        assert_eq!(metrics.grid_icon_left(41), -4);
    }
}
