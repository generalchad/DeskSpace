//! Where a space's items sit, for its current view (W16, D15).
//!
//! Every reader of item geometry goes through this pure layout: scenes, hit
//! tests, the marquee, keyboard navigation, the drag drop index, and the scroll
//! extent. The icon views keep the grid they always had; a view that places
//! items by their order (Small icons, and List since W17) never writes
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
/// Explorer's List column at 96 DPI: the same inline cell, filled top to
/// bottom. The width is provisional until W17's on-screen check measures
/// Explorer's own.
const LIST_COLUMN_WIDTH: i32 = 200;
const LIST_CELL_HEIGHT: i32 = 22;
/// The space between inline cells, horizontally and vertically.
const INLINE_GAP: i32 = 2;
/// Explorer's Details row and header heights at 96 DPI. Provisional until
/// W18's on-screen check measures Explorer's own.
const DETAILS_ROW_HEIGHT: i32 = 22;
pub const DETAILS_HEADER_HEIGHT: i32 = 25;

/// Which way a view's content flows when it does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Rows stack downward; the wheel and the scroll bar are vertical.
    Vertical,
    /// Columns run sideways; the wheel and the scroll bar are horizontal
    /// (List, W17).
    Horizontal,
}

/// Which way cells fill before the layout moves along the scroll axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// Cells fill a row across, then the next row down (every view but List).
    Rows,
    /// Cells fill a column down, then the next column across (List, W17).
    Columns,
}

impl Flow {
    fn of(mode: ViewMode) -> Self {
        match mode {
            ViewMode::List => Self::Columns,
            ViewMode::Icons
            | ViewMode::ExtraLargeIcons
            | ViewMode::LargeIcons
            | ViewMode::MediumIcons
            | ViewMode::SmallIcons
            | ViewMode::Details => Self::Rows,
        }
    }

    fn axis(self) -> ScrollAxis {
        match self {
            Self::Rows => ScrollAxis::Vertical,
            Self::Columns => ScrollAxis::Horizontal,
        }
    }
}

/// The cell geometry a view places items in, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewMetrics {
    /// The cell each item occupies. Details fills the content width, so only
    /// its height is a cell side there.
    pub cell: Size,
    /// The space between cells, on both axes.
    pub gap: i32,
    /// The edge padding inside the content area.
    pub padding: i32,
    /// The fixed header row above the items; 0 for every view but Details
    /// (W18).
    pub header: i32,
}

