//! Dissolve's pure math (ROADMAP D12): a space disappears grain by grain,
//! each grain-sized cell at its own fixed moment. No Win32 here, so the
//! painter only uploads what this writes.

use super::math::{self, Grid};

/// A cell's side per step of detail, in pixels.
const CELL_PER_DETAIL: u32 = 2;

/// Every cell of a space with the noise value that says when it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct DissolveField {
    grid: Grid,
    cell: u32,
    values: Vec<u8>,
}

impl DissolveField {
    /// The field for a space of `size` pixels, with cells `grain`
    /// (`DETAIL_RANGE`) steps of detail wide.
    pub fn for_size(size: (f32, f32), grain: u8) -> Self {
        let cell = u32::from(grain.max(1)) * CELL_PER_DETAIL;
        let grid = Grid::covering(size, cell);
        let values = grid
            .cells()
            .map(|(col, row)| noise_value(col, row))
            .collect();
        Self { grid, cell, values }
    }

    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// A cell's side in pixels.
    pub fn cell(&self) -> u32 {
        self.cell
    }

    /// Writes each cell's mask pixel into `pixels`: opaque (premultiplied
    /// white) while its noise is at or above `threshold` (0 to 1), else
    /// transparent.
    pub fn fill_mask(&self, threshold: f32, pixels: &mut [u32]) {
        fill_mask(&self.values, threshold, pixels);
    }
}

/// A cell's fixed noise value, so the same cell always dissolves at the same
/// moment and a reversed dissolve retraces itself.
fn noise_value(col: u32, row: u32) -> u8 {
    (math::hash(col, row, 0) >> 24) as u8
}

fn fill_mask(values: &[u8], threshold: f32, pixels: &mut [u32]) {
    let cutoff = threshold * 256.0;
    for (pixel, value) in pixels.iter_mut().zip(values) {
        *pixel = if f32::from(*value) >= cutoff {
            u32::MAX
        } else {
            0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_fixed_and_spread_out() {
        assert_eq!(noise_value(3, 7), noise_value(3, 7));
        let values: Vec<u8> = (0..64)
            .flat_map(|row| (0..64).map(move |col| noise_value(col, row)))
            .collect();
        let low = values.iter().filter(|&&value| value < 128).count();
        // Roughly half below the middle, so a dissolve is even.
        assert!((1600..2500).contains(&low), "{low}");
    }

    #[test]
    fn the_noise_mask_shows_cells_at_or_above_the_threshold() {
        let values = [0, 64, 128, 255];
        let mut pixels = [7; 4];
        fill_mask(&values, 0.0, &mut pixels);
        assert_eq!(pixels, [u32::MAX; 4], "all shown at 0");
        fill_mask(&values, 0.5, &mut pixels);
        assert_eq!(pixels, [0, 0, u32::MAX, u32::MAX]);
        fill_mask(&values, 1.0, &mut pixels);
        assert_eq!(pixels, [0; 4], "all hidden at 1");
    }

    #[test]
    fn the_grain_sets_the_cell() {
        let field = DissolveField::for_size((100.0, 50.0), 5);
        assert_eq!(field.cell(), 10);
        assert_eq!(field.grid(), Grid { cols: 10, rows: 5 });
        assert_eq!(field.values.len(), field.grid().cell_count());
    }
}
