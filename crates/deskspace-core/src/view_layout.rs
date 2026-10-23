//! Where a space's items sit, for its current view (W16, D15).
//!
//! Every reader of item geometry goes through this pure layout: scenes, hit
//! tests, the marquee, keyboard navigation, the drag drop index, and the scroll
//! extent. The icon views keep the grid they always had; a view that places
//! items by their order (Small icons now, List and the rest later) never writes
//! `Item.position`, so switching back to an icon view restores the hand
//! arrangement (D15).
//!
//! Layout is arithmetic: [`ViewLayout::rect`] is `O(1)`, and
//! [`ViewLayout::visible`] finds the items in a scroll window without walking
//! every item.

use std::ops::Range;

use crate::geometry::{Point, Rect, Size};
use crate::layout::{GridSpec, columns_for};
use crate::space::Space;
use crate::view::ViewMode;

/// Explorer's inline cell for Small icons at 96 DPI: a 16 px icon with one
/// label line beside it. The width is provisional until W16's on-screen check
/// measures Explorer's own.
const SMALL_ICON_CELL_WIDTH: i32 = 200;
const SMALL_ICON_CELL_HEIGHT: i32 = 22;
/// The space between inline cells, horizontally and vertically.
const SMALL_ICON_GAP: i32 = 2;

/// Which way a view's content flows when it does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Rows stack downward; the wheel and the scroll bar are vertical.
    Vertical,
    /// Columns run sideways; the wheel and the scroll bar are horizontal
    /// (List, W17).
    Horizontal,
}

/// The cell geometry a view places items in, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewMetrics {
    /// The cell each item occupies.
    pub cell: Size,
    /// The space between cells, on both axes.
    pub gap: i32,
    /// The edge padding inside the content area.
    pub padding: i32,
}

impl ViewMetrics {
    /// The metrics for `mode` on a display of `display_scale` (1.0 is 96 DPI).
    /// An icon view hands off to `grid` as it is; an order-placed view uses its
    /// own inline cell, scaled for the display rather than the look.
    pub fn for_view(mode: ViewMode, grid: GridSpec, display_scale: f32) -> Self {
        if mode.places_by_order() {
            let scale = display_scale.max(0.1);
            let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
            Self {
                cell: Size::new(
                    scaled(SMALL_ICON_CELL_WIDTH),
                    scaled(SMALL_ICON_CELL_HEIGHT),
                ),
                gap: scaled(SMALL_ICON_GAP),
                padding: grid.padding,
            }
        } else {
            Self {
                cell: grid.cell(),
                gap: grid.gap,
                padding: grid.padding,
            }
        }
    }

    /// The metrics of an icon grid, as the spaces had before views placed items
    /// by order.
    pub fn for_grid(grid: GridSpec) -> Self {
        Self {
            cell: grid.cell(),
            gap: grid.gap,
            padding: grid.padding,
        }
    }
}

/// The placement of `count` items in a space's content area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewLayout {
    mode: ViewMode,
    content: Size,
    count: usize,
    metrics: ViewMetrics,
}

impl ViewLayout {
    pub fn new(mode: ViewMode, content: Size, count: usize, metrics: ViewMetrics) -> Self {
        Self {
            mode,
            content,
            count,
            metrics,
        }
    }

    /// The layout of a space's items under `metrics`.
    pub fn for_space(space: &Space, metrics: ViewMetrics) -> Self {
        Self::new(
            space.view().mode,
            space.content_rect().size(),
            space.items().len(),
            metrics,
        )
    }

    /// The number of columns each row of cells holds (at least one).
    pub fn columns(&self) -> usize {
        columns_for(
            self.content.width,
            self.metrics.cell.width,
            self.metrics.gap,
            self.metrics.padding,
        ) as usize
    }

    /// The number of rows the items occupy.
    pub fn rows(&self) -> usize {
        self.count.div_ceil(self.columns())
    }

    /// The cell of the item at `index`, relative to the content area's origin.
    /// `index` must be below the item count.
    pub fn rect(&self, index: usize) -> Rect {
        debug_assert!(index < self.count, "item index out of the layout");
        let columns = self.columns();
        let column = (index % columns) as i32;
        let row = (index / columns) as i32;
        Rect::new(
            self.metrics.padding + column * (self.metrics.cell.width + self.metrics.gap),
            self.metrics.padding + row * (self.metrics.cell.height + self.metrics.gap),
            self.metrics.cell.width,
            self.metrics.cell.height,
        )
    }

