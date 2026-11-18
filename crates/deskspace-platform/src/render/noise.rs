//! Smooth fractal value noise, shared by the effects that tear a space
//! along a ragged front (Burn today). Pure and seeded by a lattice hash, so
//! a burn always takes the same shape and a reversed one retraces it.

use super::math::{smoothstep, unit};

/// The octaves: each halves the lattice, with these weights (summing to 1).
const OCTAVES: [f32; 3] = [0.55, 0.3, 0.15];

/// A value from 0 to 1 for every cell of a space, `cols` by `rows`: smooth
/// noise plus a bias toward the bottom, so a front sweeping through the
/// values from 0 to 1 takes the bottom first, along a ragged edge.
#[derive(Debug, Clone, PartialEq)]
pub struct RisingField {
    cols: u32,
    rows: u32,
    values: Vec<f32>,
}

impl RisingField {
    /// `lattice` is the noise's coarsest grid in cells, `noise_share` how much
    /// of each value is noise rather than height, and `seed` varies the noise.
    #[cfg(test)]
    pub fn new(cols: u32, rows: u32, lattice: f32, noise_share: f32, seed: u32) -> Self {
        let rows_f = rows.max(1) as f32;
        let values = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (col, row)))
            .map(|(col, row)| {
                let noise = fractal(col as f32, row as f32, lattice, seed);
                let from_bottom = 1.0 - (row as f32 + 0.5) / rows_f;
                noise_share * noise + (1.0 - noise_share) * from_bottom
            })
            .collect();
        Self { cols, rows, values }
    }

    pub(crate) fn from_values(cols: u32, rows: u32, values: Vec<f32>) -> Self {
        debug_assert_eq!(values.len(), cols as usize * rows as usize);
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

    pub fn value(&self, col: u32, row: u32) -> f32 {
        self.values[(row * self.cols + col) as usize]
    }

    /// The field at a point in cells, bilinear and clamped to the edges, so
    /// sampling just outside still sees the edge.
    #[cfg(test)]
    pub fn value_at(&self, x: f32, y: f32) -> f32 {
        let max_col = self.cols.saturating_sub(1);
        let max_row = self.rows.saturating_sub(1);
        let fx = x.clamp(0.0, max_col as f32);
        let fy = y.clamp(0.0, max_row as f32);
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(max_col), (y0 + 1).min(max_row));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let at = |col, row| self.value(col, row);
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * tx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * tx;
        top + (bottom - top) * ty
    }
}

/// A fixed random value from 0 to 1 for (`seed`, `index`), so a play's parts
/// (its sources, its offsets) can be seeded once and stay put.
pub fn random(seed: u32, index: u32) -> f32 {
    lattice_value(seed as i32, index as i32, 0x5EED, 0)
}

/// Fractal value noise at cell (`x`, `y`): `OCTAVES` of smooth noise, the
/// first on a `lattice`-cell grid and each next on half of it; 0 to 1.
#[cfg(test)]
pub fn fractal(x: f32, y: f32, lattice: f32, seed: u32) -> f32 {
    OCTAVES
        .iter()
        .enumerate()
        .map(|(octave, weight)| {
            let size = lattice / (1 << octave) as f32;
            weight * value_noise(x / size, y / size, octave as u32, seed)
        })
        .sum()
}

/// Adds `weight * value_noise(col * x_step / size, y / size, octave, seed)`
/// into each element of `out`, reusing row-constant `y` terms and cached
/// lattice columns across adjacent pixels.
pub(crate) fn add_octave_row(
    y: f32,
    x_step: f32,
    size: f32,
    octave: u32,
    weight: f32,
    seed: u32,
    out: &mut [f32],
) {
    let inv_size = 1.0 / size;
    let sy = y * inv_size;
    let y0 = sy.floor();
    let ty = smoothstep(sy - y0);
    let iy = y0 as i32;
    let vertical = |ix: i32| {
        let top = lattice_value(ix, iy, octave, seed);
        let bottom = lattice_value(ix, iy + 1, octave, seed);
        top + (bottom - top) * ty
    };
    let mut cached_ix = i32::MIN;
    let mut v0 = 0.0;
    let mut v1 = 0.0;
    let step = x_step * inv_size;
    let mut sx = 0.0f32;
    for dst in out.iter_mut() {
        // `sx >= 0.0` because `col`, `x_step`, and `size` are non-negative,
        // so truncation matches `floor` without a round instruction.
        let ix = sx as i32;
        if ix != cached_ix {
            if ix == cached_ix + 1 {
                v0 = v1;
                v1 = vertical(ix + 1);
            } else {
                v0 = vertical(ix);
                v1 = vertical(ix + 1);
            }
            cached_ix = ix;
        }
        let tx = smoothstep(sx - ix as f32);
        *dst += weight * (v0 + (v1 - v0) * tx);
        sx += step;
    }
}

