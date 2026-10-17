//! Where a new space goes: in line with the top row of spaces on its
//! monitor, one typical gutter to the right of them (ROADMAP D10), or, for
//! one opened by a drop on the desktop, as near the drop point as it fits
//! (ROADMAP D7).

use crate::geometry::{Point, Rect, Size};

/// The gutter between spaces when there are none to measure one from, in
/// logical pixels: two default snap steps. The caller scales it for DPI.
pub const DEFAULT_SPACE_GAP: i32 = 32;

/// Where a new space of `size` goes on the monitor whose work area is
/// `bounds`, given the rects of every existing space.
///
/// It goes one gutter to the right of the top row of spaces on that monitor,
/// its top in line with the row's. A row with no room left gives way to the
/// next one down, and the last resort is a new row under all of them, at
/// their left edge. An empty monitor starts one gutter in from the work
/// area's top-left corner. The gutter is the median gap between neighbouring
/// spaces there, so it matches an even layout exactly and takes the typical
/// value of an uneven one; with nothing to measure it is `default_gap`.
///
/// `None` when no slot keeps half a gutter clear of every space, so the
/// caller places the space itself.
pub fn new_space_origin(
    spaces: &[Rect],
    size: Size,
    bounds: Rect,
    default_gap: i32,
) -> Option<Point> {
    let local: Vec<Rect> = spaces
        .iter()
        .copied()
        .filter(|rect| bounds.contains(rect.center()))
        .collect();
    let gap = typical_gap(&local, size).unwrap_or(default_gap);
    candidates(&local, bounds, gap)
        .into_iter()
        .map(|origin| Rect::from_point_size(origin, size))
        .find(|rect| fits(*rect, bounds) && is_clear(*rect, spaces, gap))
        .map(Rect::origin)
}

/// Where a space of `size` opened by a drop at `at` goes: its top-left
/// corner as near `at` as it can be while keeping a full `gap` from every
/// space in `spaces` and from the edges of `bounds`, the work area, so the
/// taskbar keeps a gutter too.
///
/// The nearest such spot either keeps `at`'s coordinate on an axis or sits
/// one gutter beside a space on it, so only those positions are tried.
/// `None` when the work area has no such spot.
pub fn drop_space_origin(
    at: Point,
    spaces: &[Rect],
    size: Size,
    bounds: Rect,
    gap: i32,
) -> Option<Point> {
    let inner = bounds.inset(gap);
    let (max_x, max_y) = (inner.right() - size.width, inner.bottom() - size.height);
    if max_x < inner.x || max_y < inner.y {
        return None;
    }
    let xs: Vec<i32> = std::iter::once(at.x)
        .chain(
            spaces
                .iter()
                .flat_map(|space| [space.x - gap - size.width, space.right() + gap]),
        )
        .map(|x| x.clamp(inner.x, max_x))
        .collect();
    let ys: Vec<i32> = std::iter::once(at.y)
        .chain(
            spaces
                .iter()
                .flat_map(|space| [space.y - gap - size.height, space.bottom() + gap]),
        )
        .map(|y| y.clamp(inner.y, max_y))
        .collect();
    let distance = |origin: Point| {
        let (dx, dy) = (i64::from(origin.x - at.x), i64::from(origin.y - at.y));
        dx * dx + dy * dy
    };
    xs.iter()
        .flat_map(|x| ys.iter().map(move |y| Point::new(*x, *y)))
        .filter(|origin| {
            let margin = Rect::from_point_size(*origin, size).inset(-gap);
            !spaces.iter().any(|space| space.intersects(margin))
        })
        .min_by_key(|origin| distance(*origin))
}

/// The slots to try, best first: the end of each row, top to bottom, then a
/// new row underneath.
fn candidates(local: &[Rect], bounds: Rect, gap: i32) -> Vec<Point> {
    let (Some(left), Some(bottom)) = (
        local.iter().map(|rect| rect.x).min(),
        local.iter().map(|rect| rect.bottom()).max(),
    ) else {
        return vec![Point::new(bounds.x + gap, bounds.y + gap)];
    };
    let mut slots: Vec<Point> = rows(local)
        .iter()
        .filter_map(|row| {
            let right = row.iter().map(|rect| rect.right()).max()?;
            Some(Point::new(right + gap, row[0].y))
        })
        .collect();
    slots.push(Point::new(left, bottom + gap));
    slots
}

