//! Where a space's items sit, for its current view (W16, D17).
//!
//! Every reader of item geometry goes through this pure layout: scenes, hit
//! tests, the marquee, keyboard navigation, the drag drop index, and the scroll
//! extent. The icon views keep the grid they always had; a view that places
//! items by their order (Small icons, and List since W17) never writes
//! `Item.position`, so switching back to an icon view restores the hand
//! arrangement (D17).
//!
//! Layout is arithmetic: [`ViewLayout::rect`] is `O(1)` (`O(log groups)` in a
//! grouped space), and
//! [`ViewLayout::visible`] finds the items in a scroll window without walking
//! every item.

use std::ops::Range;
use std::sync::Arc;

use crate::geometry::{Point, Rect, Size};
use crate::grouping::Grouping;
use crate::layout::{GridSpec, columns_for, fit_length};
use crate::space::{ResizeEdges, Space};
use crate::view::{SpaceView, ViewMode};

/// The cells of the views that place items by order, at 96 DPI. Provisional
/// until their on-screen checks measure Explorer's own (W16-W22, W33).
struct OrderedCells {
    /// Small icons: a 16 px icon with one label line beside it.
    small_icons: (i32, i32),
    /// List: the same inline cell, filled top to bottom.
    list: (i32, i32),
    details_row: i32,
    details_header: i32,
    /// A group's header row, in every view.
    group_header: i32,
    tile: (i32, i32),
    content_row: i32,
    /// The space between inline cells, horizontally and vertically.
    gap: i32,
}

const NORMAL_CELLS: OrderedCells = OrderedCells {
    small_icons: (200, 22),
    list: (200, 22),
    details_row: 22,
    details_header: 25,
    group_header: 30,
    tile: (210, 60),
    content_row: 32,
    gap: 2,
};

/// Explorer's Compact view tightens only these views; the icon views keep the
/// Layout page's spacing, so there is one spacing control per view (D18).
const COMPACT_CELLS: OrderedCells = OrderedCells {
    small_icons: (200, 18),
    list: (200, 18),
    details_row: 18,
    details_header: 22,
    group_header: 24,
    tile: (210, 52),
    content_row: 26,
    gap: 0,
};

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
            | ViewMode::Details
            | ViewMode::Tiles
            | ViewMode::Content => Self::Rows,
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
    /// A group's header row, when the space groups its items.
    pub group_header: i32,
}

impl ViewMetrics {
    /// The metrics for `view` on a display of `display_scale` (1.0 is 96 DPI).
    /// An icon view hands off to `grid` as it is; an order-placed view uses its
    /// own cell, scaled for the display rather than the look.
    pub fn for_view(view: &SpaceView, grid: GridSpec, display_scale: f32) -> Self {
        let scale = display_scale.max(0.1);
        let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
        let cells = if view.compact {
            &COMPACT_CELLS
        } else {
            &NORMAL_CELLS
        };
        let (cell, gap, header) = match view.mode {
            ViewMode::Icons
            | ViewMode::ExtraLargeIcons
            | ViewMode::LargeIcons
            | ViewMode::MediumIcons => {
                return Self {
                    group_header: scaled(NORMAL_CELLS.group_header),
                    ..Self::for_grid(grid)
                };
            }
            ViewMode::SmallIcons => (cells.small_icons, cells.gap, 0),
            ViewMode::List => (cells.list, cells.gap, 0),
            ViewMode::Tiles => (cells.tile, cells.gap, 0),
            // Rows touch, as Explorer's do; the width is the content width,
            // worked out by the layout.
            ViewMode::Details => ((1, cells.details_row), 0, cells.details_header),
            ViewMode::Content => ((1, cells.content_row), 0, 0),
        };
        Self {
            cell: Size::new(scaled(cell.0), scaled(cell.1)),
            gap: if gap == 0 { 0 } else { scaled(gap) },
            padding: grid.padding,
            header: if header == 0 { 0 } else { scaled(header) },
            group_header: scaled(cells.group_header),
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
            group_header: NORMAL_CELLS.group_header,
        }
    }

    /// `width` rounded down to whole cell columns with equal edge padding.
    pub fn fit_width(&self, width: i32) -> i32 {
        fit_length(width, self.cell.width, self.gap, self.padding)
    }

