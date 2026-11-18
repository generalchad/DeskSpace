//! Burn's pure math (ROADMAP D12, amended 2026-10-18): a space burns up like
//! a sheet of paper. No Win32 here, so the painter (the paper) and the flame
//! overlay (the flames) share it and the tests cover both.
//!
//! A `BurnField` gives every cell of the space a value from 0 to 1: when a
//! seeded fire front solved on a coarse grid reaches it, with fine noise along
//! the edge, so the paper catches unevenly, climbs faster than it spreads, and
//! opens spot holes. A frame's `front` says how far the burn has got; each
//! cell's distance above it picks its `Stage`, from gone through the glowing
//! edge, char, and scorch, to untouched paper.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::ops::Range;
use std::time::Instant;

use deskspace_core::animation::DETAIL_RANGE;

use super::math::Grid;
use super::noise::{self, RisingField};

/// Pixels per field cell at 96 DPI; `cell_px` scales it for a display.
pub const CELL: u32 = 3;

/// Pixels per field cell at `dpi`: `CELL` scaled, rounded to a whole pixel
/// and at least 1. At 100 % the edge is the same three-pixel mesh; a 150 %
/// display gets five-pixel cells, so the burn looks the same, only bigger.
pub fn cell_px(dpi: u32) -> u32 {
    ((CELL * dpi.max(1) + 48) / 96).max(1)
}

/// How far above the front, in field units, each band of a field reaches.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bands {
    glow: f32,
    char: f32,
    scorch: f32,
    light: f32,
}

impl Bands {
    /// Default band widths in field units for band-stage unit tests.
    #[cfg(test)]
    const DEFAULT: Self = Self {
        glow: 0.035,
        char: 0.045,
        scorch: 0.07,
        light: 0.08,
    };

    fn from_pixels(cell_px: u32, dpi: u32, units_per_cell: f32) -> Self {
        let scale = dpi.max(1) as f32 / 96.0;
        let to_units = |px: f32| (px * scale) / cell_px.max(1) as f32 * units_per_cell;
        Self {
            glow: to_units(GLOW_PX),
            char: to_units(CHAR_PX),
            scorch: to_units(SCORCH_PX),
            light: to_units(LIGHT_PX),
        }
    }

    /// Everything the burn touches beyond what is already gone.
    fn total(self) -> f32 {
        self.glow + self.char + self.scorch + self.light
    }

    fn stage(self, above: f32, ember: impl FnOnce() -> bool) -> Stage {
        if above < 0.0 {
            Stage::Gone
        } else if above < self.glow {
            Stage::Burning(above / self.glow)
        } else if above < self.glow + self.char {
            Stage::Charred {
                along: (above - self.glow) / self.char,
                ember: ember(),
            }
        } else if above < self.glow + self.char + self.scorch {
            Stage::Scorched(1.0 - (above - self.glow - self.char) / self.scorch)
        } else if above < self.total() {
            Stage::Lit(1.0 - (above - self.glow - self.char - self.scorch) / self.light)
        } else {
            Stage::Whole
        }
    }
}

/// Coarse-grid stride in fine cells for Dijkstra's arrival solve.
const COARSE_STEP: u32 = 4;
/// Burn speed bounds across the paper.
const SPEED_MIN: f32 = 0.35;
const SPEED_MAX: f32 = 1.0;
/// Fire climbs faster toward row 0 than it spreads across or down.
const CLIMB_UP: f32 = 1.8;
const CLIMB_DOWN: f32 = 0.45;
/// Stagger across bottom-row ignition, as a fraction of coarse grid height.
const BOTTOM_STAGGER: f32 = 0.2;
/// Spot fires: at most `SPOTS_MAX`, one per `SPOT_AREA` coarse nodes, delayed
/// so holes open and join the main front on larger spaces.
const SPOT_AREA: u32 = 600;
const SPOTS_MAX: u32 = 3;
const SPOT_DELAY: f32 = 0.3;
const SPOT_SALT: u32 = 0x5007_F12E;
/// Fine raggedness added over the smooth coarse arrival field.
const EDGE_RAG: f32 = 0.015;
/// Band thicknesses in pixels at 96 DPI.
const GLOW_PX: f32 = 3.0;
const CHAR_PX: f32 = 5.0;
const SCORCH_PX: f32 = 12.0;
const LIGHT_PX: f32 = 24.0;
/// Seed salt and share for ember cells within the char band.
const EMBER_SALT: u32 = 0xE3BE_25A1;
const EMBER_SHARE: f32 = 0.1;
/// Flicker rates in rad/s for the live glow/paper light and char embers.
const BURNING_RATE: f32 = 13.0;
const EMBER_RATE: f32 = 5.0;
/// Seed salt and share for ash flakes lifting off burned cells.
const ASH_SALT: u32 = 0xA540_F1A3;
const ASH_SHARE: f32 = 0.008;

