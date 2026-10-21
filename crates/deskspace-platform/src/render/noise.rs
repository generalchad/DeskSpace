//! Smooth fractal value noise, shared by the effects that tear a space
//! along a ragged front (Burn today). Pure and seeded by an integer hash,
//! so an effect always takes the same shape and a reversed one retraces it.

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
    /// `lattice` is the noise's coarsest grid in cells, and `noise_share`
    /// how much of each value is noise rather than height.
    pub fn new(cols: u32, rows: u32, lattice: f32, noise_share: f32) -> Self {
        let rows_f = rows.max(1) as f32;
        let values = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (col, row)))
            .map(|(col, row)| {
                let noise = fractal(col as f32, row as f32, lattice);
                let from_bottom = 1.0 - (row as f32 + 0.5) / rows_f;
                noise_share * noise + (1.0 - noise_share) * from_bottom
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

    pub fn value(&self, col: u32, row: u32) -> f32 {
        self.values[(row * self.cols + col) as usize]
    }
}

/// Fractal value noise at (`x`, `y`): `OCTAVES` of smooth noise, the first
/// on a `lattice`-sized grid and each next on half of it; 0 to 1.
pub fn fractal(x: f32, y: f32, lattice: f32) -> f32 {
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

/// A lattice point's fixed random value, 0 to 1.
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

    #[test]
    fn the_field_stays_in_range_and_rises_from_the_bottom() {
        let field = RisingField::new(80, 60, 21.0, 0.55);
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
        let at = fractal(12.3, 4.5, 8.0);
        assert!((0.0..=1.0).contains(&at));
        assert_eq!(at, fractal(12.3, 4.5, 8.0));
    }
}
