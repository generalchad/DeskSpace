//! Burn's pure math (ROADMAP D12, amended 2026-10-18): a space burns up like
//! a sheet of paper. No Win32 here, so the painter (the paper) and the flame
//! overlay (the flames) share it and the tests cover both.
//!
//! A `BurnField` gives every cell of the space a value from 0 to 1: smooth
//! noise plus a bias toward the bottom, so the paper burns from the bottom up
//! along a ragged front. A frame's `front` says how far the burn has got;
//! each cell's distance above it picks its `Stage`, from gone through the
//! glowing edge, char, and scorch, to untouched paper.

use std::ops::Range;

use deskspace_core::animation::DETAIL_RANGE;

use super::math::Grid;
use super::noise::RisingField;

/// Pixels per field cell; the painter draws the cells with linear
/// interpolation, so the edges stay soft.
pub const CELL: u32 = 3;
/// How far above the front, in field units, each band reaches.
pub const GLOW: f32 = 0.035;
pub const CHAR: f32 = 0.045;
pub const SCORCH: f32 = 0.07;
/// Everything the burn touches beyond what is already gone.
pub const BANDS: f32 = GLOW + CHAR + SCORCH;
/// The distances above the front where a cell's `Stage` changes: a front
/// that does not pass one of these changes nothing.
const EDGES: [f32; 4] = [0.0, GLOW, GLOW + CHAR, BANDS];

/// How much of a cell's value is noise; the rest burns the bottom first.
const NOISE_SHARE: f32 = 0.55;
/// The noise's coarsest lattice, in cells, at the finest and per step of
/// detail.
const LATTICE_BASE: f32 = 6.0;
const LATTICE_PER_DETAIL: f32 = 3.0;

/// The burn's progress as a front in field units: at `gone` 0 nothing is
/// touched, at 1 everything, the bands included, has burned away.
pub fn front(gone: f32) -> f32 {
    gone * (1.0 + BANDS) - BANDS
}

/// The cells that cover a space of `size` pixels.
fn grid_for(size: (f32, f32)) -> Grid {
    Grid::covering(size, CELL)
}

/// The burn field of a space, one value per cell.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnField {
    field: RisingField,
    /// Each row's lowest and highest cell value, which say what a moving
    /// front can have changed without reading every cell.
    extremes: Vec<(f32, f32)>,
}

impl BurnField {
    /// The field for a space of `size` pixels; `detail` (`DETAIL_RANGE`)
    /// coarsens the ragged front and `seed` varies its shape. The painter
    /// and the flames each make one from the same play, so the flames stand
    /// on the edge the paper burns.
    pub fn for_space(size: (f32, f32), detail: u8, seed: u32) -> Self {
        Self::new(grid_for(size), detail, seed)
    }

