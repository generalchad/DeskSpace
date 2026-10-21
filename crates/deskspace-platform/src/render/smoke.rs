//! Smoke's pure math (ROADMAP D12): a space sublimates into plumes that rise
//! from a few seeded sources along its lower edge. The same field decides what
//! part of the window is still there and where the smoke is, so the crisp
//! window and the plume meet without a seam.
//!
//! A play passes a seed, so every play and every space smokes differently,
//! yet a play is a pure function of (`seed`, progress): reversing it retraces
//! the same wisps. The overlay samples the space's start snapshot through the
//! plume, so the contents themselves are carried away (`render::snapshot`).

use deskspace_core::animation::DETAIL_RANGE;
use deskspace_core::theme::Color;

use super::noise;
use super::snapshot::Snapshot;

/// Pixels per field cell; the painter draws the cells with linear
/// interpolation, so the edges stay soft.
pub const CELL: u32 = 3;
/// The largest value a cell takes, so the final front can pass every cell.
pub const VALUE_MAX: f32 = 1.0;
/// How far, in field units, the front's edge fades.
pub const SOFT: f32 = 0.08;

/// The noise's coarsest lattice, in cells, and per step of detail.
const LATTICE_BASE: f32 = 4.0;
const LATTICE_PER_DETAIL: f32 = 1.5;

/// The window's progress as a front in field units: at `gone` 0 nothing has
/// moved, at 1 everything has. Its ends sit a soft edge outside the field, so
/// no cell is ever half-hidden at the ends.
pub fn front(gone: f32) -> f32 {
    gone * (VALUE_MAX + 4.0 * SOFT) - 2.0 * SOFT
}

/// How much of a cell still shows at `front`: 1 whole, 0 gone.
pub fn cover(value: f32, front: f32) -> f32 {
    let edge = ((value - (front - SOFT)) / (2.0 * SOFT)).clamp(0.0, 1.0);
    edge * edge * (3.0 - 2.0 * edge)
}

/// How much room an overlay keeps beside and above the space, in pixels.
pub fn margins(width: u32, height: u32) -> (u32, u32) {
    let side = ((width as f32 * 0.18) as u32).clamp(32, 128);
    let top = ((height as f32 * 0.5) as u32).clamp(80, 260);
    (side, top)
}

/// A space's smoke field, `cols` by `rows` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct SmokeField {
    cols: u32,
    rows: u32,
    values: Vec<f32>,
    width: u32,
    height: u32,
    detail: u8,
    seed: u32,
    lattice: f32,
    offset_x: f32,
    offset_y: f32,
    sources: Vec<(f32, f32)>,
}

