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
/// The octaves: each halves the lattice, with these weights (summing to 1).
const OCTAVES: [f32; 3] = [0.55, 0.3, 0.15];

/// The burn's progress as a front in field units: at `gone` 0 nothing is
/// touched, at 1 everything, the bands included, has burned away.
pub fn front(gone: f32) -> f32 {
    gone * (1.0 + BANDS) - BANDS
}

/// The burn field of a space, `cols` by `rows` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnField {
    cols: u32,
    rows: u32,
    values: Vec<f32>,
}

impl BurnField {
    /// A field for a space of `cols` by `rows` cells; `detail`
    /// (`DETAIL_RANGE`) coarsens the ragged front.
    pub fn new(cols: u32, rows: u32, detail: u8) -> Self {
        let detail = detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1);
        let lattice = LATTICE_BASE + LATTICE_PER_DETAIL * f32::from(detail);
        let rows_f = rows.max(1) as f32;
        let values = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (col, row)))
            .map(|(col, row)| {
                let noise = fractal(col as f32, row as f32, lattice);
                let from_bottom = 1.0 - (row as f32 + 0.5) / rows_f;
                NOISE_SHARE * noise + (1.0 - NOISE_SHARE) * from_bottom
            })
            .collect();
        Self { cols, rows, values }
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }
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

/// Fractal value noise at cell (`x`, `y`): `OCTAVES` of smooth noise, the
/// first on a `lattice`-cell grid and each next on half of it; 0 to 1.
fn fractal(x: f32, y: f32, lattice: f32) -> f32 {
    OCTAVES
        .iter()
        .enumerate()
        .map(|(octave, weight)| {
            let size = lattice / (1 << octave) as f32;
            weight * value_noise(x / size, y / size, octave as u32)
        })
        .sum()
}

/// Smooth noise: random values on an integer lattice, blended with
/// smoothstep weights; 0 to 1.
fn value_noise(x: f32, y: f32, octave: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let corner = |dx: i32, dy: i32| lattice_value(ix + dx, iy + dy, octave);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
    top + (bottom - top) * ty
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// A lattice point's fixed random value, 0 to 1: an integer hash, so a
/// burn always takes the same shape and a reversed one retraces it.
fn lattice_value(x: i32, y: i32, octave: u32) -> f32 {
    let mut hash = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ octave.wrapping_mul(0xC2B2_AE3D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    (hash >> 8) as f32 / (1u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(field: &BurnField, col: u32, row: u32) -> f32 {
        field.values()[(row * field.cols() + col) as usize]
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