    fn new(grid: Grid, detail: u8, seed: u32) -> Self {
        let detail = detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1);
        let lattice = LATTICE_BASE + LATTICE_PER_DETAIL * f32::from(detail);
        let field = RisingField::new(grid.cols, grid.rows, lattice, NOISE_SHARE, seed);
        let extremes = field
            .values()
            .chunks(grid.cols.max(1) as usize)
            .map(|row| {
                row.iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), &value| {
                        (low.min(value), high.max(value))
                    })
            })
            .collect();
        Self { field, extremes }
    }

    pub fn grid(&self) -> Grid {
        Grid {
            cols: self.field.cols(),
            rows: self.field.rows(),
        }
    }

    pub fn value(&self, col: u32, row: u32) -> f32 {
        self.field.value(col, row)
    }

    /// Writes the pixels of `rows` for a burn at `front`: into `paper`,
    /// whether the paper still shows, and into `marks`, the glow, char, or
    /// scorch over it (`cell_pixels`). Every other row keeps what it had.
    pub fn fill_rows(&self, front: f32, rows: Range<u32>, paper: &mut [u32], marks: &mut [u32]) {
        let cols = self.field.cols().max(1) as usize;
        let values = self.field.values();
        let start = (rows.start as usize * cols).min(values.len());
        let end = (rows.end as usize * cols)
            .min(values.len())
            .min(paper.len())
            .min(marks.len());
        if start >= end {
            return;
        }
        fill_pixels(
            &values[start..end],
            front,
            &mut paper[start..end],
            &mut marks[start..end],
        );
    }

    /// The rows a front moving from `previous` to `front` can have changed,
    /// so a painter writes and uploads only those. A cell's `Stage` changes
    /// only when its value passes one of `EDGES` as the front moves, so a row
    /// with no value within the move of an edge stays as it was. `None` when
    /// the front did not move.
    pub fn rows_changed(&self, previous: f32, front: f32) -> Option<Range<u32>> {
        if previous == front {
            return None;
        }
        let (low, high) = (previous.min(front), previous.max(front));
        self.rows_where(|min, max| {
            EDGES
                .iter()
                .any(|edge| max >= low + edge && min < high + edge)
        })
        // `stage` compares `value - front` in `f32`, and that subtraction can
        // land a cell an edge's width the other side of one than the values
        // predict; a row each side costs little and keeps it from going stale.
        .map(|rows| rows.start.saturating_sub(1)..(rows.end + 1).min(self.field.rows()))
    }

    /// The rows a burn at `front` marks at all: where a cell is burning,
    /// charred, or scorched. Empty when it marks none, at the start and the
    /// end of the burn.
    pub fn rows_marked(&self, front: f32) -> Range<u32> {
        self.rows_where(|min, max| max >= front && min < front + BANDS)
            .unwrap_or(0..0)
    }

    /// The first and last rows (as a run, `None` when there are none) whose
    /// lowest and highest values satisfy `holds`.
    fn rows_where(&self, holds: impl Fn(f32, f32) -> bool) -> Option<Range<u32>> {
        let mut run: Option<Range<u32>> = None;
        for (row, &(min, max)) in self.extremes.iter().enumerate() {
            if !holds(min, max) {
                continue;
            }
            let row = row as u32;
            run = Some(match run {
                Some(run) => run.start..row + 1,
                None => row..row + 1,
            });
        }
        run
    }

    /// Where flames stand at `front`, into `bases`: in every `every`-th
    /// column and the last, the top of the glowing band, with how hot it is
    /// there (1 right at the front).
    pub fn flame_bases(&self, front: f32, every: u32, bases: &mut Vec<FlameBase>) {
        bases.clear();
        let (cols, rows) = (self.field.cols(), self.field.rows());
        let last = cols.saturating_sub(1);
        let columns = (0..cols)
            .step_by(every.max(1) as usize)
            .chain((!last.is_multiple_of(every.max(1))).then_some(last));
        for col in columns {
            let base = (0..rows).find_map(|row| match stage(self.value(col, row) - front) {
                Stage::Burning(along) => Some(FlameBase {
                    col,
                    row,
                    heat: 1.0 - along,
                }),
                _ => None,
            });
            bases.extend(base);
        }
    }
}

/// A point on the burning edge that a flame rises from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlameBase {
    pub col: u32,
    pub row: u32,
    /// 0 to 1.
    pub heat: f32,
}

/// What a cell is at a distance `above` the front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Gone,
    /// The glowing edge; 0 at the front, 1 where it meets the char.
    Burning(f32),
    Charred,
    /// Browning ahead of the char; 1 next to it, 0 at untouched paper.
    Scorched(f32),
    Whole,
}

pub fn stage(above: f32) -> Stage {
    if above < 0.0 {
        Stage::Gone
    } else if above < GLOW {
        Stage::Burning(above / GLOW)
    } else if above < GLOW + CHAR {
        Stage::Charred
    } else if above < BANDS {
        Stage::Scorched(1.0 - (above - GLOW - CHAR) / SCORCH)
    } else {
        Stage::Whole
    }
}

/// A cell's two pixels: whether the paper (the space's contents) still shows
/// there, as an opacity mask's premultiplied pixel, and the color laid over
/// it, premultiplied BGRA (`0xAARRGGBB`).
pub fn cell_pixels(stage: Stage) -> (u32, u32) {
    const SHOWN: u32 = u32::MAX;
    match stage {
        Stage::Gone => (0, 0),
        // White-hot at the front, through orange to a deep red glow.
        Stage::Burning(along) => {
            let color = mix((255, 244, 190), (214, 64, 12), along);
            (SHOWN, premultiplied(color, 255))
        }
        Stage::Charred => (SHOWN, premultiplied((28, 17, 11), 238)),
        Stage::Scorched(near) => {
            let alpha = (near * 170.0).round() as u8;
            (SHOWN, premultiplied((112, 62, 22), alpha))
        }
        Stage::Whole => (SHOWN, 0),
    }
}

