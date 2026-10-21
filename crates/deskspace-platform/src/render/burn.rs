//! Burn's pure math (ROADMAP D12, amended 2026-10-18): a space burns up like
//! a sheet of paper. No Win32 here, so the painter (the paper) and the flame
//! overlay (the flames) share it and the tests cover both.
//!
//! A `BurnField` gives every cell of the space a value from 0 to 1: smooth
//! noise plus a bias toward the bottom, so the paper burns from the bottom up
//! along a ragged front. A frame's `front` says how far the burn has got;
//! each cell's distance above it picks its `Stage`, from gone through the
//! glowing edge, char, and scorch, to untouched paper.

use deskspace_core::animation::DETAIL_RANGE;

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

/// The burn field of a space, `cols` by `rows` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnField {
    field: RisingField,
}

impl BurnField {
    /// A field for a space of `cols` by `rows` cells; `detail`
    /// (`DETAIL_RANGE`) coarsens the ragged front.
    pub fn new(cols: u32, rows: u32, detail: u8) -> Self {
        let detail = detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1);
        let lattice = LATTICE_BASE + LATTICE_PER_DETAIL * f32::from(detail);
        Self {
            field: RisingField::new(cols, rows, lattice, NOISE_SHARE),
        }
    }

    pub fn cols(&self) -> u32 {
        self.field.cols()
    }

    pub fn rows(&self) -> u32 {
        self.field.rows()
    }

    pub fn values(&self) -> &[f32] {
        self.field.values()
    }

    pub fn value(&self, col: u32, row: u32) -> f32 {
        self.field.value(col, row)
    }

    /// Where flames stand at `front`, into `bases`: in every `every`-th
    /// column and the last, the top of the glowing band, with how hot it is
    /// there (1 right at the front).
    pub fn flame_bases(&self, front: f32, every: u32, bases: &mut Vec<FlameBase>) {
        bases.clear();
        let last = self.cols().saturating_sub(1);
        let columns = (0..self.cols())
            .step_by(every.max(1) as usize)
            .chain((!last.is_multiple_of(every.max(1))).then_some(last));
        for col in columns {
            let base = (0..self.rows()).find_map(|row| match stage(self.value(col, row) - front) {
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

    fn value(field: &BurnField, col: u32, row: u32) -> f32 {
        field.value(col, row)
    }

    #[test]
    fn flames_stand_on_the_burning_edge() {
        let field = BurnField::new(60, 40, 5);
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
        let field = BurnField::new(80, 60, 5);
        assert!(
            field
                .values()
                .iter()
                .all(|value| (0.0..=1.0).contains(value))
        );
        let row_mean = |row: u32| {
            (0..field.cols())
                .map(|col| value(&field, col, row))
                .sum::<f32>()
                / 80.0
        };
        assert!(row_mean(59) < row_mean(30) && row_mean(30) < row_mean(0));
    }

    #[test]
    fn the_front_is_ragged() {
        let field = BurnField::new(80, 60, 5);
        let row = 30;
        let (low, high) = (0..80)
            .map(|col| value(&field, col, row))
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
        let field = BurnField::new(40, 30, 5);
        let at = |gone| {
            field
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
}