impl SmokeField {
    /// A field for a space `width` by `height` pixels; `detail`
    /// (`DETAIL_RANGE`) sets how many plumes and how ragged they are, and
    /// `seed` makes this play's plumes its own.
    pub fn new(width: u32, height: u32, detail: u8, seed: u32) -> Self {
        let detail = detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1);
        let cols = width.div_ceil(CELL).max(1);
        let rows = height.div_ceil(CELL).max(1);
        let lattice = LATTICE_BASE + LATTICE_PER_DETAIL * f32::from(detail);
        let offset_x = noise::random(seed, 0) * 100.0;
        let offset_y = noise::random(seed, 1) * 100.0;
        let sources = sources(width, height, detail, seed);
        let values = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (col, row)))
            .map(|(col, row)| {
                let noise = noise::fractal(col as f32 + offset_x, row as f32 + offset_y, lattice);
                // Low at the bottom, so the front eats upward; the noise keeps
                // its edge ragged. The columns shape the smoke, not the hole.
                let bottom = (row as f32 + 0.5) / rows as f32;
                let value = (1.0 - bottom) * 0.35 + noise * 0.65;
                value.clamp(0.0, VALUE_MAX)
            })
            .collect();
        Self {
            cols,
            rows,
            values,
            width,
            height,
            detail,
            seed,
            lattice,
            offset_x,
            offset_y,
            sources,
        }
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

    /// The field at a point in space pixels, bilinear and clamped to the
    /// edges, so sampling above or beside the space still sees its edge.
    pub fn value_at(&self, x: f32, y: f32) -> f32 {
        let fx = (x / CELL as f32 - 0.5).clamp(0.0, (self.cols - 1) as f32);
        let fy = (y / CELL as f32 - 0.5).clamp(0.0, (self.rows - 1) as f32);
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.cols - 1), (y0 + 1).min(self.rows - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let at = |col, row| self.value(col, row);
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * tx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * tx;
        top + (bottom - top) * ty
    }

    /// How much of a plume is at (`x`, `y`): the nearest source's column,
    /// narrow at the foot and spreading as it rises, with `sway` drifting the
    /// whole plume sideways. Keeps the smoke in distinct columns rather than
    /// one fog bank.
    pub fn column(&self, x: f32, y: f32, sway: f32) -> f32 {
        column_at(
            &self.sources,
            self.seed,
            self.width,
            self.height,
            x,
            y,
            sway,
        )
    }

    /// The smoke's density at (`x`, `y`): thick at the bottom and in the
    /// columns, with the same noise as the erosion so the two agree.
    pub fn density_at(&self, x: f32, y: f32, sway: f32) -> f32 {
        let bottom = (y / self.height as f32).clamp(0.0, 1.0);
        let noise = noise::fractal(
            x / CELL as f32 + self.offset_x,
            y / CELL as f32 + self.offset_y,
            self.lattice,
        );
        let column = self.column(x, y, sway);
        let body = (0.5 + 0.5 * noise) * (0.45 + 0.55 * bottom);
        ((0.05 + 0.95 * column) * body).clamp(0.0, 1.0)
    }

    /// The tint the plume carries: the space's own, softened toward a neutral
    /// gray and lifted so thin smoke reads on a dark backdrop too.
    fn body(&self, tint: Color) -> (f32, f32, f32) {
        let neutral = (0.72, 0.72, 0.74);
        if tint.a == 0 {
            return neutral;
        }
        let lift = |channel: u8| {
            let value = f32::from(channel) / 255.0;
            (value + (1.0 - value) * 0.4).max(0.48)
        };
        let mix = |tinted: f32, plain: f32| tinted * 0.5 + plain * 0.5;
        (
            mix(lift(tint.r), neutral.0),
            mix(lift(tint.g), neutral.1),
            mix(lift(tint.b), neutral.2),
        )
    }
}

/// Where a plume starts: spread along the bottom, each with its own height
/// and jitter from the play's seed.
fn sources(width: u32, height: u32, detail: u8, seed: u32) -> Vec<(f32, f32)> {
    let count = 3 + (u32::from(detail).saturating_sub(1)) / 3;
    let width = width as f32;
    let height = height as f32;
    (0..count)
        .map(|index| {
            let spread = width / count as f32;
            let base = (index as f32 + 0.5) * spread;
            let jitter = (noise::random(seed, 10 + index) - 0.5) * spread * 0.55;
            let x = (base + jitter).clamp(width * 0.06, width * 0.94);
            let lift = height * (0.02 + 0.08 * noise::random(seed, 30 + index));
            (x, height - lift)
        })
        .collect()
}

/// A source's column at (`x`, `y`): narrow at the foot and spreading as it
/// rises, with `sway` drifting each plume sideways.
fn column_at(
    sources: &[(f32, f32)],
    seed: u32,
    width: u32,
    height: u32,
    x: f32,
    y: f32,
    sway: f32,
) -> f32 {
    let width = width as f32;
    let height = height as f32;
    let above = ((height - y) / height).clamp(0.0, 1.0);
    let spread = width * (0.06 + 0.12 * above);
    sources
        .iter()
        .enumerate()
        .map(|(index, (source_x, _))| {
            let lean = (noise::random(seed, 70 + index as u32) - 0.5) * width * 0.22 * above;
            let own = sway * (0.55 + 0.45 * noise::random(seed, 50 + index as u32)) + lean;
            let distance = (x - (source_x + own)) / spread;
            (-(distance * distance)).exp()
        })
        .fold(0.0, f32::max)
}

/// The scratch a painter keeps between frames, so smoke allocates nothing
/// while it plays.
#[derive(Default)]
pub struct Scratch {
    cells: Vec<u32>,
}