/// The noise's coarsest lattice, in cells, at the finest and per step of
/// detail.
const LATTICE_BASE: f32 = 6.0;
const LATTICE_PER_DETAIL: f32 = 3.0;

/// The cells that cover a space of `size` pixels at `dpi`.
fn grid_for(size: (f32, f32), dpi: u32) -> Grid {
    Grid::covering(size, cell_px(dpi))
}

#[derive(Debug, Clone, Copy)]
struct Cost(f32);

impl PartialEq for Cost {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0) == Ordering::Equal
    }
}

impl Eq for Cost {}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Time to step across `length` (1 or √2) into a cell with `speed` and `climb`.
fn step_cost(length: f32, speed: f32, climb: f32) -> f32 {
    length / (speed * climb)
}

/// Solves fire arrival times on a coarse grid using Dijkstra's algorithm:
/// https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm
fn solve_coarse(node_cols: u32, node_rows: u32, lattice: f32, seed: u32) -> (RisingField, f32) {
    let node_cols = node_cols.max(1);
    let node_rows = node_rows.max(1);
    let total = node_cols as usize * node_rows as usize;
    let mut speeds = vec![0.0; total];
    for nrow in 0..node_rows {
        let row_slice = &mut speeds
            [nrow as usize * node_cols as usize..(nrow as usize + 1) * node_cols as usize];
        noise::fractal_row(
            (nrow * COARSE_STEP) as f32,
            COARSE_STEP as f32,
            lattice,
            seed,
            row_slice,
        );
        for speed in row_slice {
            *speed = SPEED_MIN + (SPEED_MAX - SPEED_MIN) * *speed;
        }
    }

    let mut arrivals = vec![f32::INFINITY; total];
    let mut heap = BinaryHeap::with_capacity(total / 2);

    // Bottom row ignites across the edge with seeded stagger.
    let bottom_row = node_rows - 1;
    let stagger = BOTTOM_STAGGER * node_rows as f32;
    for col in 0..node_cols {
        let idx = (bottom_row * node_cols + col) as usize;
        let start = stagger * noise::random(seed, col);
        arrivals[idx] = start;
        heap.push(Reverse((Cost(start), idx as u32)));
    }

    // Spot fires in the lower 60 % of rows and middle 80 % of columns.
    let spots = ((total as u32) / SPOT_AREA).min(SPOTS_MAX);
    let row_start = ((node_rows as f32 * 0.4).floor() as u32).min(node_rows - 1);
    let row_span = (node_rows - row_start).max(1);
    let col_start = (node_cols as f32 * 0.1).floor() as u32;
    let col_end = ((node_cols as f32 * 0.9).ceil() as u32)
        .max(col_start + 1)
        .min(node_cols);
    let col_span = (col_end - col_start).max(1);
    let spot_start = SPOT_DELAY * node_rows as f32;
    for spot in 0..spots {
        let r_col = noise::random(seed ^ SPOT_SALT, spot * 2);
        let r_row = noise::random(seed ^ SPOT_SALT, spot * 2 + 1);
        let col = (col_start + (r_col * col_span as f32) as u32).min(node_cols - 1);
        let row = (row_start + (r_row * row_span as f32) as u32).min(node_rows - 1);
        let idx = (row * node_cols + col) as usize;
        if spot_start < arrivals[idx] {
            arrivals[idx] = spot_start;
            heap.push(Reverse((Cost(spot_start), idx as u32)));
        }
    }

    const NEIGHBORS: [(i32, i32, f32, f32); 8] = [
        (0, -1, 1.0, CLIMB_UP),
        (-1, -1, std::f32::consts::SQRT_2, CLIMB_UP),
        (1, -1, std::f32::consts::SQRT_2, CLIMB_UP),
        (-1, 0, 1.0, 1.0),
        (1, 0, 1.0, 1.0),
        (0, 1, 1.0, CLIMB_DOWN),
        (-1, 1, std::f32::consts::SQRT_2, CLIMB_DOWN),
        (1, 1, std::f32::consts::SQRT_2, CLIMB_DOWN),
    ];

    let cols_i = node_cols as i32;
    let rows_i = node_rows as i32;
    while let Some(Reverse((Cost(time), idx))) = heap.pop() {
        let idx_usize = idx as usize;
        if time > arrivals[idx_usize] {
            continue;
        }
        let col = (idx % node_cols) as i32;
        let row = (idx / node_cols) as i32;
        for (dc, dr, length, climb) in NEIGHBORS {
            let nc = col + dc;
            let nr = row + dr;
            if nc < 0 || nc >= cols_i || nr < 0 || nr >= rows_i {
                continue;
            }
            let nidx = (nr as u32 * node_cols + nc as u32) as usize;
            let next = time + step_cost(length, speeds[nidx], climb);
            if next < arrivals[nidx] {
                arrivals[nidx] = next;
                heap.push(Reverse((Cost(next), nidx as u32)));
            }
        }
    }

    let max_arrival = arrivals.iter().copied().fold(0.0f32, f32::max).max(1e-4);
    (
        RisingField::from_values(node_cols, node_rows, arrivals),
        max_arrival,
    )
}

