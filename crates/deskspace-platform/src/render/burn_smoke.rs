//! Rising smoke plumes behind Burn's glowing front (ROADMAP D12).
//!
//! Fresh char behind the fire emits seeded columns of warm-lit smoke on a
//! coarse display-scaled grid (`SMOKE_CELL_PX`). As each column climbs at
//! `SMOKE_RISE_PX_PER_S`, it widens, sways on upward-scrolling noise, cools
//! from firelit amber to neutral gray, and fades before the top of the
//! overlay.

use super::burn::{self, BurnField};
use super::flames::{self, Rows};
use super::math::smoothstep;
use super::noise;

/// Coarse simulation cell size in 96-DPI pixels.
const SMOKE_CELL_PX: f32 = 6.0;
/// Upward drift speed in 96-DPI pixels per second (slower than flames' 120).
const SMOKE_RISE_PX_PER_S: f32 = 40.0;
/// Maximum opacity of the smoke layer (at most 55 %).
const MAX_OPACITY: f32 = 0.55;
const MAX_ALPHA_U8: u32 = 140;
/// Vertical distance in 96-DPI pixels over which fresh-edge firelight warms
/// the bottom of the plume before it cools to neutral gray.
const WARM_PX: f32 = 15.0;
/// Characteristic rise distance in 96-DPI pixels controlling vertical decay.
const PLUME_DECAY_PX: f32 = 95.0;
/// Width/spacing of seeded columns along the edge in 96-DPI pixels.
const COLUMN_LATTICE_PX: f32 = 28.0;
/// Low-frequency lateral sway and secondary wisp noise lattices at 96 DPI.
const SWAY_LATTICE_PX: f32 = 48.0;
const WISP_LATTICE_PX: f32 = 22.0;

const COL_SALT: u32 = 0x510C_0101;
const SWAY_SALT: u32 = 0x510C_0202;
const WISP_SALT: u32 = 0x510C_0303;

/// Destination bitmap slice for [`paint`].
pub struct Canvas<'a> {
    pub pixels: &'a mut [u32],
    pub width: u32,
    pub height: u32,
}

/// Reusable scratch buffers so shading smoke allocates nothing after warmup.
#[derive(Default)]
pub struct Scratch {
    col_profile: Vec<f32>,
    sway_row: Vec<f32>,
    wisp_row: Vec<f32>,
    density: Vec<f32>,
    warmth: Vec<f32>,
    cells: Vec<u32>,
    row_active: Vec<bool>,
    vert_row: Vec<u32>,
    x_wx: Vec<u16>,
    x_spans: Vec<(u16, u16)>,
    y_map: Vec<(u16, u16)>,
}

/// Shades rising smoke for `field` at `front` and clock `time` (seconds) into
/// `canvas` (premultiplied BGRA), clearing any rows in `*drawn` not covered by
/// this frame and updating `*drawn` to the rows written.
pub fn paint(
    canvas: Canvas<'_>,
    field: &BurnField,
    front: f32,
    time: f32,
    scratch: &mut Scratch,
    drawn: &mut Rows,
) {
    let Canvas {
        pixels,
        width,
        height,
    } = canvas;
    let total_pixels = (width as usize).saturating_mul(height as usize);
    if width == 0 || height == 0 || pixels.len() < total_pixels {
        *drawn = Rows::NONE;
        return;
    }

    let gone = field.progress(front);
    let emission = ramp(0.0, 0.15, gone) * (1.0 - ramp(0.85, 1.0, gone));
    if emission <= 0.0 {
        drawn.clear(pixels, width, height);
        *drawn = Rows::NONE;
        return;
    }

    let dpi = field.dpi();
    let scale = dpi.max(1) as f32 / 96.0;
    let step = ((SMOKE_CELL_PX * scale).round() as u32).max(1);
    let step_f = step as f32;
    let cols = width.div_ceil(step).max(1) as usize;
    let rows = height.div_ceil(step).max(1) as usize;

    prepare_scratch(
        scratch,
        GridShape {
            cols,
            rows,
            width: width as usize,
            height: height as usize,
            step_f,
            scale,
            seed: field.seed(),
        },
    );

    let (side, top) = flames::margins(dpi);
    let burn_cell = burn::cell_px(dpi) as f32;
    let (char_start, char_end) = field.char_band();
    let emit_center = 0.5 * (char_start + char_end);
    let emit_half = (0.5 * (char_end - char_start) + 0.025).max(0.03);

    populate_sources(
        scratch,
        &SourceGrid {
            field,
            front,
            emission,
            side: side as f32,
            top: top as f32,
            burn_cell,
            step_f,
            emit_center,
            emit_half,
            cols,
            rows,
        },
    );

    propagate_and_shade(scratch, cols, rows, step_f, scale, time, field.seed());
    *drawn = upscale_rows(
        &mut pixels[..total_pixels],
        width,
        height,
        scratch,
        cols,
        rows,
        *drawn,
    );
}