impl ViewMetrics {
    /// The metrics for `mode` on a display of `display_scale` (1.0 is 96 DPI).
    /// An icon view hands off to `grid` as it is; an order-placed view uses its
    /// own inline cell, scaled for the display rather than the look.
    pub fn for_view(mode: ViewMode, grid: GridSpec, display_scale: f32) -> Self {
        let scale = display_scale.max(0.1);
        let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
        if mode == ViewMode::Details {
            // Rows touch, as Explorer's Details rows do; the width is the
            // content width, worked out by the layout.
            return Self {
                cell: Size::new(1, scaled(DETAILS_ROW_HEIGHT)),
                gap: 0,
                padding: grid.padding,
                header: scaled(DETAILS_HEADER_HEIGHT),
            };
        }
        if let Some((width, height)) = inline_cell(mode) {
            Self {
                cell: Size::new(scaled(width), scaled(height)),
                gap: scaled(INLINE_GAP),
                padding: grid.padding,
                header: 0,
            }
        } else {
            Self {
                cell: grid.cell(),
                gap: grid.gap,
                padding: grid.padding,
                header: 0,
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
            header: 0,
        }
    }
}

/// The order-placed views' inline cells at 96 DPI (`width`, `height`), or
/// `None` for an icon view, which uses the look's grid. Details ignores the
/// width: each row fills the content, and only its height is a cell side.
fn inline_cell(mode: ViewMode) -> Option<(i32, i32)> {
    match mode {
        ViewMode::SmallIcons => Some((SMALL_ICON_CELL_WIDTH, SMALL_ICON_CELL_HEIGHT)),
        ViewMode::List => Some((LIST_COLUMN_WIDTH, LIST_CELL_HEIGHT)),
        ViewMode::Details => Some((1, DETAILS_ROW_HEIGHT)),
        ViewMode::Icons
        | ViewMode::ExtraLargeIcons
        | ViewMode::LargeIcons
        | ViewMode::MediumIcons => None,
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

    fn flow(&self) -> Flow {
        Flow::of(self.mode)
    }

    /// The cell side across the flow: a row's width, or a column's height.
    /// Details fills the content width, so its row is as wide as the content
    /// inside the padding.
    fn cross_cell(&self) -> i32 {
        if self.mode == ViewMode::Details {
            return (self.cross_len() - self.metrics.padding * 2).max(1);
        }
        match self.flow() {
            Flow::Rows => self.metrics.cell.width,
            Flow::Columns => self.metrics.cell.height,
        }
    }

    /// The fixed header row's height above the items (Details only, W18).
    pub fn header_height(&self) -> i32 {
        if self.mode == ViewMode::Details {
            self.metrics.header
        } else {
            0
        }
    }

    /// The cell side along the scroll axis.
    fn line_cell(&self) -> i32 {
        match self.flow() {
            Flow::Rows => self.metrics.cell.height,
            Flow::Columns => self.metrics.cell.width,
        }
    }

    /// The content length across the flow.
    fn cross_len(&self) -> i32 {
        match self.flow() {
            Flow::Rows => self.content.width,
            Flow::Columns => self.content.height,
        }
    }

    fn cross_step(&self) -> i32 {
        (self.cross_cell() + self.metrics.gap).max(1)
    }

    fn line_step(&self) -> i32 {
        (self.line_cell() + self.metrics.gap).max(1)
    }

    /// How many cells fill one line across the flow (at least one).
    pub fn cross_count(&self) -> usize {
        columns_for(
            self.cross_len(),
            self.cross_cell(),
            self.metrics.gap,
            self.metrics.padding,
        ) as usize
    }

    /// How many lines the items occupy along the scroll axis.
    pub fn line_count(&self) -> usize {
        self.count.div_ceil(self.cross_count())
    }

    /// The cell of the item at `index`, relative to the content area's origin.
    /// `index` must be below the item count.
    pub fn rect(&self, index: usize) -> Rect {
        debug_assert!(index < self.count, "item index out of the layout");
        let cross_count = self.cross_count();
        let line = (index / cross_count) as i32;
        let cross = (index % cross_count) as i32;
        let (x, y) = match self.flow() {
            Flow::Rows => (cross * self.cross_step(), line * self.line_step()),
            Flow::Columns => (line * self.line_step(), cross * self.cross_step()),
        };
        Rect::new(
            self.metrics.padding + x,
            self.header_height() + self.metrics.padding + y,
            if self.mode == ViewMode::Details {
                self.cross_cell()
            } else {
                self.metrics.cell.width
            },
            self.metrics.cell.height,
        )
    }

    /// The cell of the item at `index` after scrolling `scroll` px along the
    /// axis, relative to the content area's origin.
    pub fn scrolled(&self, index: usize, scroll: i32) -> Rect {
        let rect = self.rect(index);
        match self.flow() {
            Flow::Rows => Rect::new(rect.x, rect.y - scroll, rect.width, rect.height),
            Flow::Columns => Rect::new(rect.x - scroll, rect.y, rect.width, rect.height),
        }
    }

    /// A point relative to the content area, scrolled into layout coordinates
    /// (the inverse of `scrolled`'s offset).
    pub fn layout_point(&self, point: Point, scroll: i32) -> Point {
        match self.flow() {
            Flow::Rows => Point::new(point.x, point.y + scroll),
            Flow::Columns => Point::new(point.x + scroll, point.y),
        }
    }

    /// The content length along the scroll axis, which is the viewport when
    /// the whole content area is shown.
    pub fn viewport(&self, content: Size) -> i32 {
        match self.flow() {
            Flow::Rows => content.height,
            Flow::Columns => content.width,
        }
    }

    /// The content length along the scroll axis, padding included. Zero when
    /// the space has no items.
    pub fn extent(&self) -> i32 {
        if self.count == 0 {
            return 0;
        }
        let lines = self.line_count() as i32;
        self.header_height()
            + self.metrics.padding * 2
            + lines * self.line_cell()
            + (lines - 1) * self.metrics.gap
    }

    /// How far the content can scroll in a content area `content` long.
    pub fn max_scroll(&self, content: Size) -> i32 {
        (self.extent() - self.viewport(content)).max(0)
    }

    /// Which way the content scrolls.
    pub fn axis(&self) -> ScrollAxis {
        self.flow().axis()
    }

    /// The pixels one wheel notch scrolls: one row or one column.
    pub fn scroll_step(&self) -> i32 {
        self.line_step()
    }

    /// The item whose cell contains `point`, relative to the content area and
    /// scrolled into that coordinate space (the caller adds its scroll offset).
    pub fn index_at(&self, point: Point) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let (line, cross) = match self.flow() {
            Flow::Rows => (point.y, point.x),
            Flow::Columns => (point.x, point.y),
        };
        let line = line - self.header_height() - self.metrics.padding;
        let cross = cross - self.metrics.padding;
        if line < 0 || cross < 0 {
            return None;
        }
        let line = (line / self.line_step()) as usize;
        let cross = (cross / self.cross_step()) as usize;
        if cross >= self.cross_count() {
            return None;
        }
        let index = line * self.cross_count() + cross;
        (index < self.count && self.rect(index).contains(point)).then_some(index)
    }

    /// The nearest slot for a drag dropped at `point` (relative to the content
    /// area and scrolled into that coordinate space), clamped to the last item.
    pub fn drop_index(&self, point: Point) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let (line, cross) = match self.flow() {
            Flow::Rows => (point.y, point.x),
            Flow::Columns => (point.x, point.y),
        };
        let cross_count = self.cross_count();
        let cross = ((cross - self.metrics.padding).max(0) / self.cross_step())
            .clamp(0, cross_count.saturating_sub(1) as i32);
        let line =
            ((line - self.header_height() - self.metrics.padding).max(0) / self.line_step()).max(0);
        Some((line as usize * cross_count + cross as usize).min(self.count - 1))
    }