/// The burn field of a space, one value per cell.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnField {
    field: RisingField,
    bands: Bands,
    /// Display DPI the field was built for (`cell_px` and overlay margins).
    dpi: u32,
    /// One cell in normalized field units, from `1 / (max_arrival * COARSE_STEP)`.
    units_per_cell: f32,
    /// Seed for deterministic per-cell shimmer phase and ember selection.
    seed: u32,
    /// Cell indices (`row * cols + col`) selected to shed ash flakes.
    flakes: Vec<u32>,
    /// Each row's lowest and highest cell value, which say what a moving
    /// front can have changed without reading every cell.
    extremes: Vec<(f32, f32)>,
}

impl BurnField {
    /// The field for a space of `size` pixels at `dpi`; `detail`
    /// (`DETAIL_RANGE`) coarsens the ragged front and `seed` varies its
    /// shape. The painter and the flames each make one from the same play, so
    /// the flames stand on the edge the paper burns.
    pub fn for_space(size: (f32, f32), detail: u8, seed: u32, dpi: u32) -> Self {
        let started = Instant::now();
        let grid = grid_for(size, dpi);
        let field = Self::new(grid, detail, seed, dpi);
        tracing::debug!(
            elapsed_us = started.elapsed().as_micros() as u64,
            cols = grid.cols,
            rows = grid.rows,
            "built burn field"
        );
        field
    }

    fn new(grid: Grid, detail: u8, seed: u32, dpi: u32) -> Self {
        let detail = detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1);
        let lattice = LATTICE_BASE + LATTICE_PER_DETAIL * f32::from(detail);
        let node_cols = grid.cols.div_ceil(COARSE_STEP).max(1);
        let node_rows = grid.rows.div_ceil(COARSE_STEP).max(1);
        let (coarse, max_arrival) = solve_coarse(node_cols, node_rows, lattice, seed);
        let units_per_cell = 1.0 / (max_arrival * COARSE_STEP as f32);
        let inv_max = 1.0 / max_arrival;
        let inv_step = 1.0 / COARSE_STEP as f32;

        let mut values = vec![0.0; grid.cell_count()];
        let mut extremes = Vec::with_capacity(grid.rows as usize);
        let mut noise_row = vec![0.0; grid.cols as usize];
        let coarse_vals = coarse.values();
        let max_nc = node_cols - 1;
        let max_nr = node_rows - 1;
        // One fine octave over the coarse solve gives ragged paper detail
        // while keeping a 1920x1080 field build within the 3 ms budget.
        let fine_size = lattice * 0.25;
        for (row, out_row) in values.chunks_exact_mut(grid.cols as usize).enumerate() {
            let row = row as u32;
            noise_row.fill(0.0);
            noise::add_octave_row(row as f32, 1.0, fine_size, 2, 1.0, seed, &mut noise_row);
            let nr0 = (row / COARSE_STEP).min(max_nr);
            let nr1 = (nr0 + 1).min(max_nr);
            let ty = (row % COARSE_STEP) as f32 * inv_step;
            let r0 = &coarse_vals[nr0 as usize * node_cols as usize..];
            let r1 = &coarse_vals[nr1 as usize * node_cols as usize..];

            let mut low = f32::INFINITY;
            let mut high = f32::NEG_INFINITY;
            let step_u = COARSE_STEP as usize;
            for (nc0, (noise_chunk, dst_chunk)) in noise_row
                .chunks(step_u)
                .zip(out_row.chunks_mut(step_u))
                .enumerate()
            {
                let nc0 = nc0.min(max_nc as usize);
                let nc1 = (nc0 + 1).min(max_nc as usize);
                let v0 = (r0[nc0] + (r1[nc0] - r0[nc0]) * ty) * inv_max;
                let v1 = (r0[nc1] + (r1[nc1] - r0[nc1]) * ty) * inv_max;
                let dv = (v1 - v0) * inv_step;
                let mut arrival = v0;
                for (&rag_noise, dst) in noise_chunk.iter().zip(dst_chunk.iter_mut()) {
                    let rag = EDGE_RAG * (rag_noise - 0.5);
                    let value = (arrival + rag).clamp(0.0, 1.0);
                    low = low.min(value);
                    high = high.max(value);
                    *dst = value;
                    arrival += dv;
                }
            }
            extremes.push((low, high));
        }