struct SourceGrid<'a> {
    field: &'a BurnField,
    front: f32,
    emission: f32,
    side: f32,
    top: f32,
    burn_cell: f32,
    step_f: f32,
    emit_center: f32,
    emit_half: f32,
    cols: usize,
    rows: usize,
}

struct GridShape {
    cols: usize,
    rows: usize,
    width: usize,
    height: usize,
    step_f: f32,
    scale: f32,
    seed: u32,
}

fn prepare_scratch(scratch: &mut Scratch, shape: GridShape) {
    let GridShape {
        cols,
        rows,
        width,
        height,
        step_f,
        scale,
        seed,
    } = shape;
    scratch.col_profile.resize(cols, 0.0);
    scratch.sway_row.resize(cols, 0.0);
    scratch.wisp_row.resize(cols, 0.0);
    scratch.density.resize(cols * rows, 0.0);
    scratch.warmth.resize(cols * rows, 0.0);
    scratch.cells.resize(cols * rows, 0);
    scratch.row_active.resize(rows, false);
    scratch.vert_row.resize(cols, 0);
    scratch.x_wx.resize(width, 0);
    scratch.x_spans.resize(cols, (0, 0));
    scratch.y_map.resize(height, (0, 0));

    let inv_lattice = 1.0 / (COLUMN_LATTICE_PX * scale).max(1.0);
    let col_seed = seed ^ COL_SALT;
    for (c, slot) in scratch.col_profile.iter_mut().enumerate() {
        let x = (c as f32 + 0.5) * step_f;
        let n = noise::value_noise(x * inv_lattice, 0.5, 0, col_seed);
        let peak = ramp(0.32, 0.68, n).powf(1.3);
        *slot = 0.18 + 0.82 * peak;
    }

    let inv_step = 1.0 / step_f;
    let max_c = (cols - 1) as f32;
    let mut cur_c = 0usize;
    let mut span_start = 0u16;
    for px in 0..width {
        let fx = ((px as f32 + 0.5) * inv_step - 0.5).clamp(0.0, max_c);
        let x0 = fx.floor() as usize;
        scratch.x_wx[px] = ((fx - x0 as f32) * 256.0).round() as u16;
        while cur_c < x0 {
            scratch.x_spans[cur_c] = (span_start, px as u16);
            cur_c += 1;
            span_start = px as u16;
        }
    }
    scratch.x_spans[cur_c] = (span_start, width as u16);
    for slot in &mut scratch.x_spans[cur_c + 1..] {
        *slot = (width as u16, width as u16);
    }

    let max_r = (rows - 1) as f32;
    for (py, slot) in scratch.y_map.iter_mut().enumerate() {
        let fy = ((py as f32 + 0.5) * inv_step - 0.5).clamp(0.0, max_r);
        let y0 = fy.floor() as usize;
        let wy = ((fy - y0 as f32) * 256.0).round() as u16;
        *slot = (y0 as u16, wy);
    }
}