    /// `height` rounded down to whole cell rows below `chrome`, the height
    /// that is not items: a title bar, a status bar, and any header.
    pub fn fit_height(&self, height: i32, chrome: i32) -> i32 {
        chrome + fit_length(height - chrome, self.cell.height, self.gap, self.padding)
    }

    /// `rect` rounded to whole cells along the dragged `edges`, keeping the
    /// opposite edge fixed. `fit_columns` is false for a view whose rows fill
    /// the width, so only its height fits (D22).
    pub fn fit_rect(self, rect: Rect, edges: ResizeEdges, chrome: i32, fit_columns: bool) -> Rect {
        let mut rect = rect;
        if fit_columns && (edges.left || edges.right) {
            let width = self.fit_width(rect.width);
            rect = if edges.left {
                Rect::new(rect.right() - width, rect.y, width, rect.height)
            } else {
                Rect::new(rect.x, rect.y, width, rect.height)
            };
        }
        if edges.top || edges.bottom {
            let height = self.fit_height(rect.height, chrome);
            rect = if edges.top {
                Rect::new(rect.x, rect.bottom() - height, rect.width, height)
            } else {
                Rect::new(rect.x, rect.y, rect.width, height)
            };
        }
        rect
    }
}

/// The size that shows all of `space`'s items in a view that places them by
/// order: rows keep the width and take the height they need, and List's columns
/// keep the height and take the width. An empty space keeps room for one line.
/// It reads the space's rect rather than its content area, which a fold empties.
pub fn fit_size(space: &Space, metrics: ViewMetrics) -> Size {
    let rect = space.rect();
    let chrome = space.title_height + space.unfolded_status_bar_height();
    let content = Size::new(rect.width, (rect.height - chrome).max(0));
    let mode = space.view().mode;
    let layout = ViewLayout::new(mode, content, space.items().len().max(1), metrics);
    match layout.axis() {
        ScrollAxis::Vertical => Size::new(rect.width, chrome + layout.extent()),
        ScrollAxis::Horizontal => Size::new(layout.extent(), rect.height),
    }
}

/// The placement of `count` items in a space's content area.
///
/// Indices are the items' stored indices, in a grouped space as in any other;
/// only this type knows the display order that puts them in groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewLayout {
    mode: ViewMode,
    content: Size,
    /// How many items are shown; in a grouped space, those not folded.
    count: usize,
    metrics: ViewMetrics,
    groups: Option<GroupedLines>,
}

/// A grouped layout's arithmetic: where each group begins along the scroll axis,
/// below the column header and padding, so a lookup is a binary search.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupedLines {
    grouping: Arc<Grouping>,
    /// One per group, then the end of the last.
    tops: Vec<i32>,
}

impl ViewLayout {
    pub fn new(mode: ViewMode, content: Size, count: usize, metrics: ViewMetrics) -> Self {
        Self {
            mode,
            content,
            count,
            metrics,
            groups: None,
        }
    }

    /// This layout with its items in `grouping`'s order, under its headers.
    /// Groups stack downward in every view, as Explorer's do, so List wraps
    /// into rows while it is grouped. `None` leaves the layout as it is.
    pub fn with_grouping(mut self, grouping: Option<Arc<Grouping>>) -> Self {
        let Some(grouping) = grouping else {
            return self;
        };
        self.count = grouping.shown();
        self.groups = Some(GroupedLines {
            tops: Vec::new(),
            grouping: Arc::clone(&grouping),
        });
        let cross_count = self.cross_count();
        let line_step = self.line_step();
        let mut tops = Vec::with_capacity(grouping.groups().len() + 1);
        let mut top = 0;
        tops.push(top);
        for group in grouping.groups() {
            let lines = group.shown().div_ceil(cross_count) as i32;
            top += self.metrics.group_header + lines * line_step;
            tops.push(top);
        }
        self.groups = Some(GroupedLines { tops, grouping });
        self
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
        if self.groups.is_some() {
            Flow::Rows
        } else {
            Flow::of(self.mode)
        }
    }