    /// The indices whose lines can be inside a scroll window at `scroll` in a
    /// content area `content`, in model order. A line may be partly visible,
    /// so callers still clip each rect to be exact.
    pub fn visible(&self, scroll: i32, content: Size) -> Range<usize> {
        let viewport = self.viewport(content);
        if self.count == 0 || viewport <= 0 {
            return 0..0;
        }
        let lines = self.line_count();
        let header = self.header_height();
        let first_line =
            ((scroll - header - self.metrics.padding).max(0) / self.line_step()) as usize;
        let last_line = ((scroll + viewport - header - self.metrics.padding).max(0)
            / self.line_step()) as usize;
        if first_line >= lines {
            return self.count..self.count;
        }
        let last_line = last_line.min(lines - 1);
        let cross_count = self.cross_count();
        (first_line * cross_count).min(self.count)..((last_line + 1) * cross_count).min(self.count)
    }

    /// The scroll offset that brings the item at `index` wholly into view in a
    /// content area `content`, or `None` when it already is.
    pub fn scroll_to_show(&self, index: usize, scroll: i32, content: Size) -> Option<i32> {
        let rect = self.rect(index);
        let (start, end) = match self.flow() {
            Flow::Rows => (rect.y, rect.bottom()),
            Flow::Columns => (rect.x, rect.right()),
        };
        let viewport = self.viewport(content);
        let offset = if start - scroll < 0 {
            start
        } else if end - scroll > viewport {
            end - viewport
        } else {
            return None;
        };
        Some(offset.clamp(0, self.max_scroll(content)))
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
            header: 0,
        }
    }

    fn layout_of(mode: ViewMode, width: i32, count: usize) -> ViewLayout {
        ViewLayout::new(mode, Size::new(width, 400), count, metrics())
    }

    #[test]
    fn cells_tile_in_order_without_overlapping() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 12);
        // 360 - 24 padding + 10 gap over a 110 step = 3 columns.
        assert_eq!(layout.cross_count(), 3);
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
        assert_eq!(layout.cross_count(), 3);
        assert_eq!(layout.line_count(), 3);
        assert_eq!(layout.rect(6).y, 12 + 2 * 60);
        // A narrow space holds one column.
        assert_eq!(layout_of(ViewMode::SmallIcons, 100, 4).cross_count(), 1);
    }

    #[test]
    fn the_extent_grows_by_rows_and_keeps_the_bottom_padding() {
        let layout = layout_of(ViewMode::SmallIcons, 360, 7);
        // padding + 3 rows of 50 + 2 gaps of 10 + padding.
        assert_eq!(layout.extent(), 12 * 2 + 3 * 50 + 2 * 10);
        assert_eq!(
            layout.max_scroll(Size::new(360, 150)),
            layout.extent() - 150
        );
        assert_eq!(layout.max_scroll(Size::new(360, 400)), 0);
        assert_eq!(layout_of(ViewMode::SmallIcons, 360, 0).extent(), 0);
        assert_eq!(
            layout_of(ViewMode::SmallIcons, 360, 0).max_scroll(Size::new(360, 100)),
            0
        );
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
        assert_eq!(layout.visible(0, Size::new(360, 50)), 0..3);
        assert_eq!(layout.visible(30, Size::new(360, 10)), 0..3);
        assert_eq!(layout.visible(60, Size::new(360, 50)), 0..6);
        assert_eq!(layout.visible(130, Size::new(360, 50)), 3..9);
        // Past the end, nothing is drawn.
        assert_eq!(layout.visible(1000, Size::new(360, 50)), 12..12);
        assert_eq!(layout.visible(0, Size::new(360, 0)), 0..0);
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
            assert_eq!(layout.cross_count(), grid.columns as usize);
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
        // List is an inline cell too.
        assert_eq!(
            ViewMetrics::for_view(ViewMode::List, grid, 1.0).cell,
            Size::new(200, 22)
        );
        // An icon view ignores the display scale; its grid is already scaled.
        assert_eq!(
            ViewMetrics::for_view(ViewMode::Icons, grid, 1.5),
            ViewMetrics::for_grid(grid)
        );
        assert_eq!(crate::view::SMALL_ICON_PX, 16);
    }

    #[test]
    fn details_rows_run_full_width_under_a_fixed_header() {
        let grid = GridSpec::default();
        let padding = grid.padding;
        let metrics = ViewMetrics::for_view(ViewMode::Details, grid, 1.0);
        let layout = ViewLayout::new(ViewMode::Details, Size::new(360, 200), 5, metrics);
        assert_eq!(layout.header_height(), DETAILS_HEADER_HEIGHT);
        assert_eq!(layout.cross_count(), 1);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        assert_eq!(layout.scroll_step(), DETAILS_ROW_HEIGHT);
        let row = layout.rect(0);
        assert_eq!(
            row,
            Rect::new(
                padding,
                DETAILS_HEADER_HEIGHT + padding,
                360 - padding * 2,
                DETAILS_ROW_HEIGHT
            )
        );
        assert_eq!(layout.rect(1).y, row.y + DETAILS_ROW_HEIGHT);
        assert_eq!(
            layout.extent(),
            DETAILS_HEADER_HEIGHT + padding * 2 + 5 * DETAILS_ROW_HEIGHT
        );
        // The header row itself holds no item.
        assert_eq!(layout.index_at(Point::new(row.center().x, 5)), None);
        assert_eq!(layout.index_at(layout.rect(2).center()), Some(2));
    }

    #[test]
    fn details_scale_their_rows_and_header_with_the_display() {
        let base = ViewMetrics::for_view(ViewMode::Details, GridSpec::default(), 1.0);
        let large = ViewMetrics::for_view(ViewMode::Details, GridSpec::default(), 1.5);
        assert_eq!((base.header, base.cell.height), (25, 22));
        assert_eq!((large.header, large.cell.height), (38, 33));
    }

    #[test]
    fn details_scroll_below_the_header_and_still_show_the_first_rows() {
        let metrics = ViewMetrics::for_view(ViewMode::Details, GridSpec::default(), 1.0);
        let layout = ViewLayout::new(ViewMode::Details, Size::new(360, 200), 20, metrics);
        assert_eq!(
            layout.visible(0, Size::new(360, 100)),
            0..3,
            "rows from under the header"
        );
        let scrolled = layout.visible(100, Size::new(360, 100));
        assert!(scrolled.start > 0);
        assert_eq!(scrolled.start, 2);
        assert!(scrolled.end > scrolled.start);
    }

    #[test]
    fn list_fills_columns_top_to_bottom_and_runs_sideways() {
        // 160 px gives 2 rows of 50 + 10 gap in a 24 px padding, so 7 items
        // fill 4 columns.
        let layout = ViewLayout::new(ViewMode::List, Size::new(360, 160), 7, metrics());
        assert_eq!(layout.axis(), ScrollAxis::Horizontal);
        assert_eq!(layout.cross_count(), 2);
        assert_eq!(layout.line_count(), 4);
        assert_eq!(layout.rect(0), Rect::new(12, 12, 100, 50));
        assert_eq!(layout.rect(1), Rect::new(12, 72, 100, 50));
        assert_eq!(layout.rect(2), Rect::new(122, 12, 100, 50));
        assert_eq!(layout.rect(6), Rect::new(342, 12, 100, 50));
        // 4 columns of 100 + 3 gaps of 10, plus the padding on both sides.
        assert_eq!(layout.extent(), 12 * 2 + 4 * 100 + 3 * 10);
        assert_eq!(layout.scroll_step(), 110, "one column per wheel notch");
    }

    #[test]
    fn list_hit_tests_and_drops_follow_its_columns() {
        let layout = ViewLayout::new(ViewMode::List, Size::new(360, 160), 7, metrics());
        for index in 0..7 {
            assert_eq!(layout.index_at(layout.rect(index).center()), Some(index));
        }
        assert_eq!(layout.index_at(Point::new(15, 80)), Some(1));
        assert_eq!(layout.index_at(Point::new(240, 80)), Some(5));
        assert_eq!(layout.drop_index(Point::new(15, 80)), Some(1));
        assert_eq!(layout.drop_index(Point::new(240, 80)), Some(5));
        // Past the last column, a drop clamps to the last item.
        assert_eq!(layout.drop_index(Point::new(1000, 1000)), Some(6));
    }

    #[test]
    fn list_visible_and_scroll_use_the_horizontal_axis() {
        let layout = ViewLayout::new(ViewMode::List, Size::new(200, 160), 7, metrics());
        assert_eq!(layout.viewport(Size::new(200, 160)), 200);
        assert_eq!(
            layout.max_scroll(Size::new(200, 160)),
            layout.extent() - 200
        );
        assert_eq!(layout.visible(0, Size::new(200, 160)), 0..4);
        assert_eq!(layout.visible(130, Size::new(200, 160)), 2..6);
        assert_eq!(layout.visible(1000, Size::new(200, 160)), 7..7);
    }

    #[test]
    fn scrolling_shifts_the_layout_along_its_axis() {
        let list = ViewLayout::new(ViewMode::List, Size::new(200, 160), 7, metrics());
        assert_eq!(list.scrolled(2, 110), Rect::new(12, 12, 100, 50));
        assert_eq!(
            list.layout_point(Point::new(10, 40), 110),
            Point::new(120, 40)
        );

        let small = layout_of(ViewMode::SmallIcons, 360, 7);
        assert_eq!(small.scrolled(3, 116), Rect::new(12, -44, 100, 50));
        assert_eq!(
            small.layout_point(Point::new(10, 40), 116),
            Point::new(10, 156)
        );
    }

    #[test]
    fn scroll_to_show_brings_a_cell_inside_the_viewport() {
        let list = ViewLayout::new(ViewMode::List, Size::new(200, 160), 7, metrics());
        assert_eq!(list.scroll_to_show(0, 0, Size::new(200, 160)), None);
        assert_eq!(
            list.scroll_to_show(4, 0, Size::new(200, 160)),
            Some(132),
            "a column past the right edge"
        );
        assert_eq!(list.scroll_to_show(4, 132, Size::new(200, 160)), None);
        assert_eq!(
            list.scroll_to_show(0, 400, Size::new(200, 160)),
            Some(12),
            "a cell scrolled past the left edge moves back to its start"
        );

        let small = layout_of(ViewMode::SmallIcons, 360, 7);
        assert_eq!(
            small.scroll_to_show(6, 0, Size::new(360, 100)),
            Some(82),
            "a row below the bottom edge"
        );
    }
}