/// Where smoke is drawn: the overlay's pixels and size, with the space's
/// top-left at `origin`.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u32],
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
}

/// One frame of smoke: the field, the contents it carries, its tint, and
/// `gone` (0 whole, 1 gone).
pub struct Plume<'a> {
    pub field: &'a SmokeField,
    pub snapshot: Option<&'a Snapshot>,
    pub tint: Color,
    pub gone: f32,
}

/// Shades `plume` into `canvas`, premultiplied BGRA. Clears `canvas` when
/// there is no smoke.
pub fn paint(canvas: Canvas<'_>, plume: Plume<'_>, scratch: &mut Scratch) {
    let Canvas {
        pixels,
        width,
        height,
        origin,
    } = canvas;
    let Plume {
        field,
        snapshot,
        tint,
        gone,
    } = plume;
    let gone = gone.clamp(0.0, 1.0);
    let emission = smoothstep(0.0, 0.2, gone) * (1.0 - smoothstep(0.85, 1.0, gone));
    if emission <= 0.0 {
        pixels.fill(0);
        return;
    }
    let step = overlay_cell(width, height);
    let cols = width.div_ceil(step);
    let rows = height.div_ceil(step);
    scratch.cells.resize((cols * rows) as usize, 0);
    let front = front(gone);
    let body = field.body(tint);
    let scale = 10.0 + 2.0 * f32::from(field.detail);
    let warp_scale = scale * 0.5;
    let space_width = field.width as f32;
    let space_height = field.height as f32;
    for row in 0..rows {
        for col in 0..cols {
            let x = (col as f32 + 0.5) * step as f32;
            let y = (row as f32 + 0.5) * step as f32;
            let sx = x - origin.0 as f32;
            let sy = y - origin.1 as f32;
            let above = (space_height - sy).max(0.0) / space_height;
            // The plume keeps climbing with the play, so smoke from the first
            // eroded cells is well above the front by the middle.
            let rise = gone * space_height * 0.45;
            let lift = above.min(1.8) * space_height * 0.35 + rise;
            let drift = noise::fractal(
                sx / scale + field.offset_x,
                (sy - gone * space_height * 0.8) / scale + field.offset_y,
                6.0,
            );
            let sway = (drift - 0.5) * space_width * 0.14;
            let turbulence = noise::fractal(
                (sx + sway) / warp_scale + field.offset_y,
                (sy - gone * space_height * 0.6) / warp_scale + field.offset_x,
                5.0,
            );
            let fine = noise::fractal(
                (sx - sway) / (warp_scale * 0.35) + field.offset_x,
                (sy - gone * space_height * 0.9) / (warp_scale * 0.35) + field.offset_y,
                4.0,
            );
            let sample_x = sx - sway + (fine - 0.5) * space_width * 0.06;
            let sample_y = sy + lift;
            let fade = 1.0 - smoothstep(0.85, 1.5, above);
            let puff = 0.7 + 0.3 * smoothstep(0.25, 0.75, turbulence);
            let edge = 0.8 + 0.2 * fine;
            // Only the part the front has already reached smokes, so the
            // plume starts low and climbs with the window.
            let gate = smoothstep(-0.3, 0.05, front - field.value_at(sample_x, sample_y));
            let density =
                (emission * gate * field.density_at(sample_x, sample_y, sway) * puff * edge * fade)
                    .min(0.9);
            let light = 0.88 + 0.12 * turbulence;
            let pixel = shade(
                density,
                snapshot.and_then(|snapshot| {
                    (sample_x >= 0.0
                        && sample_y >= 0.0
                        && sample_x < snapshot.width() as f32
                        && sample_y < snapshot.height() as f32)
                        .then(|| snapshot.sample(sample_x, sample_y))
                }),
                body,
                light,
            );
            scratch.cells[(row * cols + col) as usize] = pixel;
        }
    }
    upscale(pixels, width, height, &scratch.cells, cols, rows, step);
}

/// The coarse cell size for an overlay of `width` by `height`: bigger overlays
/// use bigger cells, so the CPU work stays bounded and the bilinear upscale
/// keeps the smoke soft.
pub(super) fn overlay_cell(width: u32, height: u32) -> u32 {
    let area = width as f32 * height as f32;
    ((area / 45_000.0).sqrt().ceil() as u32).clamp(3, 8)
}