/// `spaces` grouped into rows, top to bottom. A space joins the row above
/// when its top is within half that row's first space's height of the row's
/// top, so slightly staggered spaces still read as one row.
fn rows(spaces: &[Rect]) -> Vec<Vec<Rect>> {
    let mut sorted = spaces.to_vec();
    sorted.sort_by_key(|rect| (rect.y, rect.x));
    let mut rows: Vec<Vec<Rect>> = Vec::new();
    for rect in sorted {
        match rows.last_mut() {
            Some(row) if rect.y - row[0].y <= row[0].height / 2 => row.push(rect),
            _ => rows.push(vec![rect]),
        }
    }
    rows
}

/// The median gutter between neighbouring `spaces`: side by side if any
/// are, else stacked. A gap the new space would fit in is open desktop, not
/// a gutter, so it is left out.
fn typical_gap(spaces: &[Rect], size: Size) -> Option<i32> {
    median(horizontal_gaps(spaces, size.width))
        .or_else(|| median(vertical_gaps(spaces, size.height)))
}

/// For each space, the gap to its nearest neighbour on the left that shares
/// some of its height, when that gap is below `limit`.
fn horizontal_gaps(spaces: &[Rect], limit: i32) -> Vec<i32> {
    spaces
        .iter()
        .filter_map(|space| {
            spaces
                .iter()
                .filter(|other| spans_overlap(other.y, other.bottom(), space.y, space.bottom()))
                .map(|other| space.x - other.right())
                .filter(|gap| (0..limit).contains(gap))
                .min()
        })
        .collect()
}

/// For each space, the gap to its nearest neighbour above that shares some of
/// its width, when that gap is below `limit`.
fn vertical_gaps(spaces: &[Rect], limit: i32) -> Vec<i32> {
    spaces
        .iter()
        .filter_map(|space| {
            spaces
                .iter()
                .filter(|other| spans_overlap(other.x, other.right(), space.x, space.right()))
                .map(|other| space.y - other.bottom())
                .filter(|gap| (0..limit).contains(gap))
                .min()
        })
        .collect()
}

fn spans_overlap(start: i32, end: i32, other_start: i32, other_end: i32) -> bool {
    start < other_end && other_start < end
}

fn median(mut values: Vec<i32>) -> Option<i32> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2
    } else {
        values[middle]
    })
}

fn fits(rect: Rect, bounds: Rect) -> bool {
    rect.x >= bounds.x
        && rect.y >= bounds.y
        && rect.right() <= bounds.right()
        && rect.bottom() <= bounds.bottom()
}