fn populate_sources(scratch: &mut Scratch, src: &SourceGrid<'_>) {
    scratch.density.fill(0.0);
    scratch.warmth.fill(0.0);
    scratch.row_active.fill(false);
    let grid = src.field.grid();
    let space_w = grid.cols as f32 * src.burn_cell;
    let space_h = grid.rows as f32 * src.burn_cell;
    let inv_cell = 1.0 / src.burn_cell.max(1.0);
    let inv_half = 1.0 / src.emit_half;
    let sub_offset = 0.25 * src.step_f;

    let min_c = ((src.side / src.step_f).floor().max(0.0) as usize).min(src.cols);
    let max_c = (((src.side + space_w) / src.step_f).ceil().max(0.0) as usize).min(src.cols);
    let min_r = ((src.top / src.step_f).floor().max(0.0) as usize).min(src.rows);
    let max_r = (((src.top + space_h) / src.step_f).ceil().max(0.0) as usize).min(src.rows);

    for r in min_r..max_r {
        let y = (r as f32 + 0.5) * src.step_f - src.top;
        if !(0.0..space_h).contains(&y) {
            continue;
        }
        let fy0 = (((y - sub_offset).clamp(0.0, space_h - 0.5)) * inv_cell) as u32;
        let fy1 = (((y + sub_offset).clamp(0.0, space_h - 0.5)) * inv_cell) as u32;
        let fy0 = fy0.min(grid.rows - 1);
        let fy1 = fy1.min(grid.rows - 1);
        let row_off = r * src.cols;
        let mut row_hit = false;

        for c in min_c..max_c {
            let x = (c as f32 + 0.5) * src.step_f - src.side;
            if !(0.0..space_w).contains(&x) {
                continue;
            }
            let fc = ((x * inv_cell) as u32).min(grid.cols - 1);
            let s0 = source_at(
                src.field.value(fc, fy0) - src.front,
                src.emit_center,
                inv_half,
            );
            let s1 = source_at(
                src.field.value(fc, fy1) - src.front,
                src.emit_center,
                inv_half,
            );
            let emitted = s0.max(s1) * scratch.col_profile[c] * src.emission * 0.65;
            if emitted > 0.0 {
                scratch.density[row_off + c] = emitted;
                scratch.warmth[row_off + c] = emitted;
                row_hit = true;
            }
        }
        scratch.row_active[r] = row_hit;
    }
}

fn source_at(above: f32, center: f32, inv_half: f32) -> f32 {
    let norm = (above - center).abs() * inv_half;
    if norm >= 1.0 {
        0.0
    } else {
        let t = 1.0 - norm;
        t * t * (3.0 - 2.0 * t)
    }
}