/// A smoke cell's premultiplied pixel at `density`, carrying `content` (the
/// snapshot pixel, premultiplied) or just the tinted body when there is none.
fn shade(density: f32, content: Option<u32>, body: (f32, f32, f32), light: f32) -> u32 {
    if density <= 0.0 {
        return 0;
    }
    let density = density.clamp(0.0, 1.0);
    let thin = 1.0 - density;
    let lit = |value: f32| (value * light).clamp(0.0, 1.0);
    let (color, alpha) = match content {
        Some(pixel) if pixel >> 24 > 4 => {
            let pixel_alpha = f32::from((pixel >> 24) as u8) / 255.0;
            let channel = |shift: u32| {
                (f32::from(((pixel >> shift) & 0xFF) as u8) / 255.0) / pixel_alpha.max(0.01)
            };
            let lift = |value: f32| value * 0.72 + 0.28;
            let content = (lift(channel(16)), lift(channel(8)), lift(channel(0)));
            let tinted = (
                content.0 * 0.55 + body.0 * 0.45,
                content.1 * 0.55 + body.1 * 0.45,
                content.2 * 0.55 + body.2 * 0.45,
            );
            let color = (
                lit(tinted.0 + (0.94 - tinted.0) * 0.5 * thin),
                lit(tinted.1 + (0.94 - tinted.1) * 0.5 * thin),
                lit(tinted.2 + (0.96 - tinted.2) * 0.5 * thin),
            );
            (color, density * (0.65 + 0.25 * pixel_alpha))
        }
        _ => {
            let color = (
                lit(body.0 + (0.94 - body.0) * 0.5 * thin),
                lit(body.1 + (0.94 - body.1) * 0.5 * thin),
                lit(body.2 + (0.96 - body.2) * 0.5 * thin),
            );
            (color, density * 0.85)
        }
    };
    let channel = |value: f32| (value.clamp(0.0, 1.0) * alpha * 255.0).round() as u32;
    ((alpha * 255.0).round() as u32) << 24
        | channel(color.0) << 16
        | channel(color.1) << 8
        | channel(color.2)
}