/// Whether `rect` keeps at least half a gutter from every space.
fn is_clear(rect: Rect, spaces: &[Rect], gap: i32) -> bool {
    let margin = rect.inset(-(gap / 2));
    !spaces.iter().any(|space| space.intersects(margin))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::new(0, 0, 1920, 1040);
    const SIZE: Size = Size::new(240, 120);

    fn place(spaces: &[Rect]) -> Option<Point> {
        new_space_origin(spaces, SIZE, BOUNDS, DEFAULT_SPACE_GAP)
    }

    #[test]
    fn an_empty_monitor_starts_one_gutter_in_from_its_corner() {
        assert_eq!(place(&[]), Some(Point::new(32, 32)));
        let second = Rect::new(1920, 0, 1920, 1040);
        assert_eq!(
            new_space_origin(&[], SIZE, second, DEFAULT_SPACE_GAP),
            Some(Point::new(1952, 32))
        );
    }

    #[test]
    fn a_new_space_joins_the_top_row_one_gutter_along() {
        // Two spaces side by side 34 px apart, a wider one below them.
        let spaces = [
            Rect::new(8, 15, 173, 147),
            Rect::new(215, 15, 322, 147),
            Rect::new(8, 192, 333, 253),
        ];
        assert_eq!(place(&spaces), Some(Point::new(537 + 34, 15)));
    }

    #[test]
    fn a_slightly_lower_space_still_belongs_to_the_top_row() {
        let spaces = [Rect::new(0, 0, 200, 100), Rect::new(230, 16, 200, 100)];
        assert_eq!(place(&spaces), Some(Point::new(430 + 30, 0)));
    }

    #[test]
    fn uneven_gutters_take_the_median() {
        let spaces = [
            Rect::new(0, 0, 200, 100),
            Rect::new(220, 0, 200, 100),
            Rect::new(460, 0, 200, 100),
            Rect::new(0, 300, 200, 100),
            Rect::new(230, 300, 200, 100),
        ];
        // Gutters of 20, 40, and 30.
        assert_eq!(place(&spaces), Some(Point::new(660 + 30, 0)));
    }

    #[test]
    fn a_gap_the_new_space_fits_in_is_not_a_gutter() {
        let spaces = [
            Rect::new(0, 0, 200, 100),
            Rect::new(220, 0, 200, 100),
            Rect::new(900, 0, 200, 100),
        ];
        assert_eq!(place(&spaces), Some(Point::new(1100 + 20, 0)));
    }

    #[test]
    fn stacked_spaces_set_the_gutter_when_none_sit_side_by_side() {
        let spaces = [Rect::new(0, 0, 400, 150), Rect::new(0, 200, 400, 150)];
        assert_eq!(place(&spaces), Some(Point::new(400 + 50, 0)));
    }

    #[test]
    fn a_lone_space_gets_the_default_gutter() {
        let spaces = [Rect::new(100, 100, 300, 200)];
        assert_eq!(place(&spaces), Some(Point::new(400 + 32, 100)));
    }

    #[test]
    fn a_full_row_gives_way_to_the_next() {
        let spaces = [Rect::new(0, 0, 1700, 150), Rect::new(0, 200, 400, 150)];
        assert_eq!(place(&spaces), Some(Point::new(400 + 50, 200)));
    }

    #[test]
    fn full_rows_open_a_new_row_underneath() {
        let spaces = [Rect::new(10, 0, 1700, 150), Rect::new(10, 200, 1700, 150)];
        assert_eq!(place(&spaces), Some(Point::new(10, 350 + 50)));
    }

    #[test]
    fn a_slot_too_close_to_another_space_is_skipped() {
        // The top row's slot would run into the tall space below it.
        let spaces = [
            Rect::new(0, 0, 200, 100),
            Rect::new(232, 0, 200, 100),
            Rect::new(500, 110, 200, 300),
        ];
        assert_eq!(place(&spaces), Some(Point::new(700 + 32, 110)));
    }

    #[test]
    fn spaces_on_another_monitor_do_not_set_the_row() {
        let second = Rect::new(1920, 0, 1920, 1040);
        let spaces = [Rect::new(100, 100, 300, 200)];
        assert_eq!(
            new_space_origin(&spaces, SIZE, second, DEFAULT_SPACE_GAP),
            Some(Point::new(1952, 32))
        );
    }

    #[test]
    fn a_full_monitor_has_no_slot() {
        assert_eq!(place(&[BOUNDS]), None);
    }

    fn drop_at(x: i32, y: i32, spaces: &[Rect]) -> Option<Point> {
        drop_space_origin(Point::new(x, y), spaces, SIZE, BOUNDS, DEFAULT_SPACE_GAP)
    }

    #[test]
    fn a_drop_on_open_desktop_opens_the_space_at_the_drop_point() {
        assert_eq!(drop_at(500, 400, &[]), Some(Point::new(500, 400)));
    }

    #[test]
    fn a_drop_near_the_edge_keeps_a_gutter_from_the_work_area() {
        // The work area ends above the taskbar, so the bottom gutter is
        // kept from the taskbar too.
        assert_eq!(drop_at(5, 1030, &[]), Some(Point::new(32, 1040 - 32 - 120)));
        assert_eq!(
            drop_at(1900, 10, &[]),
            Some(Point::new(1920 - 32 - 240, 32))
        );
    }

    #[test]
    fn a_drop_on_a_space_moves_to_the_nearest_clear_side() {
        // Dropped just inside the space's right edge: beside it is nearer
        // than below it.
        let space = Rect::new(400, 300, 300, 400);
        assert_eq!(drop_at(680, 450, &[space]), Some(Point::new(700 + 32, 450)));
        // Just inside its bottom edge: below it is nearer.
        assert_eq!(drop_at(450, 690, &[space]), Some(Point::new(450, 700 + 32)));
    }

    #[test]
    fn a_drop_between_spaces_keeps_a_full_gutter_from_both() {
        // A 300 px gap between two spaces fits the 240 px space but not its
        // two 32 px gutters, so it goes past one of them.
        let left = Rect::new(100, 100, 300, 300);
        let right = Rect::new(700, 100, 300, 300);
        let origin = drop_at(420, 150, &[left, right]).unwrap();
        let placed = Rect::from_point_size(origin, SIZE);
        for space in [left, right] {
            assert!(!space.intersects(placed.inset(-DEFAULT_SPACE_GAP)));
        }
    }

    #[test]
    fn a_full_work_area_has_no_spot_for_a_drop() {
        assert_eq!(drop_at(500, 400, &[BOUNDS]), None);
        let tiny = Rect::new(0, 0, 200, 100);
        assert_eq!(
            drop_space_origin(Point::new(0, 0), &[], SIZE, tiny, DEFAULT_SPACE_GAP),
            None
        );
    }
}