fn propagate_and_shade(
    scratch: &mut Scratch,
    cols: usize,
    rows: usize,
    step_f: f32,
    scale: f32,
    time: f32,
    seed: u32,
) {
    let scroll_y = time * SMOKE_RISE_PX_PER_S * scale;
    let sway_size = SWAY_LATTICE_PX * scale;
    let wisp_size = WISP_LATTICE_PX * scale;
    let step_decay = (-step_f / (PLUME_DECAY_PX * scale)).exp();
    let warm_decay = (-step_f / (WARM_PX * scale)).exp();
    let top_fade_px = (flames::SMOKE_ROOM_PX as f32 * 0.45 * scale).max(step_f * 2.0);
    let max_c = (cols - 1) as f32;

    if cols == 0 || rows == 0 {
        return;
    }
    // Keep row 0 strictly transparent so the top row of overlay pixels is
    // always zero ("gone by the top of the overlay").
    scratch.cells[..cols].fill(0);
    scratch.row_active[0] = false;

    let mut below_active = false;
    for r in (1..rows).rev() {
        let cur_start = r * cols;
        let src_active = scratch.row_active[r];
        if !below_active && !src_active {
            scratch.cells[cur_start..cur_start + cols].fill(0);
            scratch.row_active[r] = false;
            continue;
        }

        let y = (r as f32 + 0.5) * step_f;
        let top_fade = ramp(step_f, top_fade_px, y);
        scratch.sway_row.fill(0.0);
        noise::add_octave_row(
            y + scroll_y,
            step_f,
            sway_size,
            0,
            1.0,
            seed ^ SWAY_SALT,
            &mut scratch.sway_row,
        );
        scratch.wisp_row.fill(0.0);
        noise::add_octave_row(
            y + scroll_y * 1.15,
            step_f,
            wisp_size,
            1,
            1.0,
            seed ^ WISP_SALT,
            &mut scratch.wisp_row,
        );

        let mut active = false;
        if below_active && r + 1 < rows {
            let below_start = (r + 1) * cols;
            for c in 0..cols {
                let shift = (scratch.sway_row[c] - 0.5) * 0.7;
                let src_c = (c as f32 - shift).clamp(0.0, max_c);
                let i0 = src_c.floor() as usize;
                let i1 = (i0 + 1).min(cols - 1);
                let t = src_c - i0 as f32;

                let (d0, w0) =
                    sample_wide(&scratch.density, &scratch.warmth, below_start, cols, i0);
                let (d1, w1) =
                    sample_wide(&scratch.density, &scratch.warmth, below_start, cols, i1);
                let wisp = 0.90 + 0.08 * scratch.wisp_row[c];
                let prop_d = (d0 + (d1 - d0) * t) * step_decay * wisp;
                let prop_w = (w0 + (w1 - w0) * t) * step_decay * warm_decay * wisp;

                let emit = scratch.density[cur_start + c];
                let mut d = (1.0 - (1.0 - prop_d) * (1.0 - emit)).clamp(0.0, 1.0);
                let mut w = (prop_w + emit).min(d);
                if d < 0.015 {
                    d = 0.0;
                    w = 0.0;
                }
                scratch.density[cur_start + c] = d;
                scratch.warmth[cur_start + c] = w;

                let cell = shade_cell(d, w, top_fade);
                scratch.cells[cur_start + c] = cell;
                active |= cell != 0;
            }
        } else {
            for c in 0..cols {
                let d = scratch.density[cur_start + c];
                let w = scratch.warmth[cur_start + c];
                let cell = shade_cell(d, w, top_fade);
                scratch.cells[cur_start + c] = cell;
                active |= cell != 0;
            }
        }
        scratch.row_active[r] = active;
        below_active = active;
    }
}

/// 3-tap horizontal spread around column `col` in `row_start` so rising
/// plumes widen naturally with height.
fn sample_wide(
    density: &[f32],
    warmth: &[f32],
    row_start: usize,
    cols: usize,
    col: usize,
) -> (f32, f32) {
    let left = col.saturating_sub(1);
    let right = (col + 1).min(cols - 1);
    let d = 0.18 * density[row_start + left]
        + 0.64 * density[row_start + col]
        + 0.18 * density[row_start + right];
    let w = 0.18 * warmth[row_start + left]
        + 0.64 * warmth[row_start + col]
        + 0.18 * warmth[row_start + right];
    (d, w)
}

