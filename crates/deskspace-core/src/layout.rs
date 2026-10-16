use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Rect, Size};
use crate::space::{DEFAULT_TITLE_HEIGHT, ResizeEdges, Space};

/// The bounds grid `gap` and `padding` are clamped to, in logical pixels.
pub const GAP_RANGE: (i32, i32) = (0, 24);
/// The bounds grid `snap` is clamped to, in logical pixels.
pub const SNAP_RANGE: (i32, i32) = (0, 64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridSpec {
    pub cell_width: i32,
    pub cell_height: i32,
    pub gap: i32,
    pub padding: i32,
    pub title_height: i32,
    pub columns: i32,
    pub snap: i32,
    /// Round a user-sized space's width and height to whole grid columns and
    /// rows when it is resized, so its edge padding stays even. Off keeps the
    /// exact dragged size (still snapped to `snap`).
    pub fit_to_grid: bool,
}

impl Default for GridSpec {
    fn default() -> Self {
        Self {
            cell_width: 96,
            cell_height: 104,
            gap: 10,
            padding: 12,
            title_height: DEFAULT_TITLE_HEIGHT,
            columns: 4,
            snap: 16,
            fit_to_grid: true,
        }
    }
}

impl GridSpec {
    /// This grid scaled by `scale` for a display or icon-size factor. The
    /// stored spec is the logical 96-DPI / 100 % base; the app scales it for
    /// the monitor's DPI and the icon-size setting, so no persisted value has
    /// to carry a scale.
    pub fn scaled(&self, scale: f32) -> Self {
        let scale = scale.max(0.1);
        let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
        Self {
            cell_width: scaled(self.cell_width),
            cell_height: scaled(self.cell_height),
            gap: scaled(self.gap),
            padding: scaled(self.padding),
            title_height: scaled(self.title_height),
            columns: self.columns,
            snap: scaled(self.snap),
            fit_to_grid: self.fit_to_grid,
        }
    }

    pub const fn cell(&self) -> Size {
        Size::new(self.cell_width, self.cell_height)
    }

    /// This grid with `gap`, `padding`, and `snap` brought into range, so a
    /// hand-edited config loads within the bounds its settings expose.
    pub fn clamped(self) -> Self {
        let clamp = |value: i32, range: (i32, i32)| value.clamp(range.0, range.1);
        Self {
            gap: clamp(self.gap, GAP_RANGE),
            padding: clamp(self.padding, GAP_RANGE),
            snap: clamp(self.snap, SNAP_RANGE),
            ..self
        }
    }

    pub fn grid_position(&self, index: usize) -> Point {
        self.grid_position_in(index, self.columns)
    }

    pub fn grid_position_in(&self, index: usize, columns: i32) -> Point {
        let columns = columns.max(1) as usize;
        let column = (index % columns) as i32;
        let row = (index / columns) as i32;
        Point::new(
            self.padding + column * (self.cell_width + self.gap),
            self.padding + row * (self.cell_height + self.gap),
        )
    }

    /// How many cell columns fit in a space `width` pixels wide (at least 1).
    pub fn columns_for(&self, width: i32) -> i32 {
        let step = (self.cell_width + self.gap).max(1);
        ((width - self.padding * 2 + self.gap) / step).max(1)
    }

    /// The width that holds `columns` whole columns with equal edge padding.
    pub fn width_for_columns(&self, columns: i32) -> i32 {
        let columns = columns.max(1);
        self.padding * 2 + columns * (self.cell_width + self.gap) - self.gap
    }

    /// `width` rounded down to whole columns, so a user-sized space has the
    /// same padding on both sides instead of the remainder on the right.
    pub fn fit_width(&self, width: i32) -> i32 {
        self.width_for_columns(self.columns_for(width))
    }

    /// How many cell rows fit in a space `height` pixels tall, below the title
    /// bar (at least 1).
    pub fn rows_for(&self, height: i32) -> i32 {
        let step = (self.cell_height + self.gap).max(1);
        ((height - self.title_height - self.padding * 2 + self.gap) / step).max(1)
    }

    /// The height that holds `rows` whole rows below the title bar with equal
    /// top and bottom padding.
    pub fn height_for_rows(&self, rows: i32) -> i32 {
        let rows = rows.max(1);
        self.title_height + self.padding * 2 + rows * (self.cell_height + self.gap) - self.gap
    }

    /// `height` rounded down to whole rows, so a user-sized space has the same
    /// padding above and below the grid instead of the remainder at the bottom.
    pub fn fit_height(&self, height: i32) -> i32 {
        self.height_for_rows(self.rows_for(height))
    }

    /// The smallest space a user can resize to: one cell plus padding.
    pub fn min_space_size(&self) -> Size {
        Size::new(
            self.padding * 2 + self.cell_width,
            self.title_height + self.padding * 2 + self.cell_height,
        )
    }

    /// The pixels one mouse-wheel notch scrolls: one row of cells.
    pub fn scroll_step(&self) -> i32 {
        self.cell_height + self.gap
    }
}

/// Moves the `edges` of `rect` to `point`, snapped to `snap`, keeping the
/// opposite edges fixed. The result is at least `min` and stays inside
/// `bounds`.
pub fn resize_rect(
    rect: Rect,
    edges: ResizeEdges,
    point: Point,
    min: Size,
    snap: i32,
    bounds: Rect,
) -> Rect {
    let mut left = rect.left();
    let mut top = rect.top();
    let mut right = rect.right();
    let mut bottom = rect.bottom();
    if edges.left {
        left = snap_to_grid(point.x, snap)
            .max(bounds.left())
            .min(right - min.width);
    }
    if edges.right {
        right = snap_to_grid(point.x, snap)
            .min(bounds.right())
            .max(left + min.width);
    }
    if edges.top {
        top = snap_to_grid(point.y, snap)
            .max(bounds.top())
            .min(bottom - min.height);
    }
    if edges.bottom {
        bottom = snap_to_grid(point.y, snap)
            .min(bounds.bottom())
            .max(top + min.height);
    }
    Rect::new(left, top, right - left, bottom - top)
}

/// Rounds `rect`'s width down to whole columns, keeping equal edge padding.
/// The grabbed edge stays put, so a left resize moves the left edge and any
/// other resize moves the right one.
pub fn fit_rect_width(rect: Rect, edges: ResizeEdges, spec: &GridSpec) -> Rect {
    let width = spec.fit_width(rect.width);
    if edges.left {
        Rect::new(rect.right() - width, rect.y, width, rect.height)
    } else {
        Rect::new(rect.x, rect.y, width, rect.height)
    }
}

/// Rounds `rect`'s height down to whole rows below the title bar, keeping equal
/// top and bottom padding. The grabbed edge stays put, so a top resize moves
/// the top edge and any other resize moves the bottom one.
pub fn fit_rect_height(rect: Rect, edges: ResizeEdges, spec: &GridSpec) -> Rect {
    let height = spec.fit_height(rect.height);
    if edges.top {
        Rect::new(rect.x, rect.bottom() - height, rect.width, height)
    } else {
        Rect::new(rect.x, rect.y, rect.width, height)
    }
}

/// Sizes the space to fit its items (unless the user sized it) and lays the
/// items out. Locked spaces are left alone.
pub fn arrange_space(space: &mut Space, spec: GridSpec) {
    if space.locked {
        return;
    }
    // The title height follows the grid even for user-sized spaces, so an icon
    // or DPI change reaches every space.
    space.title_height = spec.title_height;
    if space.user_sized {
        position_items(space, spec);
        return;
    }
    let columns = spec.columns.max(1) as usize;
    let rows = space.items.len().div_ceil(columns).max(1) as i32;
    let width = spec.padding * 2
        + spec.columns.max(1) * spec.cell_width
        + (spec.columns.max(1) - 1) * spec.gap;
    let height =
        spec.title_height + spec.padding * 2 + rows * spec.cell_height + (rows - 1) * spec.gap;
    space.rect.width = width;
    space.rect.height = height;
    position_items(space, spec);
}

/// Places items on the grid. Positions are relative to the space's content
/// area, so moving the space never has to touch its items. User-sized spaces
/// use as many columns as their width allows; others use `spec.columns`.
pub fn position_items(space: &mut Space, spec: GridSpec) {
    let columns = if space.user_sized {
        spec.columns_for(space.rect.width)
    } else {
        spec.columns
    };
    for (index, item) in space.items.iter_mut().enumerate() {
        item.position = spec.grid_position_in(index, columns);
    }
}

pub fn snap_to_grid(value: i32, grid: i32) -> i32 {
    if grid <= 1 {
        return value;
    }
    (value as f32 / grid as f32).round() as i32 * grid
}

pub fn snap_rect(rect: Rect, grid: i32) -> Rect {
    if grid <= 1 {
        return rect;
    }
    Rect::new(
        snap_to_grid(rect.x, grid),
        snap_to_grid(rect.y, grid),
        rect.width,
        rect.height,
    )
}

pub fn clamp_to_bounds(rect: Rect, bounds: Rect) -> Rect {
    let max_x = (bounds.right() - rect.width).max(bounds.x);
    let max_y = (bounds.bottom() - rect.height).max(bounds.y);
    Rect::new(
        rect.x.clamp(bounds.x, max_x),
        rect.y.clamp(bounds.y, max_y),
        rect.width,
        rect.height,
    )
}

pub fn resolve_collisions(spaces: &mut [Space], gap: i32, bounds: Rect) {
    let mut placed: Vec<Rect> = Vec::with_capacity(spaces.len());
    for space in spaces.iter_mut() {
        if space.locked {
            placed.push(space.rect);
            continue;
        }
        let mut candidate = space.rect;
        let mut attempts = 0;
        while placed.iter().any(|other| other.intersects(candidate)) && attempts < 64 {
            candidate.y += candidate.height + gap;
            candidate = clamp_to_bounds(candidate, bounds);
            attempts += 1;
        }
        space.rect = candidate;
        placed.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{ItemId, SpaceId};
    use crate::item::Item;
    use crate::item::ItemFacts;

    fn space_with_items(count: usize) -> Space {
        let mut space = Space::new(SpaceId(1), "Test", Rect::new(0, 0, 10, 10));
        for index in 0..count {
            space.push_item(Item::new(
                ItemId(index as u64 + 1),
                format!("item{index}"),
                ItemFacts::default(),
            ));
        }
        space
    }

    #[test]
    fn arrange_positions_items_inside_content() {
        let spec = GridSpec::default();
        let mut space = space_with_items(5);
        space.rect.x = 100;
        space.rect.y = 200;
        arrange_space(&mut space, spec);

        let content = space.content_rect();
        let local_content = Rect::new(0, 0, content.width, content.height);
        assert_eq!(space.items.len(), 5);
        for item in &space.items {
            assert!(local_content.contains(item.position));
        }
    }

    #[test]
    fn moving_a_space_keeps_item_positions() {
        let spec = GridSpec::default();
        let mut space = space_with_items(3);
        arrange_space(&mut space, spec);
        let before: Vec<Point> = space.items.iter().map(|item| item.position).collect();
        space.set_position(Point::new(500, 400));
        let after: Vec<Point> = space.items.iter().map(|item| item.position).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn scaling_grows_the_grid_but_keeps_columns() {
        let base = GridSpec::default();
        let doubled = base.scaled(2.0);
        assert_eq!(doubled.cell_width, base.cell_width * 2);
        assert_eq!(doubled.cell_height, base.cell_height * 2);
        assert_eq!(doubled.gap, base.gap * 2);
        assert_eq!(doubled.padding, base.padding * 2);
        assert_eq!(doubled.title_height, base.title_height * 2);
        assert_eq!(doubled.columns, base.columns);
        // A scale never collapses a dimension to zero.
        assert!(base.scaled(0.0).cell_width >= 1);
        assert_eq!(base.scaled(1.0), base);
    }

    #[test]
    fn snap_rounds_to_nearest_multiple() {
        assert_eq!(snap_to_grid(23, 16), 16);
        assert_eq!(snap_to_grid(25, 16), 32);
        assert_eq!(snap_to_grid(7, 0), 7);
    }

    #[test]
    fn clamped_brings_the_grid_into_range() {
        let spec = GridSpec {
            gap: 999,
            padding: -4,
            snap: 500,
            ..GridSpec::default()
        }
        .clamped();
        assert_eq!(spec.gap, GAP_RANGE.1);
        assert_eq!(spec.padding, GAP_RANGE.0);
        assert_eq!(spec.snap, SNAP_RANGE.1);
        assert_eq!(GridSpec::default().clamped(), GridSpec::default());
    }

    #[test]
    fn clamp_keeps_rect_inside_bounds() {
        let bounds = Rect::new(0, 0, 800, 600);
        let rect = Rect::new(900, 700, 100, 100);
        let clamped = clamp_to_bounds(rect, bounds);
        assert_eq!(clamped, Rect::new(700, 500, 100, 100));
    }

    #[test]
    fn columns_follow_width() {
        let spec = GridSpec::default();
        // 12 + 96 + 12 fits one column; each further column needs 106.
        assert_eq!(spec.columns_for(120), 1);
        assert_eq!(spec.columns_for(225), 1);
        assert_eq!(spec.columns_for(226), 2);
        assert_eq!(spec.columns_for(0), 1);
    }

    #[test]
    fn width_fits_whole_columns() {
        let spec = GridSpec::default();
        // One column is padding + cell + padding; each further column adds 106.
        assert_eq!(spec.width_for_columns(1), 120);
        assert_eq!(spec.width_for_columns(4), 438);
        assert_eq!(spec.fit_width(480), 438);
        assert_eq!(spec.fit_width(438), 438, "already exact");
        assert_eq!(spec.fit_width(437), 332, "rounds down to three columns");
        assert_eq!(spec.fit_width(0), spec.min_space_size().width);
    }

    #[test]
    fn fit_rect_width_keeps_the_grabbed_edge() {
        let spec = GridSpec::default();
        let rect = Rect::new(100, 20, 480, 200);
        let right = fit_rect_width(
            rect,
            ResizeEdges {
                right: true,
                ..ResizeEdges::NONE
            },
            &spec,
        );
        assert_eq!(right, Rect::new(100, 20, 438, 200));
        let left = fit_rect_width(
            rect,
            ResizeEdges {
                left: true,
                ..ResizeEdges::NONE
            },
            &spec,
        );
        assert_eq!(left.right(), rect.right(), "the far edge stays put");
        assert_eq!(left.width, 438);
    }

    #[test]
    fn height_fits_whole_rows() {
        let spec = GridSpec::default();
        // Title bar and padding, then rows of 114; the last row has no gap.
        assert_eq!(spec.height_for_rows(1), spec.min_space_size().height);
        assert_eq!(spec.height_for_rows(2), 28 + 24 + 2 * 114 - 10);
        assert_eq!(spec.fit_height(320), 270);
        assert_eq!(spec.fit_height(270), 270, "already exact");
        assert_eq!(spec.fit_height(100), spec.min_space_size().height);
    }

    #[test]
    fn fit_rect_height_keeps_the_grabbed_edge() {
        let spec = GridSpec::default();
        let rect = Rect::new(100, 20, 438, 320);
        let bottom = fit_rect_height(
            rect,
            ResizeEdges {
                bottom: true,
                ..ResizeEdges::NONE
            },
            &spec,
        );
        assert_eq!(bottom, Rect::new(100, 20, 438, 270));
        let top = fit_rect_height(
            rect,
            ResizeEdges {
                top: true,
                ..ResizeEdges::NONE
            },
            &spec,
        );
        assert_eq!(top.bottom(), rect.bottom(), "the far edge stays put");
        assert_eq!(top.height, 270);
    }

    #[test]
    fn user_sized_spaces_keep_size_and_reflow() {
        let spec = GridSpec::default();
        let mut space = space_with_items(5);
        space.user_sized = true;
        space.rect = Rect::new(0, 0, 226, 150);
        arrange_space(&mut space, spec);
        assert_eq!(space.rect, Rect::new(0, 0, 226, 150));
        assert_eq!(space.items[2].position, Point::new(12, 12 + 104 + 10));
        assert_eq!(space.items[1].position.y, space.items[0].position.y);
    }

    #[test]
    fn resize_moves_only_grabbed_edges_and_respects_minimum() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let min = Size::new(120, 160);
        let rect = Rect::new(400, 400, 400, 300);
        let bottom_right = ResizeEdges {
            right: true,
            bottom: true,
            ..ResizeEdges::NONE
        };
        let grown = resize_rect(rect, bottom_right, Point::new(1001, 899), min, 16, bounds);
        assert_eq!(grown, Rect::new(400, 400, 608, 496));

        let left = ResizeEdges {
            left: true,
            ..ResizeEdges::NONE
        };
        let squeezed = resize_rect(rect, left, Point::new(790, 500), min, 16, bounds);
        assert_eq!(squeezed, Rect::new(680, 400, 120, 300));

        let clamped = resize_rect(rect, bottom_right, Point::new(5000, 5000), min, 16, bounds);
        assert_eq!((clamped.right(), clamped.bottom()), (1920, 1080));
    }

    #[test]
    fn collisions_push_spaces_down() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let mut spaces = vec![
            Space::new(SpaceId(1), "A", Rect::new(0, 0, 200, 200)),
            Space::new(SpaceId(2), "B", Rect::new(0, 0, 200, 200)),
        ];
        resolve_collisions(&mut spaces, 10, bounds);
        assert!(!spaces[0].rect.intersects(spaces[1].rect));
    }
}