/// Bilinearly stretches the coarse cells over every pixel of `pixels`.
pub(super) fn upscale(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    cells: &[u32],
    cols: u32,
    rows: u32,
    step: u32,
) {
    for y in 0..height {
        let fy = ((y as f32 + 0.5) / step as f32 - 0.5).clamp(0.0, (rows - 1) as f32);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(rows - 1);
        let ty = fy - y0 as f32;
        for x in 0..width {
            let fx = ((x as f32 + 0.5) / step as f32 - 0.5).clamp(0.0, (cols - 1) as f32);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(cols - 1);
            let tx = fx - x0 as f32;
            let cell = |col, row| cells[(row * cols + col) as usize];
            let (top_left, top_right) = (cell(x0, y0), cell(x1, y0));
            let (bottom_left, bottom_right) = (cell(x0, y1), cell(x1, y1));
            let mut value = 0;
            for channel in 0..4 {
                let shift = channel * 8;
                let at = |pixel: u32| f32::from(((pixel >> shift) & 0xFF) as u8);
                let top = at(top_left) + (at(top_right) - at(top_left)) * tx;
                let bottom = at(bottom_left) + (at(bottom_right) - at(bottom_left)) * tx;
                let channel = (top + (bottom - top) * ty).round() as u32;
                value |= channel << shift;
            }
            pixels[(y * width + x) as usize] = value;
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> SmokeField {
        SmokeField::new(80, 60, 5, 7)
    }

    #[test]
    fn the_front_runs_from_whole_to_gone() {
        let at = |gone| front(gone);
        for value in [0.0, 0.1, 0.5, VALUE_MAX] {
            assert_eq!(cover(value, at(0.0)), 1.0, "nothing hides at 0");
            assert_eq!(cover(value, at(1.0)), 0.0, "everything hides at 1");
        }
    }

    #[test]
    fn the_field_stays_in_range_and_rises_from_the_bottom() {
        let field = field();
        assert!(
            field
                .values()
                .iter()
                .all(|value| (0.0..=VALUE_MAX).contains(value))
        );
        let row_mean = |row: u32| {
            (0..field.cols())
                .map(|col| field.value(col, row))
                .sum::<f32>()
                / field.cols() as f32
        };
        assert!(row_mean(0) > row_mean(field.rows() / 2));
        assert!(row_mean(field.rows() / 2) > row_mean(field.rows() - 1));
    }

    #[test]
    fn sampling_between_cells_matches_the_cells() {
        let field = field();
        for (col, row) in [(0, 0), (3, 7), (10, 15)] {
            let x = (col as f32 + 0.5) * CELL as f32;
            let y = (row as f32 + 0.5) * CELL as f32;
            assert!((field.value_at(x, y) - field.value(col, row)).abs() < 0.001);
        }
        let at = field.value_at(-100.0, -100.0);
        assert_eq!(at, field.value(0, 0), "clamped to the edge");
    }

    fn painted(gone: f32, seed: u32, snapshot: Option<&Snapshot>) -> (Vec<u32>, u32, u32) {
        let field = SmokeField::new(80, 60, 5, seed);
        let (width, height) = (160, 160);
        let mut pixels = vec![7; (width * height) as usize];
        let mut scratch = Scratch::default();
        paint(
            Canvas {
                pixels: &mut pixels,
                width,
                height,
                origin: (40, 60),
            },
            Plume {
                field: &field,
                snapshot,
                tint: Color::rgba(20, 60, 200, 200),
                gone,
            },
            &mut scratch,
        );
        (pixels, width, height)
    }

    #[test]
    fn nothing_is_drawn_at_either_end() {
        for gone in [0.0, 1.0] {
            let (pixels, _, _) = painted(gone, 7, None);
            assert!(pixels.iter().all(|pixel| *pixel == 0), "{gone}");
        }
    }

    #[test]
    fn the_smoke_is_tinted_and_carries_the_snapshot() {
        let snapshot = Snapshot::new(80, 60, vec![0xFFFF_0000; 80 * 60].into_boxed_slice());
        let (pixels, _, _) = painted(0.5, 7, Some(&snapshot));
        assert!(pixels.iter().any(|pixel| *pixel != 0), "smoke shows");
        // The snapshot is red; the tint is blue. Both reach the plume.
        assert!(
            pixels
                .iter()
                .any(|pixel| (pixel >> 16) & 0xFF > 40 && (pixel >> 24) & 0xFF > 20),
            "the contents' red survives"
        );
        let (pixels, _, _) = painted(0.5, 7, None);
        assert!(
            pixels.iter().any(|pixel| pixel & 0xFF > 40),
            "the tint's blue bodies the smoke"
        );
    }

    #[test]
    fn smoke_rises_past_the_frame_by_the_middle_of_the_play() {
        let (pixels, width, _) = painted(0.5, 7, None);
        // The space's top is 60 px down in this overlay.
        let above = (0..60)
            .flat_map(|y| (0..width).map(move |x| (y * width + x) as usize))
            .any(|index| pixels[index] != 0);
        assert!(above, "the plume reaches above the space");
    }

    #[test]
    fn a_play_is_stable_and_its_seed_changes_the_smoke() {
        let (first, _, _) = painted(0.5, 7, None);
        let (again, _, _) = painted(0.5, 7, None);
        assert_eq!(first, again);
        let (other, _, _) = painted(0.5, 8, None);
        assert_ne!(first, other);
    }

    #[test]
    fn every_smoke_pixel_stays_premultiplied() {
        let (pixels, _, _) = painted(0.5, 7, None);
        for pixel in pixels {
            let alpha = pixel >> 24;
            for shift in [0, 8, 16] {
                assert!((pixel >> shift) & 0xFF <= alpha, "{pixel:#010X}");
            }
        }
    }

    #[test]
    fn margins_leave_room_above_and_beside() {
        assert_eq!(margins(400, 300), (72, 150));
        assert_eq!(margins(4000, 2000), (128, 260));
        assert_eq!(margins(10, 10), (32, 80));
    }
}