fn ramp(edge0: f32, edge1: f32, value: f32) -> f32 {
    smoothstep(((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0))
}

fn shade_cell(density: f32, warmth: f32, top_fade: f32) -> u32 {
    if density <= 0.0 || top_fade <= 0.0 {
        return 0;
    }
    let alpha = (density * top_fade).clamp(0.0, 1.0) * MAX_OPACITY;
    let alpha_u8 = (alpha * 255.0).round().clamp(0.0, MAX_ALPHA_U8 as f32) as u32;
    if alpha_u8 < 3 {
        return 0;
    }
    let warm = (warmth / density.max(1e-4)).clamp(0.0, 1.0) * 0.72;
    let gray = 0.56 + 0.18 * (1.0 - density);
    let red = gray + (0.96 - gray) * warm;
    let green = gray + (0.54 - gray) * warm;
    let blue = (gray + 0.01) + (0.20 - (gray + 0.01)) * warm;

    let a_f = alpha_u8 as f32;
    let ch = |c: f32| (c.clamp(0.0, 1.0) * a_f).round().min(a_f) as u32;
    (alpha_u8 << 24) | (ch(red) << 16) | (ch(green) << 8) | ch(blue)
}

fn upscale_rows(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    scratch: &mut Scratch,
    cols: usize,
    rows: usize,
    previous: Rows,
) -> Rows {
    let w_usize = width as usize;
    let max_c = cols.saturating_sub(1);
    let max_r = rows.saturating_sub(1);
    let mut top_row = u32::MAX;
    let mut bottom_row = 0u32;
    let prev_top = if previous.is_empty() {
        u32::MAX
    } else {
        previous.top()
    };
    let prev_bottom = if previous.is_empty() {
        0
    } else {
        previous.bottom().min(height)
    };

    for py in 0..height {
        let (y0_u16, wy_u16) = scratch.y_map[py as usize];
        let y0 = y0_u16 as usize;
        let y1 = (y0 + 1).min(max_r);
        let row_slice = &mut pixels[py as usize * w_usize..(py as usize + 1) * w_usize];

        if !scratch.row_active[y0] && !scratch.row_active[y1] {
            if py >= prev_top && py < prev_bottom {
                row_slice.fill(0);
            }
            continue;
        }

        let r0 = &scratch.cells[y0 * cols..y0 * cols + cols];
        let r1 = &scratch.cells[y1 * cols..y1 * cols + cols];
        let wy = u32::from(wy_u16);
        for (dst, (&c0, &c1)) in scratch.vert_row.iter_mut().zip(r0.iter().zip(r1.iter())) {
            *dst = if (c0 | c1) == 0 {
                0
            } else {
                mix_swar(c0, c1, wy)
            };
        }

        for (c, &(start_u16, end_u16)) in scratch.x_spans.iter().enumerate() {
            let start = start_u16 as usize;
            let end = end_u16 as usize;
            if start >= end {
                continue;
            }
            let left = scratch.vert_row[c];
            let right = scratch.vert_row[(c + 1).min(max_c)];
            let dst_span = &mut row_slice[start..end];
            if (left | right) == 0 {
                dst_span.fill(0);
                continue;
            }
            for (dst, &wx_u16) in dst_span.iter_mut().zip(&scratch.x_wx[start..end]) {
                *dst = mix_swar(left, right, u32::from(wx_u16));
            }
        }
        top_row = top_row.min(py);
        bottom_row = bottom_row.max(py + 1);
    }

    if top_row >= bottom_row {
        Rows::NONE
    } else {
        Rows::new(top_row, bottom_row)
    }
}

#[inline(always)]
fn mix_swar(a: u32, b: u32, w: u32) -> u32 {
    const MASK: u32 = 0x00FF_00FF;
    let inv = 256 - w;
    let rb = ((((a & MASK) * inv) + ((b & MASK) * w)) >> 8) & MASK;
    let ag = (((((a >> 8) & MASK) * inv) + (((b >> 8) & MASK) * w)) >> 8) & MASK;
    rb | (ag << 8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::DETAIL_DEFAULT;

    fn painted(size: (u32, u32), dpi: u32, seed: u32, gone: f32, time: f32) -> Vec<u32> {
        let field = BurnField::for_space((size.0 as f32, size.1 as f32), DETAIL_DEFAULT, seed, dpi);
        let (width, height) = flames::overlay_size(size.0, size.1, dpi);
        let mut pixels = vec![7u32; (width * height) as usize];
        let mut scratch = Scratch::default();
        let mut drawn = Rows::ALL;
        paint(
            Canvas {
                pixels: &mut pixels,
                width,
                height,
            },
            &field,
            field.front(gone),
            time,
            &mut scratch,
            &mut drawn,
        );
        pixels
    }

    #[test]
    fn nothing_smokes_before_or_after_the_burn() {
        for gone in [0.0, 1.0] {
            let pixels = painted((320, 240), 96, 1, gone, 0.3);
            assert!(
                pixels.iter().all(|&px| px == 0),
                "no smoke should remain at gone={gone}"
            );
        }
    }

    #[test]
    fn same_inputs_give_identical_pixels_and_seeds_differ() {
        let first = painted((320, 240), 96, 3, 0.5, 0.4);
        let second = painted((320, 240), 96, 3, 0.5, 0.4);
        let other_seed = painted((320, 240), 96, 4, 0.5, 0.4);
        assert_eq!(first, second);
        assert_ne!(first, other_seed);
    }

    #[test]
    fn smoke_is_denser_and_warmer_near_edge_than_at_top_and_premultiplied() {
        let space = (400, 300);
        let pixels = painted(space, 96, 1, 0.5, 0.25);
        let (width, _) = flames::overlay_size(space.0, space.1, 96);
        let w = width as usize;

        assert!(
            pixels[..w].iter().all(|&px| (px >> 24) == 0),
            "smoke must be gone at the very top row"
        );

        let mut max_alpha = 0u32;
        let mut warmest_diff = 0i32;
        for &px in &pixels {
            let a = px >> 24;
            let r = (px >> 16) & 0xff;
            let g = (px >> 8) & 0xff;
            let b = px & 0xff;
            assert!(r <= a && g <= a && b <= a, "invalid premul {px:#010x}");
            max_alpha = max_alpha.max(a);
            if a >= 25 {
                warmest_diff = warmest_diff.max(r as i32 - b as i32);
            }
        }
        assert!(max_alpha > 20, "mid-burn must produce visible smoke");
        assert!(
            max_alpha <= 141,
            "smoke must never exceed 55 % opacity ({max_alpha})"
        );
        assert!(
            warmest_diff > 10,
            "smoke near fire edge should be warmer than cool gray"
        );

        let top_band_max: u32 = pixels[..12 * w]
            .iter()
            .map(|&px| px >> 24)
            .max()
            .unwrap_or(0);
        assert!(
            max_alpha > top_band_max,
            "smoke near the burning edge ({max_alpha}) must exceed top band ({top_band_max})"
        );
    }

    #[test]
    fn high_burn_warmth_cools_from_edge_to_upper_plume() {
        let space = (320, 240);
        let pixels = painted(space, 96, 2, 0.6, 0.1);
        let (width, height) = flames::overlay_size(space.0, space.1, 96);
        let w = width as usize;

        // Find the peak row's average warmth vs 40 px higher.
        let row_stats: Vec<(u32, f32)> = (0..height as usize)
            .map(|y| {
                let mut a_sum = 0u32;
                let mut diff_sum = 0i32;
                let mut count = 0u32;
                for &px in &pixels[y * w..(y + 1) * w] {
                    let a = px >> 24;
                    a_sum += a;
                    if a >= 15 {
                        let r = ((px >> 16) & 0xff) as i32;
                        let b = (px & 0xff) as i32;
                        diff_sum += r - b;
                        count += 1;
                    }
                }
                (
                    a_sum,
                    if count > 0 {
                        diff_sum as f32 / count as f32
                    } else {
                        0.0
                    },
                )
            })
            .collect();

        let (peak_row, _) = row_stats
            .iter()
            .enumerate()
            .max_by_key(|&(_, (a_sum, _))| *a_sum)
            .unwrap();
        let (upper_row, upper_stats) = row_stats[..peak_row.saturating_sub(24)]
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, (a_sum, _))| *a_sum > 0)
            .expect("upper plume should have cool smoke rows above edge");
        assert!(
            row_stats[peak_row].1 > upper_stats.1,
            "lower smoke warmth at row {peak_row} ({}) should cool by row {upper_row} ({})",
            row_stats[peak_row].1,
            upper_stats.1
        );
    }

    #[test]
    fn paint_writes_nothing_outside_valid_rows_and_survives_tiny_buffers() {
        let field = BurnField::for_space((40.0, 40.0), DETAIL_DEFAULT, 1, 96);
        let mut scratch = Scratch::default();
        let mut drawn = Rows::ALL;
        let mut one = [9u32; 1];
        paint(
            Canvas {
                pixels: &mut one,
                width: 1,
                height: 1,
            },
            &field,
            field.front(0.5),
            0.0,
            &mut scratch,
            &mut drawn,
        );
        let px = one[0];
        let a = px >> 24;
        assert!(((px >> 16) & 0xff) <= a && ((px >> 8) & 0xff) <= a && (px & 0xff) <= a);
    }
}