        let mut flakes = Vec::with_capacity((values.len() as f32 * ASH_SHARE) as usize + 8);
        let ash_seed = seed ^ ASH_SALT;
        for index in 0..values.len() as u32 {
            if noise::random(ash_seed, index) < ASH_SHARE {
                flakes.push(index);
            }
        }
        let field = RisingField::from_values(grid.cols, grid.rows, values);
        let bands = Bands::from_pixels(cell_px(dpi), dpi, units_per_cell);
        Self {
            field,
            bands,
            dpi,
            units_per_cell,
            seed,
            flakes,
            extremes,
        }
    }

    pub(crate) fn dpi(&self) -> u32 {
        self.dpi
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    pub(crate) fn values(&self) -> &[f32] {
        self.field.values()
    }

    pub(crate) fn flake_cells(&self) -> &[u32] {
        &self.flakes
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

    /// The burn's progress as a front in this field's units: at `gone` 0
    /// nothing is touched, at 1 everything, the bands included, has burned
    /// away.
    pub fn front(&self, gone: f32) -> f32 {
        // Push slightly past 1.0 at gone=1 so a cell clamped to 1.0 crosses
        // below 0 (`Stage::Gone`) rather than staying at `Burning(0.0)`.
        const END_MARGIN: f32 = 1e-4;
        let bands = self.bands.total();
        gone * (1.0 + END_MARGIN + bands) - bands
    }

    /// What cell (`col`, `row`) is at `front`.
    pub fn stage(&self, col: u32, row: u32, front: f32) -> Stage {
        let index = row * self.field.cols() + col;
        self.bands
            .stage(self.value(col, row) - front, || is_ember(self.seed, index))
    }

    /// Writes the pixels of `rows` for a burn at `front` and animation clock
    /// `time`: into `paper`, whether the paper still shows, and into `marks`,
    /// the glow, char, scorch, or firelight over it (`cell_pixels`). Every
    /// other row keeps what it had.
    pub fn fill_rows(
        &self,
        front: f32,
        time: f32,
        rows: Range<u32>,
        paper: &mut [u32],
        marks: &mut [u32],
    ) {
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
            start as u32,
            RowFill {
                front,
                time,
                seed: self.seed,
                bands: self.bands,
            },
            &mut paper[start..end],
            &mut marks[start..end],
        );
    }

    /// The rows a front moving from `previous` to `front` can have changed,
    /// so a painter writes and uploads only those. A cell's `Stage` changes
    /// only when its value passes one of the band edges as the front moves,
    /// so a row with no value within the move of an edge stays as it was.
    /// `None` when the front did not move.
    pub fn rows_changed(&self, previous: f32, front: f32) -> Option<Range<u32>> {
        if previous == front {
            return None;
        }
        let (low, high) = (previous.min(front), previous.max(front));
        let bands = self.bands.total();
        self.rows_where(|min, max| max >= low && min < high + bands)
            // `stage` compares `value - front` in `f32`, and that subtraction can
            // land a cell an edge's width the other side of one than the values
            // predict; a row each side costs little and keeps it from going stale.
            .map(|rows| rows.start.saturating_sub(1)..(rows.end + 1).min(self.field.rows()))
    }

    /// The rows a burn at `front` marks at all: where a cell is burning,
    /// charred, scorched, or lit by the fire. Empty when it marks none, at the
    /// start and the end of the burn.
    pub fn rows_marked(&self, front: f32) -> Range<u32> {
        let bands = self.bands.total();
        self.rows_where(|min, max| max >= front && min < front + bands)
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
            let base = (0..rows).find_map(|row| match self.stage(col, row, front) {
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

/// What a cell is at a distance above the front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Gone,
    /// The glowing edge; 0 at the front, 1 where it meets the char.
    Burning(f32),
    /// Carbonized band ahead of the glow; `along` is 0 at the glow and 1 at
    /// the scorch, and `ember` marks glowing coal specks.
    Charred {
        along: f32,
        ember: bool,
    },
    /// Browning ahead of the char; 1 next to it, 0 at the light band.
    Scorched(f32),
    /// Warm light cast on untouched paper ahead of the scorch; 1 near scorch,
    /// 0 at untouched paper.
    Lit(f32),
    Whole,
}

/// A cell's two pixels: whether the paper (the space's contents) still shows
/// there, as an opacity mask's premultiplied pixel, and the color laid over
/// it, premultiplied BGRA (`0xAARRGGBB`). `shimmer` is in `0..=1`.
pub fn cell_pixels(stage: Stage, shimmer: f32) -> (u32, u32) {
    const SHOWN: u32 = u32::MAX;
    match stage {
        Stage::Gone => (0, 0),
        // White-hot at the front, through orange to a deep red glow, shimmering.
        Stage::Burning(along) => {
            let (r, g, b) = mix((255, 244, 190), (214, 64, 12), along);
            let factor = 0.82 + 0.18 * shimmer;
            let scale = |c: u8| (f32::from(c) * factor).round().clamp(0.0, 255.0) as u8;
            (SHOWN, premultiplied((scale(r), scale(g), scale(b)), 255))
        }
        Stage::Charred { along, ember } => {
            let char_px = premultiplied((28, 17, 11), 238);
            let marks = if ember {
                let fade = (1.0 - along).clamp(0.0, 1.0).powf(1.5);
                let alpha = (fade * (0.4 + 0.6 * shimmer) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                over(premultiplied((255, 110, 30), alpha), char_px)
            } else {
                char_px
            };
            (SHOWN, marks)
        }
        Stage::Scorched(near) => {
            let alpha = (near * 170.0).round() as u8;
            (SHOWN, premultiplied((112, 62, 22), alpha))
        }
        Stage::Lit(near) => {
            let near = near.clamp(0.0, 1.0);
            let alpha = (near * near * 48.0 * (0.75 + 0.25 * shimmer))
                .round()
                .clamp(0.0, 48.0) as u8;
            (SHOWN, premultiplied((255, 150, 60), alpha))
        }
        Stage::Whole => (SHOWN, 0),
    }
}

#[derive(Clone, Copy)]
struct RowFill {
    front: f32,
    time: f32,
    seed: u32,
    bands: Bands,
}

fn is_ember(seed: u32, index: u32) -> bool {
    noise::random(seed ^ EMBER_SALT, index) < EMBER_SHARE
}

fn cell_shimmer(seed: u32, index: u32, time: f32, rate: f32) -> f32 {
    let phase = noise::random(seed, index) * std::f32::consts::TAU;
    0.5 + 0.5 * (time * rate + phase).sin()
}

fn fill_pixels(
    values: &[f32],
    start_index: u32,
    fill: RowFill,
    paper: &mut [u32],
    marks: &mut [u32],
) {
    for (offset, ((&value, paper), marks)) in values
        .iter()
        .zip(paper.iter_mut())
        .zip(marks.iter_mut())
        .enumerate()
    {
        let index = start_index + offset as u32;
        let stage = fill
            .bands
            .stage(value - fill.front, || is_ember(fill.seed, index));
        let shimmer = match stage {
            Stage::Burning(_) | Stage::Lit(_) => {
                cell_shimmer(fill.seed, index, fill.time, BURNING_RATE)
            }
            Stage::Charred { ember: true, .. } => {
                cell_shimmer(fill.seed, index, fill.time, EMBER_RATE)
            }
            Stage::Gone
            | Stage::Charred { ember: false, .. }
            | Stage::Scorched(_)
            | Stage::Whole => 0.0,
        };
        (*paper, *marks) = cell_pixels(stage, shimmer);
    }
}

/// Premultiplied `src` over `dst` (`src + dst * (1 - src.alpha)`), BGRA.
fn over(src: u32, dst: u32) -> u32 {
    let inv_a = 255 - (src >> 24);
    let channel = |shift: u32| {
        let s = (src >> shift) & 0xff;
        let d = (dst >> shift) & 0xff;
        s + (d * inv_a + 127) / 255
    };
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
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
        let field = BurnField::new(Grid { cols: 60, rows: 40 }, 5, 0, 96);
        let mut bases = Vec::new();
        field.flame_bases(0.4, 4, &mut bases);
        assert!(!bases.is_empty());
        for base in &bases {
            let above = field.value(base.col, base.row) - 0.4;
            assert!((0.0..field.bands.glow).contains(&above));
            assert!((0.0..=1.0).contains(&base.heat));
        }
        field.flame_bases(-1.0, 4, &mut bases);
        assert!(bases.is_empty(), "nothing burns yet");
    }

    #[test]
    fn the_field_stays_in_range_and_burns_the_bottom_first() {
        let field = BurnField::new(Grid { cols: 80, rows: 60 }, 5, 0, 96);
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
        let field = BurnField::new(Grid { cols: 80, rows: 60 }, 5, 0, 96);
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
        let field = BurnField::new(Grid { cols: 40, rows: 30 }, 5, 0, 96);
        let all = |gone, expected| {
            let front = field.front(gone);
            (0..30).all(|row| (0..40).all(|col| field.stage(col, row, front) == expected))
        };
        assert!(all(0.0, Stage::Whole));
        assert!(all(1.0, Stage::Gone));
    }

    #[test]
    fn the_bands_run_glow_char_scorch_lit() {
        let bands = Bands::DEFAULT;
        assert_eq!(bands.stage(-0.01, || false), Stage::Gone);
        assert_eq!(bands.stage(0.0, || false), Stage::Burning(0.0));
        assert!(matches!(
            bands.stage(bands.glow + 0.001, || false),
            Stage::Charred { along, ember: false } if along < 0.1
        ));
        assert!(matches!(
            bands.stage(bands.glow + bands.char + 0.001, || false),
            Stage::Scorched(near) if near > 0.9
        ));
        assert!(matches!(
            bands.stage(bands.glow + bands.char + bands.scorch + 0.001, || false),
            Stage::Lit(near) if near > 0.9
        ));
        assert_eq!(bands.stage(bands.total(), || false), Stage::Whole);
        for shimmer in [0.0, 1.0] {
            assert_eq!(cell_pixels(Stage::Gone, shimmer), (0, 0));
            assert_eq!(cell_pixels(Stage::Whole, shimmer), (u32::MAX, 0));
        }
        let (_, glow) = cell_pixels(Stage::Burning(0.0), 1.0);
        assert_eq!(glow >> 24, 255, "the edge is opaque fire");
    }

    #[test]
    fn a_burn_writes_both_pixels_of_every_cell() {
        let values = [0.0, 0.5, 1.0];
        let (mut paper, mut marks) = ([7; 3], [7; 3]);
        let fill = RowFill {
            front: 0.5,
            time: 0.2,
            seed: 1,
            bands: Bands::DEFAULT,
        };
        fill_pixels(&values, 0, fill, &mut paper, &mut marks);
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
        let field = BurnField::new(Grid { cols, rows }, 5, 0, 96);
        let mut previous = field.front(0.0);
        let mut moved = 0;
        for step in 1..=40 {
            let current = field.front(step as f32 / 40.0);
            let changed_rows = field.rows_changed(previous, current);
            moved += usize::from(changed_rows.is_some());
            for row in 0..rows {
                let changed = (0..cols)
                    .any(|col| field.stage(col, row, previous) != field.stage(col, row, current));
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
        let field = BurnField::new(Grid { cols, rows }, 5, 0, 96);
        for step in 0..=40 {
            let front = field.front(step as f32 / 40.0);
            let marked_rows = field.rows_marked(front);
            for row in 0..rows {
                let marked = (0..cols)
                    .any(|col| !matches!(field.stage(col, row, front), Stage::Whole | Stage::Gone));
                assert!(
                    !marked || marked_rows.contains(&row),
                    "step {step}: row {row} is marked outside {marked_rows:?}"
                );
            }
        }
        assert!(
            field.rows_marked(field.front(0.0)).is_empty(),
            "nothing is marked before the burn"
        );
        assert!(
            field.rows_marked(field.front(1.0)).is_empty(),
            "nothing is marked once it has burned through"
        );
    }

    #[test]
    fn fill_rows_leaves_the_rows_it_is_not_given() {
        let (cols, rows) = (10, 4);
        let field = BurnField::new(Grid { cols, rows }, 5, 0, 96);
        let (mut paper, mut marks) = ([7u32; 40], [7u32; 40]);
        field.fill_rows(0.5, 0.4, 1..3, &mut paper, &mut marks);

        let (mut all_paper, mut all_marks) = ([7u32; 40], [7u32; 40]);
        for row in 0..rows {
            field.fill_rows(0.5, 0.4, row..row + 1, &mut all_paper, &mut all_marks);
        }
        let written = cols as usize..3 * cols as usize;
        assert_eq!(paper[written.clone()], all_paper[written.clone()]);
        assert_eq!(marks[written.clone()], all_marks[written]);
        assert!(paper[..cols as usize].iter().all(|&pixel| pixel == 7));
        assert!(marks[3 * cols as usize..].iter().all(|&pixel| pixel == 7));
    }

    #[test]
    fn the_field_covers_the_space() {
        let field = BurnField::for_space((301.0, 199.0), 5, 1, 96);
        assert_eq!(
            field.grid(),
            Grid {
                cols: 101,
                rows: 67
            }
        );
        assert_eq!(field.grid(), Grid::covering((301.0, 199.0), cell_px(96)));
        assert_eq!(field.field.values().len(), field.grid().cell_count());
    }

    #[test]
    fn the_field_coarsens_with_the_displays_dpi() {
        assert_eq!(cell_px(96), 3);
        assert_eq!(cell_px(144), 5, "rounded up from 4.5");
        assert_eq!(cell_px(192), 6);
        assert_eq!(cell_px(1), 1, "a cell is never zero pixels");
        let size = (600.0, 400.0);
        let at_96 = BurnField::for_space(size, 5, 1, 96).grid();
        let at_192 = BurnField::for_space(size, 5, 1, 192).grid();
        assert_eq!(
            at_96,
            Grid {
                cols: 200,
                rows: 134
            }
        );
        assert_eq!(
            at_192,
            Grid {
                cols: 100,
                rows: 67
            }
        );
        assert_eq!(at_96.cols, at_192.cols * 2, "half the columns at 200 %");
    }

    #[test]
    fn the_seed_varies_the_field_and_repeats() {
        assert_eq!(
            BurnField::for_space((200.0, 160.0), 5, 7, 96),
            BurnField::for_space((200.0, 160.0), 5, 7, 96)
        );
        assert_ne!(
            BurnField::for_space((200.0, 160.0), 5, 1, 96),
            BurnField::for_space((200.0, 160.0), 5, 2, 96)
        );
    }

    #[test]
    fn step_cost_prefers_climbing_up() {
        let up = step_cost(1.0, 0.7, CLIMB_UP);
        let level = step_cost(1.0, 0.7, 1.0);
        let down = step_cost(1.0, 0.7, CLIMB_DOWN);
        assert!(up < level && level < down);
    }

    #[test]
    fn a_spot_fire_opens_a_hole_ahead_of_the_main_front() {
        let (coarse, max_arrival) =
            solve_coarse(300, 300, LATTICE_BASE + LATTICE_PER_DETAIL * 5.0, 1);
        let has_hole = (0..300)
            .any(|col| (0..299).any(|row| coarse.value(col, row) < coarse.value(col, row + 1)));
        assert!(
            has_hole && max_arrival > 0.0,
            "a higher cell burns before a lower one"
        );
    }

    #[test]
    fn glow_thickness_stays_similar_on_short_and_tall_spaces() {
        let median_glow_px = |height: f32| {
            let field = BurnField::for_space((320.0, height), 5, 1, 96);
            let front = field.front(0.5);
            let Grid { cols, rows } = field.grid();
            let mut counts: Vec<f32> = (0..cols)
                .filter_map(|col| {
                    let cells = (0..rows)
                        .filter(|&row| matches!(field.stage(col, row, front), Stage::Burning(_)))
                        .count();
                    (cells > 0).then_some(cells as f32 * cell_px(96) as f32)
                })
                .collect();
            counts.sort_by(f32::total_cmp);
            assert!(!counts.is_empty());
            counts[counts.len() / 2]
        };
        let short = median_glow_px(100.0);
        let tall = median_glow_px(400.0);
        let ratio = (short / tall).max(tall / short);
        assert!(
            ratio < 2.0,
            "short={short} px, tall={tall} px, ratio={ratio}"
        );
    }

    #[test]
    fn ember_cells_near_glow_are_brighter_than_plain_char() {
        let red = |px: u32| (px >> 16) & 0xff;
        let (_, plain) = cell_pixels(
            Stage::Charred {
                along: 0.1,
                ember: false,
            },
            0.5,
        );
        let (_, ember_dim) = cell_pixels(
            Stage::Charred {
                along: 0.1,
                ember: true,
            },
            0.0,
        );
        let (_, ember_bright) = cell_pixels(
            Stage::Charred {
                along: 0.1,
                ember: true,
            },
            1.0,
        );
        assert!(red(ember_dim) > red(plain));
        assert!(red(ember_bright) > red(ember_dim));

        let field = BurnField::for_space((320.0, 240.0), 5, 1, 96);
        let front = field.front(0.5);
        let Grid { cols, rows } = field.grid();
        let mut paper = vec![0; field.grid().cell_count()];
        let mut marks = vec![0; field.grid().cell_count()];
        field.fill_rows(front, 0.3, 0..rows, &mut paper, &mut marks);
        let (mut saw_ember, mut saw_plain) = (None, None);
        for row in 0..rows {
            for col in 0..cols {
                if let Stage::Charred { along, ember } = field.stage(col, row, front)
                    && along < 0.2
                {
                    let px = marks[(row * cols + col) as usize];
                    if ember {
                        saw_ember = Some(px);
                    } else {
                        saw_plain = Some(px);
                    }
                }
            }
        }
        assert!(
            red(saw_ember.expect("field has ember char near glow"))
                > red(saw_plain.expect("field has plain char near glow"))
        );
    }

    #[test]
    fn lit_alpha_caps_at_48_and_drops_to_zero_at_far_edge() {
        let (_, near) = cell_pixels(Stage::Lit(1.0), 1.0);
        let (_, far) = cell_pixels(Stage::Lit(0.0), 1.0);
        assert_eq!(near >> 24, 48);
        assert_eq!(far >> 24, 0);

        let bands = Bands::DEFAULT;
        let Stage::Lit(edge) = bands.stage(bands.total() - 1e-6, || false) else {
            panic!("just inside light band must be Stage::Lit");
        };
        let (_, edge_px) = cell_pixels(Stage::Lit(edge), 1.0);
        assert_eq!(edge_px >> 24, 0);
    }

    #[test]
    fn burning_pixels_shimmer_over_time_and_stay_opaque() {
        let (_, at_zero) = cell_pixels(Stage::Burning(0.3), 0.0);
        let (_, at_one) = cell_pixels(Stage::Burning(0.3), 1.0);
        assert_ne!(at_zero, at_one);
        assert_eq!(at_zero >> 24, 255);
        assert_eq!(at_one >> 24, 255);

        let field = BurnField::for_space((200.0, 160.0), 5, 1, 96);
        let front = field.front(0.5);
        let Grid { cols, rows } = field.grid();
        let (mut p0, mut m0) = (
            vec![0; field.grid().cell_count()],
            vec![0; field.grid().cell_count()],
        );
        let (mut p1, mut m1) = (p0.clone(), m0.clone());
        field.fill_rows(front, 0.0, 0..rows, &mut p0, &mut m0);
        field.fill_rows(front, 0.25, 0..rows, &mut p1, &mut m1);
        let mut checked = 0;
        for row in 0..rows {
            for col in 0..cols {
                if matches!(field.stage(col, row, front), Stage::Burning(_)) {
                    let i = (row * cols + col) as usize;
                    assert_eq!(m0[i] >> 24, 255);
                    assert_eq!(m1[i] >> 24, 255);
                    if m0[i] != m1[i] {
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "burning edge must shimmer over time");
    }

    #[test]
    fn every_burn_pixel_is_premultiplied() {
        let check = |px: u32| {
            let a = px >> 24;
            let r = (px >> 16) & 0xff;
            let g = (px >> 8) & 0xff;
            let b = px & 0xff;
            assert!(r <= a && g <= a && b <= a, "invalid premul {px:#010x}");
        };
        for shimmer in [0.0, 0.5, 1.0] {
            for stage in [
                Stage::Gone,
                Stage::Burning(0.0),
                Stage::Burning(0.7),
                Stage::Charred {
                    along: 0.0,
                    ember: true,
                },
                Stage::Charred {
                    along: 0.8,
                    ember: false,
                },
                Stage::Scorched(0.5),
                Stage::Lit(0.8),
                Stage::Whole,
            ] {
                let (paper, marks) = cell_pixels(stage, shimmer);
                check(paper);
                check(marks);
            }
        }
        let field = BurnField::for_space((240.0, 180.0), 5, 3, 96);
        let rows = field.grid().rows;
        let (mut paper, mut marks) = (
            vec![0; field.grid().cell_count()],
            vec![0; field.grid().cell_count()],
        );
        field.fill_rows(field.front(0.5), 0.7, 0..rows, &mut paper, &mut marks);
        for px in paper.into_iter().chain(marks) {
            check(px);
        }
    }
}