    /// The content length along the scroll axis, padding included. Zero when
    /// the space has no items.
    pub fn extent(&self) -> i32 {
        if self.count == 0 {
            return 0;
        }
        let rows = self.rows() as i32;
        self.metrics.padding * 2 + rows * self.metrics.cell.height + (rows - 1) * self.metrics.gap
    }

    /// How far the content can scroll in a viewport `viewport` px long.
    pub fn max_scroll(&self, viewport: i32) -> i32 {
        (self.extent() - viewport).max(0)
    }

    /// Which way the content scrolls.
    pub fn axis(&self) -> ScrollAxis {
        match self.mode {
            // Every view W16 places stacks rows downward; List is the first
            // that runs sideways (W17).
            ViewMode::Icons
            | ViewMode::ExtraLargeIcons
            | ViewMode::LargeIcons
            | ViewMode::MediumIcons
            | ViewMode::SmallIcons => ScrollAxis::Vertical,
        }
    }

    /// The pixels one wheel notch scrolls: one row.
    pub fn scroll_step(&self) -> i32 {
        self.metrics.cell.height + self.metrics.gap
    }

    /// The item whose cell contains `point`, relative to the content area and
    /// scrolled into that coordinate space (the caller adds its scroll offset).
    pub fn index_at(&self, point: Point) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let step_x = (self.metrics.cell.width + self.metrics.gap).max(1);
        let step_y = (self.metrics.cell.height + self.metrics.gap).max(1);
        let x = point.x - self.metrics.padding;
        let y = point.y - self.metrics.padding;
        if x < 0 || y < 0 {
            return None;
        }
        let column = (x / step_x) as usize;
        let row = (y / step_y) as usize;
        if column >= self.columns() {
            return None;
        }
        let index = row * self.columns() + column;
        (index < self.count && self.rect(index).contains(point)).then_some(index)
    }

    /// The nearest slot for a drag dropped at `point` (relative to the content
    /// area and scrolled into that coordinate space), clamped to the last item.
    pub fn drop_index(&self, point: Point) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let step_x = (self.metrics.cell.width + self.metrics.gap).max(1);
        let step_y = (self.metrics.cell.height + self.metrics.gap).max(1);
        let columns = self.columns();
        let column = ((point.x - self.metrics.padding).max(0) / step_x)
            .clamp(0, columns.saturating_sub(1) as i32);
        let row = ((point.y - self.metrics.padding).max(0) / step_y).max(0);
        Some((row as usize * columns + column as usize).min(self.count - 1))
    }

    /// The indices whose rows can be inside a `viewport`-tall scroll window at
    /// `scroll`, in model order. A row may be partly visible, so callers still
    /// clip each rect to be exact.
    pub fn visible(&self, scroll: i32, viewport: i32) -> Range<usize> {
        if self.count == 0 || viewport <= 0 {
            return 0..0;
        }
        let step_y = (self.metrics.cell.height + self.metrics.gap).max(1);
        let rows = self.rows();
        let first_row = ((scroll - self.metrics.padding).max(0) / step_y) as usize;
        let last_row = ((scroll + viewport - self.metrics.padding).max(0) / step_y) as usize;
        if first_row >= rows {
            return self.count..self.count;
        }
        let last_row = last_row.min(rows - 1);
        let columns = self.columns();
        (first_row * columns).min(self.count)..((last_row + 1) * columns).min(self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> ViewMetrics {
        ViewMetrics {
            cell: Size::new(100, 50),
            gap: 10,
            padding: 12,
        }
    }

    fn layout_of(mode: ViewMode, width: i32, count: usize) -> ViewLayout {
        ViewLayout::new(mode, Size::new(width, 400), count, metrics())
    }

    #[test]
    fn cells_tile_in_order_without_overlapping() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 12);
        // 360 - 24 padding + 10 gap over a 110 step = 3 columns.
        assert_eq!(layout.columns(), 3);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        for index in 0..12 {
            let rect = layout.rect(index);
            assert_eq!(rect.width, 100);
            assert_eq!(rect.height, 50);
            for other in (index + 1)..12 {
                assert!(!rect.intersects(layout.rect(other)), "{index} and {other}");
            }
        }
        // Row-major: the fourth item starts the second row.
        assert_eq!(layout.rect(3).x, 12);
        assert_eq!(layout.rect(3).y, 12 + 60);
        assert_eq!(layout.rect(2).x, 12 + 2 * 110);
    }

    #[test]
    fn rows_wrap_at_the_content_width() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 7);
        assert_eq!(layout.columns(), 3);
        assert_eq!(layout.rows(), 3);
        assert_eq!(layout.rect(6).y, 12 + 2 * 60);
        // A narrow space holds one column.
        assert_eq!(layout_of(ViewMode::SmallIcons, 100, 4).columns(), 1);
    }

    #[test]
    fn the_extent_grows_by_rows_and_keeps_the_bottom_padding() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 7);
        // padding + 3 rows of 50 + 2 gaps of 10 + padding.
        assert_eq!(layout.extent(), 12 * 2 + 3 * 50 + 2 * 10);
        assert_eq!(layout.max_scroll(150), layout.extent() - 150);
        assert_eq!(layout.max_scroll(400), 0);
        assert_eq!(layout_of(ViewMode::SmallIcons, 360, 0).extent(), 0);
        assert_eq!(layout_of(ViewMode::SmallIcons, 360, 0).max_scroll(100), 0);
    }

    #[test]
    fn the_center_of_each_cell_hits_its_item() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 12);
        for index in 0..12 {
            assert_eq!(layout.index_at(layout.rect(index).center()), Some(index));
        }
        // The gap between two cells belongs to neither.
        let gap = layout.rect(1);
        assert_eq!(layout.index_at(Point::new(gap.x - 5, gap.center().y)), None);
        // Padding and the space past the last item are empty.
        assert_eq!(layout.index_at(Point::new(0, 0)), None);
        assert_eq!(layout.index_at(Point::new(1000, 1000)), None);
    }

    #[test]
    fn the_visible_range_holds_the_rows_a_scroll_window_touches() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 12);
        assert_eq!(layout.visible(0, 50), 0..3);
        assert_eq!(layout.visible(30, 10), 0..3);
        assert_eq!(layout.visible(60, 50), 0..6);
        assert_eq!(layout.visible(130, 50), 3..9);
        // Past the end, nothing is drawn.
        assert_eq!(layout.visible(1000, 50), 12..12);
        assert_eq!(layout.visible(0, 0), 0..0);
    }

    #[test]
    fn a_drop_between_cells_lands_on_the_nearest_slot() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 7);
        assert_eq!(layout.drop_index(Point::new(15, 15)), Some(0));
        assert_eq!(layout.drop_index(Point::new(200, 15)), Some(1));
        assert_eq!(layout.drop_index(Point::new(15, 80)), Some(3));
        // Past the last cell, the drop clamps to the last item.
        assert_eq!(layout.drop_index(Point::new(1000, 1000)), Some(6));
        assert_eq!(
            layout_of(ViewMode::SmallIcons, 360, 0).drop_index(Point::ZERO),
            None
        );
    }

    #[test]
    fn the_icon_views_keep_the_grid_positions() {
        let grid = GridSpec {
            cell_width: 96,
            cell_height: 104,
            gap: 10,
            padding: 12,
            columns: 4,
            ..GridSpec::default()
        };
        let metrics = ViewMetrics::for_grid(grid);
        let content = Size::new(grid.width_for_columns(grid.columns), 400);
        for count in 1..=12 {
            let layout = ViewLayout::new(ViewMode::Icons, content, count, metrics);
            assert_eq!(layout.columns(), grid.columns as usize);
            for index in 0..count {
                let rect = layout.rect(index);
                let position = grid.grid_position(index);
                assert_eq!((rect.x, rect.y), (position.x, position.y), "{index}");
            }
        }
    }

    #[test]
    fn an_icon_view_scrolls_down_and_steps_by_a_row() {
        let metrics = ViewMetrics::for_grid(GridSpec::default());
        let layout = ViewLayout::new(ViewMode::MediumIcons, Size::new(400, 400), 12, metrics);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        assert_eq!(layout.scroll_step(), metrics.cell.height + metrics.gap);
    }

    #[test]
    fn inline_metrics_scale_with_the_display_not_the_look() {
        let grid = GridSpec::default();
        let base = ViewMetrics::for_view(ViewMode::SmallIcons, grid, 1.0);
        let large = ViewMetrics::for_view(ViewMode::SmallIcons, grid, 1.5);
        assert_eq!(base.cell, Size::new(200, 22));
        assert_eq!(large.cell, Size::new(300, 33));
        assert_eq!(base.gap, 2);
        assert_eq!(large.gap, 3);
        assert_eq!(base.padding, grid.padding, "the look's edge padding");
        // An icon view ignores the display scale; its grid is already scaled.
        assert_eq!(
            ViewMetrics::for_view(ViewMode::Icons, grid, 1.5),
            ViewMetrics::for_grid(grid)
        );
        assert_eq!(crate::view::SMALL_ICON_PX, 16);
    }
}