/// Fills `out` with `fractal(col as f32 * x_step, y, lattice, seed)` for each
/// `col` in `0..out.len()`.
pub(crate) fn fractal_row(y: f32, x_step: f32, lattice: f32, seed: u32, out: &mut [f32]) {
    out.fill(0.0);
    for (octave, &weight) in OCTAVES.iter().enumerate() {
        let size = lattice / (1 << octave) as f32;
        add_octave_row(y, x_step, size, octave as u32, weight, seed, out);
    }
}

/// Smooth noise: random values on an integer lattice, blended with
/// smoothstep weights; 0 to 1.
#[cfg(test)]
fn value_noise(x: f32, y: f32, octave: u32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (smoothstep(x - x0), smoothstep(y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let corner = |dx: i32, dy: i32| lattice_value(ix + dx, iy + dy, octave, seed);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
    top + (bottom - top) * ty
}

/// A lattice point's fixed random value, 0 to 1, so a burn always takes the
/// same shape and a reversed one retraces it.
fn lattice_value(x: i32, y: i32, octave: u32, seed: u32) -> f32 {
    let mut hash = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ octave.wrapping_mul(0xC2B2_AE3D)
        ^ seed.wrapping_mul(0x27D4_EB2F);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    unit(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_stays_in_range_and_rises_from_the_bottom() {
        let field = RisingField::new(80, 60, 21.0, 0.55, 0);
        assert_eq!((field.cols(), field.rows()), (80, 60));
        assert!(
            field
                .values()
                .iter()
                .all(|value| (0.0..=1.0).contains(value))
        );
        let row_mean = |row: u32| (0..80).map(|col| field.value(col, row)).sum::<f32>() / 80.0;
        assert!(row_mean(59) < row_mean(30) && row_mean(30) < row_mean(0));
    }

    #[test]
    fn noise_is_fixed_and_in_range() {
        let at = fractal(12.3, 4.5, 8.0, 0);
        assert!((0.0..=1.0).contains(&at));
        assert_eq!(at, fractal(12.3, 4.5, 8.0, 0));
        assert_ne!(at, fractal(12.3, 4.5, 8.0, 1));
    }

    #[test]
    fn seed_zero_keeps_todays_values() {
        let field = RisingField::new(50, 40, 21.0, 0.55, 0);
        assert_eq!(
            (
                fractal(12.3, 4.5, 8.0, 0).to_bits(),
                field.value(0, 0).to_bits(),
                field.value(25, 20).to_bits(),
            ),
            (1_056_006_717, 1_058_637_254, 1_051_498_394)
        );
        assert_ne!(field, RisingField::new(50, 40, 21.0, 0.55, 1));
    }

    #[test]
    fn random_is_fixed_and_varies_with_seed_and_index() {
        let value = random(7, 3);
        assert!((0.0..1.0).contains(&value));
        assert_eq!(value, random(7, 3));
        assert_ne!(value, random(8, 3));
        assert_ne!(value, random(7, 4));
    }

    #[test]
    fn value_at_interpolates_and_clamps_to_the_edges() {
        let field = RisingField::from_values(2, 2, vec![0.0, 1.0, 0.5, 0.5]);
        assert_eq!(field.value_at(0.0, 0.0), 0.0);
        assert_eq!(field.value_at(1.0, 0.0), 1.0);
        assert!((field.value_at(0.5, 0.0) - 0.5).abs() < 1e-6);
        assert!((field.value_at(0.5, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(field.value_at(-5.0, -5.0), 0.0);
        assert_eq!(field.value_at(99.0, -5.0), 1.0);
        assert_eq!(field.value_at(-5.0, 99.0), 0.5);
    }

    #[test]
    fn fractal_row_matches_point_fractal() {
        let mut row = vec![0.0; 120];
        for (y, x_step) in [(0.0, 1.0), (13.7, 1.0), (12.0, 4.0)] {
            fractal_row(y, x_step, 21.0, 7, &mut row);
            for (col, &got) in row.iter().enumerate() {
                let expected = fractal(col as f32 * x_step, y, 21.0, 7);
                assert!(
                    (got - expected).abs() < 1e-4,
                    "at ({col}, {y}): got {got}, expected {expected}"
                );
            }
        }
    }
}