    /// The display position of the item stored at `index`, or `None` while it
    /// is folded or past the items.
    fn position_of(&self, index: usize) -> Option<usize> {
        match &self.groups {
            Some(groups) => groups.grouping.position_of(index),
            None => (index < self.count).then_some(index),
        }
    }

    /// The stored index of the item at display `position`.
    fn item_at(&self, position: usize) -> Option<usize> {
        match &self.groups {
            Some(groups) => groups.grouping.item_at(position),
            None => (position < self.count).then_some(position),
        }
    }

    /// Whether the item stored at `index` is shown, not folded away.
    pub fn is_shown(&self, index: usize) -> bool {
        self.position_of(index).is_some()
    }

    /// The stored indices of the items shown, in display order.
    pub fn shown(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.count).filter_map(|position| self.item_at(position))
    }

    /// Where the rows start along the scroll axis, inside the content area.
    fn lines_start(&self) -> i32 {
        self.header_height() + self.metrics.padding
    }

    /// The group whose band holds `y`, measured from `lines_start`.
    fn group_band_at(&self, y: i32) -> Option<usize> {
        let groups = self.groups.as_ref()?;
        let band = groups
            .tops
            .partition_point(|top| *top <= y)
            .checked_sub(1)?;
        (band < groups.grouping.groups().len()).then_some(band)
    }

    /// The cell side across the flow: a row's width, or a column's height.
    /// Details fills the content width, so its row is as wide as the content
    /// inside the padding.
    fn cross_cell(&self) -> i32 {
        if matches!(self.mode, ViewMode::Details | ViewMode::Content) {
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
        if let Some(groups) = &self.groups {
            return self.grouped_rect(groups, index);
        }
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
            if matches!(self.mode, ViewMode::Details | ViewMode::Content) {
                self.cross_cell()
            } else {
                self.metrics.cell.width
            },
            self.metrics.cell.height,
        )
    }

    /// A grouped item's cell, or an empty rect while it is folded.
    fn grouped_rect(&self, groups: &GroupedLines, index: usize) -> Rect {
        let grouping = &groups.grouping;
        let Some((position, group)) = grouping
            .position_of(index)
            .and_then(|position| Some((position, grouping.group_at(position)?)))
        else {
            return Rect::ZERO;
        };
        let cross_count = self.cross_count();
        let local = position - grouping.groups()[group].first();
        let line = (local / cross_count) as i32;
        let cross = (local % cross_count) as i32;
        Rect::new(
            self.metrics.padding + cross * self.cross_step(),
            self.lines_start()
                + groups.tops[group]
                + self.metrics.group_header
                + line * self.line_step(),
            if matches!(self.mode, ViewMode::Details | ViewMode::Content) {
                self.cross_cell()
            } else {
                self.metrics.cell.width
            },
            self.metrics.cell.height,
        )
    }

    /// Group `group`'s header row across the content, relative to the content
    /// area's origin, before scrolling.
    pub fn group_rect(&self, group: usize) -> Option<Rect> {
        let groups = self.groups.as_ref()?;
        Some(Rect::new(
            self.metrics.padding,
            self.lines_start() + groups.tops.get(group)?,
            (self.content.width - self.metrics.padding * 2).max(1),
            self.metrics.group_header,
        ))
    }

    /// The group whose header row holds `point`, relative to the content area
    /// and scrolled into that coordinate space.
    pub fn group_at(&self, point: Point) -> Option<usize> {
        let y = point.y - self.lines_start();
        let group = self.group_band_at(y)?;
        self.group_rect(group)?.contains(point).then_some(group)
    }

    /// The groups whose bands can be inside a scroll window.
    pub fn visible_groups(&self, scroll: i32, content: Size) -> Range<usize> {
        let Some(groups) = &self.groups else {
            return 0..0;
        };
        let start = scroll - self.lines_start();
        let end = start + self.viewport(content);
        let first = self
            .group_band_at(start.max(0))
            .unwrap_or(groups.tops.len());
        let count = groups.grouping.groups().len();
        let last = self
            .group_band_at(end.max(0))
            .map_or(count, |last| last + 1);
        first.min(count)..last
    }

    /// The display position `y` reaches in a grouped layout: the first item at
    /// or after it, measured from `lines_start`.
    fn grouped_position_at(&self, groups: &GroupedLines, y: i32) -> usize {
        let Some(group) = self.group_band_at(y.max(0)) else {
            return self.count;
        };
        let entry = &groups.grouping.groups()[group];
        let into_rows = y.max(0) - groups.tops[group] - self.metrics.group_header;
        if into_rows < 0 {
            return entry.first();
        }
        let local = (into_rows / self.line_step()) as usize * self.cross_count();
        entry.first() + local.min(entry.shown())
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
        if let Some(groups) = &self.groups {
            return match groups.tops.last() {
                Some(end) if *end > 0 => self.header_height() + self.metrics.padding * 2 + end,
                _ => 0,
            };
        }
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
        if let Some(groups) = &self.groups {
            return self.grouped_index_at(groups, point);
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

    fn grouped_index_at(&self, groups: &GroupedLines, point: Point) -> Option<usize> {
        let cross = point.x - self.metrics.padding;
        let y = point.y - self.lines_start();
        if cross < 0 || y < 0 {
            return None;
        }
        let group = self.group_band_at(y)?;
        let into_rows = y - groups.tops[group] - self.metrics.group_header;
        if into_rows < 0 {
            return None;
        }
        let cross = (cross / self.cross_step()) as usize;
        if cross >= self.cross_count() {
            return None;
        }
        let entry = &groups.grouping.groups()[group];
        let local = (into_rows / self.line_step()) as usize * self.cross_count() + cross;
        if local >= entry.shown() {
            return None;
        }
        let index = groups.grouping.item_at(entry.first() + local)?;
        self.rect(index).contains(point).then_some(index)
    }

    /// The nearest slot for a drag dropped at `point` (relative to the content
    /// area and scrolled into that coordinate space), clamped to the last item.
    pub fn drop_index(&self, point: Point) -> Option<usize> {
        if self.count == 0 || self.groups.is_some() {
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

    /// The stored indices of the items that can be inside a scroll window, in
    /// display order. A line may be partly visible, so callers still clip each
    /// rect to be exact.
    pub fn visible_items(&self, scroll: i32, content: Size) -> impl Iterator<Item = usize> + '_ {
        self.visible(scroll, content)
            .filter_map(|position| self.item_at(position))
    }

    /// The display positions whose lines can be inside a scroll window at
    /// `scroll` in a content area `content`.
    pub fn visible(&self, scroll: i32, content: Size) -> Range<usize> {
        let viewport = self.viewport(content);
        if self.count == 0 || viewport <= 0 {
            return 0..0;
        }
        if let Some(groups) = &self.groups {
            let start = scroll - self.lines_start();
            let first = self.grouped_position_at(groups, start);
            let last = self.grouped_position_at(groups, start + viewport);
            return first..(last + self.cross_count()).min(self.count);
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
        if !self.is_shown(index) {
            return None;
        }
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
            group_header: 30,
        }
    }

    #[test]
    fn fitting_uses_the_cells_it_is_given() {
        let metrics = metrics();
        assert_eq!(metrics.fit_width(480), 454);
        assert_eq!(metrics.fit_height(300, 28), 282);
    }

    #[test]
    fn fitting_keeps_the_grabbed_edge() {
        let metrics = metrics();
        let rect = Rect::new(10, 20, 480, 300);
        let right_bottom = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert_eq!(
            metrics.fit_rect(rect, right_bottom, 28, true),
            Rect::new(10, 20, 454, 282)
        );
        let left_top = ResizeEdges {
            left: true,
            top: true,
            ..ResizeEdges::NONE
        };
        assert_eq!(
            metrics.fit_rect(rect, left_top, 28, true),
            Rect::new(36, 38, 454, 282),
            "the far edges stay put"
        );
    }

    #[test]
    fn a_row_view_fits_only_its_height() {
        let metrics = metrics();
        let corner = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        assert_eq!(
            metrics.fit_rect(Rect::new(10, 20, 480, 300), corner, 28, false),
            Rect::new(10, 20, 480, 282)
        );
    }

    fn layout_of(mode: ViewMode, width: i32, count: usize) -> ViewLayout {
        ViewLayout::new(mode, Size::new(width, 400), count, metrics())
    }

    /// Groups of the given sizes over stored indices in reverse, so a display
    /// position is never the stored index by accident.
    fn grouping_of(sizes: &[usize], folded: &[usize]) -> Arc<Grouping> {
        let total: usize = sizes.iter().sum();
        let mut next = total;
        let groups = sizes
            .iter()
            .enumerate()
            .map(|(group, size)| {
                let items = (0..*size)
                    .map(|_| {
                        next -= 1;
                        next as u32
                    })
                    .collect();
                (group.to_string(), items)
            })
            .collect();
        let folded: Vec<String> = folded.iter().map(ToString::to_string).collect();
        Arc::new(Grouping::new(groups, total, |name| {
            folded.iter().any(|group| group == name)
        }))
    }

    fn grouped(mode: ViewMode, width: i32, sizes: &[usize], folded: &[usize]) -> ViewLayout {
        let total = sizes.iter().sum();
        ViewLayout::new(mode, Size::new(width, 400), total, metrics())
            .with_grouping(Some(grouping_of(sizes, folded)))
    }

    #[test]
    fn a_header_comes_before_each_group_and_rows_follow_it() {
        // 3 cells per row; groups of 4 and 2. Rows step 60, headers 30.
        let layout = grouped(ViewMode::SmallIcons, 360, &[4, 2], &[]);
        let first = layout.group_rect(0).unwrap();
        assert_eq!((first.y, first.height), (12, 30));
        assert_eq!(layout.rect(5).y, 12 + 30, "stored last, shown first");
        assert_eq!(layout.rect(4).x, layout.rect(5).x + 110);
        assert_eq!(layout.rect(2).y, 12 + 30 + 60, "the fourth wraps");
        let second = layout.group_rect(1).unwrap();
        assert_eq!(second.y, 12 + 30 + 2 * 60);
        assert_eq!(layout.rect(1).y, second.y + 30);
        assert_eq!(layout.extent(), 12 * 2 + 2 * 30 + 3 * 60);
    }

    #[test]
    fn a_grouped_view_keeps_its_details_header_above_the_groups() {
        let metrics = ViewMetrics {
            header: 25,
            gap: 0,
            ..metrics()
        };
        let layout = ViewLayout::new(ViewMode::Details, Size::new(360, 400), 2, metrics)
            .with_grouping(Some(grouping_of(&[2], &[])));
        assert_eq!(layout.group_rect(0).unwrap().y, 25 + 12);
        assert_eq!(layout.rect(1).y, 25 + 12 + 30);
        assert_eq!(layout.rect(1).width, 360 - 24, "rows stay full width");
    }

    #[test]
    fn a_grouped_list_stacks_downward() {
        let layout = grouped(ViewMode::List, 360, &[4, 2], &[]);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        assert_eq!(layout.viewport(Size::new(360, 400)), 400);
        assert_eq!(layout.rect(4).y, layout.rect(5).y, "rows, not columns");
    }

    #[test]
    fn a_folded_group_keeps_its_header_and_loses_its_rows() {
        let layout = grouped(ViewMode::SmallIcons, 360, &[4, 2], &[0]);
        assert_eq!(layout.group_rect(1).unwrap().y, 12 + 30);
        assert_eq!(layout.rect(1).y, 12 + 30 + 30);
        assert!(!layout.is_shown(5));
        assert!(layout.is_shown(1));
        assert_eq!(layout.rect(5), Rect::ZERO);
        assert_eq!(layout.shown().collect::<Vec<_>>(), [1, 0]);
        assert_eq!(layout.extent(), 12 * 2 + 2 * 30 + 60);
        assert_eq!(layout.scroll_to_show(5, 0, Size::new(360, 100)), None);
    }

    #[test]
    fn every_group_folded_still_scrolls_its_headers() {
        let layout = grouped(ViewMode::SmallIcons, 360, &[4, 2], &[0, 1]);
        assert_eq!(layout.extent(), 12 * 2 + 2 * 30);
        assert_eq!(layout.visible(0, Size::new(360, 400)), 0..0);
        assert_eq!(layout.index_at(Point::new(20, 50)), None);
    }

    #[test]
    fn a_grouped_space_with_no_groups_is_empty() {
        let layout = grouped(ViewMode::SmallIcons, 360, &[], &[]);
        assert_eq!(layout.extent(), 0);
        assert_eq!(layout.group_at(Point::new(20, 20)), None);
    }

    #[test]
    fn a_header_row_hits_its_group_and_no_item() {
        let layout = grouped(ViewMode::SmallIcons, 360, &[4, 2], &[]);
        let second = layout.group_rect(1).unwrap();
        let on_header = second.center();
        assert_eq!(layout.group_at(on_header), Some(1));
        assert_eq!(layout.index_at(on_header), None);
        let on_row = layout.rect(1).center();
        assert_eq!(layout.group_at(on_row), None);
        assert_eq!(layout.index_at(on_row), Some(1));
    }

    #[test]
    fn a_grouped_layout_refuses_a_drop_slot() {
        let layout = grouped(ViewMode::SmallIcons, 360, &[4, 2], &[]);
        assert_eq!(layout.drop_index(Point::new(20, 50)), None);
    }

    /// Walks every item and header to find what a scroll window should hold.
    #[test]
    fn the_visible_window_matches_a_brute_force_walk_on_random_groups() {
        let mut seed = 0x2545_f491_u32;
        let mut next = move |limit: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % limit
        };
        for _ in 0..200 {
            let sizes: Vec<usize> = (0..next(12) + 1).map(|_| next(40) as usize).collect();
            let folded: Vec<usize> = (0..sizes.len()).filter(|_| next(4) == 0).collect();
            let width = 120 + next(800) as i32;
            let layout = grouped(ViewMode::SmallIcons, width, &sizes, &folded);
            let content = Size::new(width, 50 + next(600) as i32);
            let scroll = next(layout.extent().max(1) as u32 + 100) as i32;
            let window = Rect::new(0, scroll, width, content.height);

            let visible: Vec<usize> = layout.visible_items(scroll, content).collect();
            let shown: Vec<usize> = layout.shown().collect();
            for item in &shown {
                if layout.rect(*item).intersects(window) {
                    assert!(
                        visible.contains(item),
                        "{sizes:?} folded {folded:?} width {width} scroll {scroll}: {item}"
                    );
                }
            }
            for group in 0..sizes.len() {
                let header = layout.group_rect(group).unwrap();
                if header.intersects(window) {
                    assert!(
                        layout.visible_groups(scroll, content).contains(&group),
                        "{sizes:?} folded {folded:?} scroll {scroll}: header {group}"
                    );
                }
            }
            // Every shown item is found at its own cell, and no two overlap.
            for item in &shown {
                let rect = layout.rect(*item);
                assert_eq!(layout.index_at(rect.center()), Some(*item));
            }
        }
    }

    #[test]
    fn compact_rows_are_shorter_in_every_order_placed_view() {
        let grid = GridSpec::default();
        for mode in [
            ViewMode::SmallIcons,
            ViewMode::List,
            ViewMode::Details,
            ViewMode::Tiles,
            ViewMode::Content,
        ] {
            let normal = SpaceView::default().with_mode(mode);
            let compact = SpaceView {
                compact: true,
                ..normal.clone()
            };
            let (normal, compact) = (
                ViewMetrics::for_view(&normal, grid, 1.0),
                ViewMetrics::for_view(&compact, grid, 1.0),
            );
            assert!(compact.cell.height < normal.cell.height, "{mode:?}");
            assert_eq!(compact.cell.width, normal.cell.width, "{mode:?}");
            assert!(compact.gap <= normal.gap, "{mode:?}");
        }
        let details = |compact| SpaceView {
            compact,
            ..SpaceView::default().with_mode(ViewMode::Details)
        };
        assert!(
            ViewMetrics::for_view(&details(true), grid, 1.0).header
                < ViewMetrics::for_view(&details(false), grid, 1.0).header
        );
    }

    #[test]
    fn compact_leaves_the_icon_views_alone() {
        let grid = GridSpec::default();
        for mode in [
            ViewMode::Icons,
            ViewMode::ExtraLargeIcons,
            ViewMode::LargeIcons,
            ViewMode::MediumIcons,
        ] {
            let normal = SpaceView::default().with_mode(mode);
            let compact = SpaceView {
                compact: true,
                ..normal.clone()
            };
            assert_eq!(
                ViewMetrics::for_view(&compact, grid, 1.5),
                ViewMetrics::for_view(&normal, grid, 1.5),
                "{mode:?}"
            );
        }
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
        let base = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::SmallIcons),
            grid,
            1.0,
        );
        let large = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::SmallIcons),
            grid,
            1.5,
        );
        assert_eq!(base.cell, Size::new(200, 22));
        assert_eq!(large.cell, Size::new(300, 33));
        assert_eq!(base.gap, 2);
        assert_eq!(large.gap, 3);
        assert_eq!(base.padding, grid.padding, "the look's edge padding");
        // List is an inline cell too.
        assert_eq!(
            ViewMetrics::for_view(&SpaceView::default().with_mode(ViewMode::List), grid, 1.0).cell,
            Size::new(200, 22)
        );
        // An icon view ignores the display scale; its grid is already scaled.
        assert_eq!(
            ViewMetrics::for_view(&SpaceView::default().with_mode(ViewMode::Icons), grid, 1.5).cell,
            ViewMetrics::for_grid(grid).cell
        );
        assert_eq!(crate::view::SMALL_ICON_PX, 16);
    }

    #[test]
    fn details_rows_run_full_width_under_a_fixed_header() {
        let grid = GridSpec::default();
        let padding = grid.padding;
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Details),
            grid,
            1.0,
        );
        let layout = ViewLayout::new(ViewMode::Details, Size::new(360, 200), 5, metrics);
        assert_eq!(layout.header_height(), NORMAL_CELLS.details_header);
        assert_eq!(layout.cross_count(), 1);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        assert_eq!(layout.scroll_step(), NORMAL_CELLS.details_row);
        let row = layout.rect(0);
        assert_eq!(
            row,
            Rect::new(
                padding,
                NORMAL_CELLS.details_header + padding,
                360 - padding * 2,
                NORMAL_CELLS.details_row
            )
        );
        assert_eq!(layout.rect(1).y, row.y + NORMAL_CELLS.details_row);
        assert_eq!(
            layout.extent(),
            NORMAL_CELLS.details_header + padding * 2 + 5 * NORMAL_CELLS.details_row
        );
        // The header row itself holds no item.
        assert_eq!(layout.index_at(Point::new(row.center().x, 5)), None);
        assert_eq!(layout.index_at(layout.rect(2).center()), Some(2));
    }

    #[test]
    fn details_scale_their_rows_and_header_with_the_display() {
        let base = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Details),
            GridSpec::default(),
            1.0,
        );
        let large = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Details),
            GridSpec::default(),
            1.5,
        );
        assert_eq!((base.header, base.cell.height), (25, 22));
        assert_eq!((large.header, large.cell.height), (38, 33));
    }

    #[test]
    fn details_scroll_below_the_header_and_still_show_the_first_rows() {
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Details),
            GridSpec::default(),
            1.0,
        );
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
    fn tiles_wrap_into_rows_with_their_own_cell() {
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Tiles),
            GridSpec::default(),
            1.0,
        );
        assert_eq!(metrics.cell, Size::new(210, 60));
        let layout = ViewLayout::new(ViewMode::Tiles, Size::new(460, 300), 5, metrics);
        assert_eq!(layout.axis(), ScrollAxis::Vertical);
        assert_eq!(layout.cross_count(), 2);
        assert_eq!(layout.rect(0).width, 210);
        assert_eq!(layout.rect(1).x, 12 + 212);
        assert_eq!(layout.rect(2).y, 12 + 62);
        assert_eq!(layout.extent(), 12 * 2 + 3 * 60 + 2 * 2);
    }

    #[test]
    fn content_rows_fill_the_width_and_touch() {
        let metrics = ViewMetrics::for_view(
            &SpaceView::default().with_mode(ViewMode::Content),
            GridSpec::default(),
            1.0,
        );
        let layout = ViewLayout::new(ViewMode::Content, Size::new(360, 200), 3, metrics);
        assert_eq!(layout.header_height(), 0);
        assert_eq!(layout.cross_count(), 1);
        assert_eq!(layout.rect(0), Rect::new(12, 12, 336, 32));
        assert_eq!(layout.rect(1).y, 44);
        assert_eq!(layout.extent(), 12 * 2 + 3 * 32);
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
