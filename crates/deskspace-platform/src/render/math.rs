//! Small pure helpers the frame math shares: interpolation, easing, and a
//! fixed hash for noise.

pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Smoothstep: 0 at 0 and 1 at 1, slow at both ends, and symmetric, so
/// played backwards it traces the same curve.
pub fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// A fixed, well-spread hash of three integers, so noise built on it takes
/// the same shape every time and an effect played backwards retraces it.
/// The three multipliers are xxHash32's primes
/// (https://github.com/Cyan4973/xxHash/blob/dev/doc/xxhash_spec.md).
pub fn hash(x: u32, y: u32, z: u32) -> u32 {
    let mut hash =
        x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ z.wrapping_mul(0xC2B2_AE3D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    hash
}

/// `hash`'s top 24 bits as a value from 0 up to, but not including, 1.
pub fn unit(hash: u32) -> f32 {
    (hash >> 8) as f32 / (1u32 << 24) as f32
}

/// Square cells covering a space, the last row and column partly outside
/// it: the grid a cell effect (Dissolve, Burn) works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
}

impl Grid {
    /// The cells `cell` pixels square that cover a space of `size`, at
    /// least one each way.
    pub fn covering((width, height): (f32, f32), cell: u32) -> Self {
        let count = |length: f32| (length / cell.max(1) as f32).ceil().max(1.0) as u32;
        Self {
            cols: count(width),
            rows: count(height),
        }
    }

    pub fn cell_count(self) -> usize {
        self.cols as usize * self.rows as usize
    }

    /// Every cell's column and row, row by row.
    pub fn cells(self) -> impl Iterator<Item = (u32, u32)> {
        (0..self.rows).flat_map(move |row| (0..self.cols).map(move |col| (col, row)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_runs_from_0_to_1_and_is_symmetric() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(0.5), 0.5);
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            assert!((smoothstep(t) + smoothstep(1.0 - t) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_hash_is_fixed_and_unit_stays_below_1() {
        assert_eq!(hash(3, 7, 1), hash(3, 7, 1));
        assert_ne!(hash(3, 7, 1), hash(7, 3, 1));
        assert!((0.0..1.0).contains(&unit(u32::MAX)));
        assert_eq!(unit(0), 0.0);
    }

    #[test]
    fn a_grid_covers_the_whole_space() {
        let grid = Grid::covering((100.0, 31.0), 10);
        assert_eq!(grid, Grid { cols: 10, rows: 4 });
        assert_eq!(grid.cell_count(), 40);
        assert_eq!(grid.cells().nth(11), Some((1, 1)));
        assert_eq!(Grid::covering((0.0, 0.0), 3), Grid { cols: 1, rows: 1 });
    }
}