fn fill_pixels(values: &[f32], front: f32, paper: &mut [u32], marks: &mut [u32]) {
    for ((value, paper), marks) in values.iter().zip(paper.iter_mut()).zip(marks.iter_mut()) {
        (*paper, *marks) = cell_pixels(stage(value - front));
    }
}

fn mix(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

fn premultiplied((r, g, b): (u8, u8, u8), alpha: u8) -> u32 {
    let scale = |channel: u8| (u32::from(channel) * u32::from(alpha) + 127) / 255;
    (u32::from(alpha) << 24) | (scale(r) << 16) | (scale(g) << 8) | scale(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flames_stand_on_the_burning_edge() {
        let field = BurnField::new(Grid { cols: 60, rows: 40 }, 5, 0);
        let mut bases = Vec::new();
        field.flame_bases(0.4, 4, &mut bases);
        assert!(!bases.is_empty());
        for base in &bases {
            let above = field.value(base.col, base.row) - 0.4;
            assert!((0.0..GLOW).contains(&above));
            assert!((0.0..=1.0).contains(&base.heat));
        }
        field.flame_bases(-1.0, 4, &mut bases);
        assert!(bases.is_empty(), "nothing burns yet");
    }

    #[test]
    fn the_field_stays_in_range_and_burns_the_bottom_first() {
        let field = BurnField::new(Grid { cols: 80, rows: 60 }, 5, 0);
        assert!(
            field
                .field
                .values()
                .iter()
                .all(|value| (0.0..=1.0).contains(value))
        );
        let row_mean = |row: u32| {
            (0..field.grid().cols)
                .map(|col| field.value(col, row))
                .sum::<f32>()
                / 80.0
        };
        assert!(row_mean(59) < row_mean(30) && row_mean(30) < row_mean(0));
    }

    #[test]
    fn the_front_is_ragged() {
        let field = BurnField::new(Grid { cols: 80, rows: 60 }, 5, 0);
        let row = 30;
        let (low, high) = (0..80)
            .map(|col| field.value(col, row))
            .fold((1.0f32, 0.0f32), |(low, high), value| {
                (low.min(value), high.max(value))
            });
        assert!(
            high - low > 0.1,
            "a straight line would not look like paper"
        );
    }

    #[test]
    fn the_front_runs_from_untouched_to_all_gone() {
        let field = BurnField::new(Grid { cols: 40, rows: 30 }, 5, 0);
        let at = |gone| {
            field
                .field
                .values()
                .iter()
                .map(move |value| stage(value - front(gone)))
        };
        assert!(at(0.0).all(|stage| stage == Stage::Whole));
        assert!(at(1.0).all(|stage| stage == Stage::Gone));
    }

    #[test]
    fn the_bands_run_glow_char_scorch() {
        assert_eq!(stage(-0.01), Stage::Gone);
        assert_eq!(stage(0.0), Stage::Burning(0.0));
        assert_eq!(stage(GLOW + 0.001), Stage::Charred);
        assert!(matches!(stage(GLOW + CHAR + 0.001), Stage::Scorched(near) if near > 0.9));
        assert_eq!(stage(BANDS), Stage::Whole);
        let (mask, color) = cell_pixels(Stage::Gone);
        assert_eq!((mask, color), (0, 0));
        let (mask, color) = cell_pixels(Stage::Whole);
        assert_eq!((mask, color), (u32::MAX, 0));
        let (_, glow) = cell_pixels(Stage::Burning(0.0));
        assert_eq!(glow >> 24, 255, "the edge is opaque fire");
    }

    #[test]
    fn a_burn_writes_both_pixels_of_every_cell() {
        let values = [0.0, 0.5, 1.0];
        let (mut paper, mut marks) = ([7; 3], [7; 3]);
        fill_pixels(&values, 0.5, &mut paper, &mut marks);
        assert_eq!(paper[0], 0, "below the front is gone");
        assert_eq!(paper[1], u32::MAX, "the front itself still burns");
        assert_eq!(marks[1] >> 24, 255, "and glows");
        assert_eq!(
            (paper[2], marks[2]),
            (u32::MAX, 0),
            "far above is untouched"
        );
    }

    #[test]
    fn a_moving_front_touches_every_row_whose_cells_it_changes() {
        let (cols, rows) = (80, 60);
        let field = BurnField::new(Grid { cols, rows }, 5, 0);
        let mut previous = front(0.0);
        let mut moved = 0;
        for step in 1..=40 {
            let current = front(step as f32 / 40.0);
            let changed_rows = field.rows_changed(previous, current);
            moved += usize::from(changed_rows.is_some());
            for row in 0..rows {
                let changed = (0..cols).any(|col| {
                    let value = field.value(col, row);
                    stage(value - previous) != stage(value - current)
                });
                assert!(
                    !changed
                        || changed_rows
                            .clone()
                            .is_some_and(|changed| changed.contains(&row)),
                    "step {step}: row {row} changed outside {changed_rows:?}"
                );
            }
            previous = current;
        }
        assert!(
            moved > 20,
            "the ragged field changes on most steps, not {moved}"
        );
        assert_eq!(
            field.rows_changed(0.25, 0.25),
            None,
            "a still front changes nothing"
        );
    }

    #[test]
    fn the_marked_rows_hold_every_marked_cell() {
        let (cols, rows) = (60, 40);
        let field = BurnField::new(Grid { cols, rows }, 5, 0);
        for step in 0..=40 {
            let front = front(step as f32 / 40.0);
            let marked_rows = field.rows_marked(front);
            for row in 0..rows {
                let marked = (0..cols).any(|col| {
                    !matches!(
                        stage(field.value(col, row) - front),
                        Stage::Whole | Stage::Gone
                    )
                });
                assert!(
                    !marked || marked_rows.contains(&row),
                    "step {step}: row {row} is marked outside {marked_rows:?}"
                );
            }
        }
        assert!(
            field.rows_marked(front(0.0)).is_empty(),
            "nothing is marked before the burn"
        );
        assert!(
            field.rows_marked(front(1.0)).is_empty(),
            "nothing is marked once it has burned through"
        );
    }

    #[test]
    fn fill_rows_leaves_the_rows_it_is_not_given() {
        let (cols, rows) = (10, 4);
        let field = BurnField::new(Grid { cols, rows }, 5, 0);
        let (mut paper, mut marks) = ([7u32; 40], [7u32; 40]);
        field.fill_rows(0.5, 1..3, &mut paper, &mut marks);

        let (mut all_paper, mut all_marks) = ([7u32; 40], [7u32; 40]);
        for row in 0..rows {
            field.fill_rows(0.5, row..row + 1, &mut all_paper, &mut all_marks);
        }
        let written = cols as usize..3 * cols as usize;
        assert_eq!(paper[written.clone()], all_paper[written.clone()]);
        assert_eq!(marks[written.clone()], all_marks[written]);
        assert!(paper[..cols as usize].iter().all(|&pixel| pixel == 7));
        assert!(marks[3 * cols as usize..].iter().all(|&pixel| pixel == 7));
    }

    #[test]
    fn the_field_covers_the_space() {
        let field = BurnField::for_space((301.0, 199.0), 5, 1);
        assert_eq!(
            field.grid(),
            Grid {
                cols: 101,
                rows: 67
            }
        );
        assert_eq!(field.grid(), Grid::covering((301.0, 199.0), CELL));
        assert_eq!(field.field.values().len(), field.grid().cell_count());
    }

    #[test]
    fn the_seed_varies_the_field_and_repeats() {
        assert_eq!(
            BurnField::for_space((200.0, 160.0), 5, 7),
            BurnField::for_space((200.0, 160.0), 5, 7)
        );
        assert_ne!(
            BurnField::for_space((200.0, 160.0), 5, 1),
            BurnField::for_space((200.0, 160.0), 5, 2)
        );
    }

    #[test]
    fn the_field_values_are_pinned() {
        let field = BurnField::new(Grid { cols: 60, rows: 40 }, 5, 0);
        assert_eq!(
            (
                field.value(0, 0).to_bits(),
                field.value(30, 20).to_bits(),
                field.value(59, 39).to_bits(),
            ),
            (0x3f19_85c6, 0x3eae_8ac5, 0x3e35_7780)
        );
    }
}
