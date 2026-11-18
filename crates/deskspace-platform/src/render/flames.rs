//! Burn's flames (ROADMAP D12, amended 2026-10-18): tongues of fire that
//! stand on the burning edge (`burn::BurnField::flame_bases`) and lick up
//! past the space's frame, shaded on the CPU into premultiplied pixels for a
//! layered overlay window. Pure, so the layout and the shading are tested.

use std::cell::RefCell;
use std::time::Instant;

use super::burn::{self, FlameBase};
use super::math;
use super::noise;

/// How much room the overlay keeps beside and above the space for flames and
/// rising smoke, in pixels at 96 DPI; `margins` scales them for a display.
const MARGIN_SIDE: u32 = 24;
const FLAME_REACH_PX: u32 = 72;
pub const SMOKE_ROOM_PX: u32 = 120;
const MARGIN_TOP: u32 = FLAME_REACH_PX + SMOKE_ROOM_PX;
/// A flame every this many burn cells along the front.
pub const SPACING: u32 = 5;

/// The overlay's margins beside and above the space, in pixels, at `dpi`:
/// `MARGIN_SIDE` and `MARGIN_TOP` at 96 DPI, scaled and rounded.
pub fn margins(dpi: u32) -> (u32, u32) {
    let dpi = dpi.max(1);
    ((MARGIN_SIDE * dpi + 48) / 96, (MARGIN_TOP * dpi + 48) / 96)
}

/// The pixel size of the overlay a space `width` by `height` pixels at `dpi`
/// burns on: the space plus the room beside and above it the flames rise
/// into. The window that hosts the overlay and a composed frame
/// (`effect_sheet`) both take their size from here, so they cannot disagree.
pub fn overlay_size(width: u32, height: u32, dpi: u32) -> (u32, u32) {
    let (side, top) = margins(dpi);
    (width + 2 * side, height + top)
}

/// A flame's height as a share of the space's, and its limits in pixels.
const HEIGHT_SHARE: f32 = 0.2;
const HEIGHT_RANGE: (f32, f32) = (18.0, 60.0);
/// A flame's width as a share of its height.
const WIDTH_SHARE: f32 = 0.42;
/// How much of a flame's height its heat decides: a cold one is this share
/// shorter than a hot one.
const HEAT_HEIGHT: f32 = 0.45;

/// The flicker: a flame's height swings between `FLICKER_BASE -
/// FLICKER_DEPTH` and `FLICKER_BASE + FLICKER_DEPTH` of its own as two waves
/// (radians per second) that never fall into step.
const FLICKER_BASE: f32 = 0.78;
const FLICKER_DEPTH: f32 = 0.22;
const FLICKER_RATES: (f32, f32) = (9.0, 5.3);
/// How much the second wave's phase differs from the first's.
const FLICKER_PHASE_SPREAD: f32 = 1.7;

/// The outline: the half-width tapers as `(1 - up)^TAPER` and narrows by
/// up to `NECK` toward the tip.
const TAPER: f32 = 0.6;
const NECK: f32 = 0.45;
/// The sway: waves along a flame's height, radians per second, and the
/// swing at the tip as a share of its width.
const SWAY_WAVES: f32 = 4.5;
const SWAY_RATE: f32 = 7.0;
const SWAY_SHARE: f32 = 0.3;
/// The heat across a flame falls off toward its tip as `(1 - up)^TIP_FALLOFF`,
/// and a flame's own heat brightens it by up to `HEAT_BRIGHTNESS`.
const TIP_FALLOFF: f32 = 0.8;
const HEAT_BRIGHTNESS: f32 = 0.4;
/// Below this heat a pixel stays clear.
const HEAT_CUTOFF: f32 = 0.02;
/// How fast turbulence scrolls upward through a tongue, in 96-DPI pixels/s.
const RISE_PX_PER_S: f32 = 120.0;
/// Maximum factor turbulence (`0.45 + 1.1 * n`) can multiply heat by.
const MAX_TURBULENCE: f32 = 1.55;

/// Every this many flames throws an ember.
const EMBER_EVERY: usize = 3;
/// Where an ember starts, up its flame, and how far above that it rises, in
/// flame heights.
const EMBER_START: f32 = 0.4;
const EMBER_RISE: f32 = 2.2;
/// Rises per second, and how far a flame's phase and its place in the row
/// offset its ember, so the embers do not rise together.
const EMBER_RATE: f32 = 0.9;
const EMBER_PHASE_SHARE: f32 = 0.37;
const EMBER_ORDER_SHARE: f32 = 0.13;
/// The side-to-side drift, in radians per second, and its swing as a share
/// of the flame's width.
const EMBER_DRIFT_RATE: f32 = 3.0;
const EMBER_DRIFT_SHARE: f32 = 0.6;
/// How hot an ember starts, fading as it rises.
const EMBER_HEAT: f32 = 0.55;

/// Fire's colors: the color (red, green, blue) at each heat, running
/// straight between them: deep red at the edges, through orange, to a
/// yellow-white core.
const FIRE_RAMP: [(f32, (f32, f32, f32)); 4] = [
    (0.0, (200.0, 40.0, 10.0)),
    (0.4, (255.0, 100.0, 10.0)),
    (0.75, (255.0, 210.0, 40.0)),
    (1.0, (255.0, 250.0, 190.0)),
];
/// Fire's opacity per unit of heat: fully opaque from 1 / `FIRE_ALPHA_GAIN`.
const FIRE_ALPHA_GAIN: f32 = 1.6;
/// Quarter-resolution glow around flames: downsample block, separable box-blur
/// radius in quarter cells (2 passes), and additive brightness gain.
const GLOW_STEP: u32 = 4;
const GLOW_RADIUS: usize = 3;
const GLOW_GAIN: f32 = 0.5;

/// One tongue of fire, in overlay pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tongue {
    /// The middle of its foot.
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub width: f32,
    /// 0 to 1: how bright and tall it burns.
    pub heat: f32,
    /// Its own flicker, so neighbours do not move in step.
    pub phase: f32,
    /// The display it was made for, `dpi / 96`: the ember it throws is this
    /// much bigger.
    pub scale: f32,
    /// Seeds turbulence and spark phases so two plays differ.
    pub seed: u32,
}

/// The tongues standing on `bases` at `time` (seconds) for a space
/// `space_height` tall on a display of `dpi`, into `tongues`. `seed` varies
/// tongue sizes and phases across plays.
pub fn tongues(
    bases: &[FlameBase],
    space_height: f32,
    dpi: u32,
    time: f32,
    seed: u32,
    tongues: &mut Vec<Tongue>,
) {
    let (side, top) = margins(dpi);
    let cell = burn::cell_px(dpi);
    let scale = dpi.max(1) as f32 / 96.0;
    tongues.clear();
    let tallest =
        (space_height * HEIGHT_SHARE).clamp(HEIGHT_RANGE.0 * scale, HEIGHT_RANGE.1 * scale);
    tongues.extend(bases.iter().map(|base| {
        let phase = phase_of(seed, base.col);
        let waves = (time * FLICKER_RATES.0 + phase).sin()
            * (time * FLICKER_RATES.1 + phase * FLICKER_PHASE_SPREAD).cos();
        let flicker = FLICKER_BASE + FLICKER_DEPTH * waves;
        let size_vary = 0.6 + 0.7 * noise::random(seed, base.col);
        let height = tallest * (1.0 - HEAT_HEIGHT + HEAT_HEIGHT * base.heat) * flicker * size_vary;
        Tongue {
            x: side as f32 + (base.col * cell) as f32 + cell as f32 / 2.0,
            y: top as f32 + (base.row * cell) as f32 + cell as f32,
            height,
            width: height * WIDTH_SHARE,
            heat: base.heat,
            phase,
            scale,
            seed,
        }
    }));
}

/// A run of the overlay's rows, from `top` up to but not including `bottom`:
/// what one frame drew, which is all the next frame has to clear. The flames
/// cover only a band of the overlay, so clearing all of it for every frame
/// is mostly wasted work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    top: u32,
    bottom: u32,
}

impl Rows {
    /// Every row: a buffer nothing is known about, so its first frame clears
    /// all of it.
    pub const ALL: Self = Self {
        top: 0,
        bottom: u32::MAX,
    };
    pub(crate) const NONE: Self = Self { top: 0, bottom: 0 };

    pub(crate) fn new(top: u32, bottom: u32) -> Self {
        Self { top, bottom }
    }

    pub fn is_empty(self) -> bool {
        self.top >= self.bottom
    }

    pub fn top(self) -> u32 {
        self.top
    }

    pub fn bottom(self) -> u32 {
        self.bottom
    }

    /// The run that holds both.
    pub fn union(self, other: Self) -> Self {
        if other.is_empty() {
            self
        } else if self.is_empty() {
            other
        } else {
            Self {
                top: self.top.min(other.top),
                bottom: self.bottom.max(other.bottom),
            }
        }
    }

    /// Clears these rows of `pixels`, `width` by `height`.
    pub(crate) fn clear(self, pixels: &mut [u32], width: u32, height: u32) {
        if self.is_empty() {
            return;
        }
        let start_of = |row: u32| row.min(height) as usize * width as usize;
        pixels[start_of(self.top)..start_of(self.bottom)].fill(0);
    }
}

#[derive(Default)]
struct GlowScratch {
    q0: Vec<[f32; 4]>,
    q1: Vec<[f32; 4]>,
    col_acc: Vec<[f32; 4]>,
    q_px: Vec<u32>,
    vert: Vec<u32>,
}

thread_local! {
    static GLOW: RefCell<GlowScratch> = RefCell::new(GlowScratch::default());
}

/// Shades `tongues` at `time` (seconds) into `pixels`, `width` by `height`
/// premultiplied BGRA (`0xAARRGGBB`). Overlapping fire adds channel by channel,
/// capped at 255.
///
/// `drawn` belongs to `pixels`: it comes in as the rows the frame before
/// drew there (`Rows::ALL` for a new buffer), which are cleared first, and
/// goes out as the rows this frame drew. Rows outside it are not touched.
pub fn paint(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    tongues: &[Tongue],
    time: f32,
    drawn: &mut Rows,
) {
    let started = Instant::now();
    drawn.clear(pixels, width, height);
    let band_rows = tongue_row_bounds(tongues, height);
    let mut now = Rows::NONE;
    if !band_rows.is_empty() && width > 0 {
        GLOW.with(|cell| {
            let mut scratch = cell.borrow_mut();
            let (q_top, q_bottom, full_q_rows) = glow_quarter_bounds(band_rows, height);
            let q_cols = width.div_ceil(GLOW_STEP) as usize;
            let q_rows = (q_bottom - q_top) as usize;
            let q_len = q_cols * q_rows;
            scratch.q0.resize(q_len, [0.0; 4]);
            scratch.q0.fill([0.0; 4]);

            for tongue in tongues {
                let rows = paint_tongue(
                    pixels,
                    width,
                    height,
                    tongue,
                    time,
                    Some((&mut scratch.q0, q_top, q_cols)),
                );
                now = now.union(rows);
            }
            if !now.is_empty() {
                let glow_rows = apply_glow(
                    pixels,
                    width,
                    height,
                    (q_top, q_bottom, full_q_rows),
                    &mut scratch,
                );
                now = now.union(glow_rows);
            }
        });
    }
    for (index, tongue) in tongues.iter().enumerate().step_by(EMBER_EVERY) {
        now = now.union(paint_ember(pixels, width, height, tongue, time, index));
    }
    *drawn = now;
    tracing::trace!(
        elapsed_us = started.elapsed().as_micros() as u64,
        tongues = tongues.len(),
        "shaded flames"
    );
}

fn tongue_row_bounds(tongues: &[Tongue], height: u32) -> Rows {
    let mut bounds = Rows::NONE;
    for tongue in tongues {
        let top = (tongue.y - tongue.height).floor().max(0.0) as u32;
        let bottom = (tongue.y.ceil().max(0.0) as u32).min(height);
        bounds = bounds.union(Rows { top, bottom });
    }
    bounds
}

fn glow_quarter_bounds(band_rows: Rows, height: u32) -> (u32, u32, usize) {
    let full_q_rows = height.div_ceil(GLOW_STEP) as usize;
    let expand = (GLOW_RADIUS * 2) as u32;
    let q_src_top = band_rows.top / GLOW_STEP;
    let q_src_bottom = band_rows.bottom.div_ceil(GLOW_STEP).min(full_q_rows as u32);
    let q_top = q_src_top.saturating_sub(expand);
    let q_bottom = (q_src_bottom + expand).min(full_q_rows as u32);
    (q_top, q_bottom, full_q_rows)
}

/// Shades one tongue directly into `pixels` and optionally accumulates its raw
/// pixel channels into the quarter-resolution grid `glow_acc` `(cells, q_top, q_cols)`.
fn paint_tongue(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    tongue: &Tongue,
    time: f32,
    mut glow_acc: Option<(&mut [[f32; 4]], u32, usize)>,
) -> Rows {
    let left = (tongue.x - tongue.width).floor().max(0.0) as u32;
    let right = ((tongue.x + tongue.width).ceil() as u32).min(width);
    let top = (tongue.y - tongue.height).floor().max(0.0) as u32;
    let bottom = (tongue.y.ceil() as u32).min(height);
    let brightness = 1.0 - HEAT_BRIGHTNESS + HEAT_BRIGHTNESS * tongue.heat;
    let inv_turb_w = 1.0 / (tongue.width * 0.6).max(0.5);
    let inv_turb_h = 1.0 / (tongue.height * 0.3).max(0.5);
    let scroll = time * RISE_PX_PER_S * tongue.scale;
    let mut lit_top = u32::MAX;
    let mut lit_bottom = 0u32;
    for py in top..bottom {
        // 0 at the foot, 1 at the tip.
        let up = (tongue.y - py as f32) / tongue.height;
        if !(0.0..1.0).contains(&up) {
            continue;
        }
        // The tongue narrows to its tip and sways more the higher it gets.
        let below = 1.0 - up;
        let half = (tongue.width * 0.5 * below.powf(TAPER) * (1.0 - NECK + NECK * below)).max(0.5);
        let wave = (up * SWAY_WAVES - time * SWAY_RATE + tongue.phase).sin();
        let center = tongue.x + wave * tongue.width * SWAY_SHARE * up;
        let falloff = below.powf(TIP_FALLOFF);
        let level = falloff * brightness;
        // Include peak turbulence so bright tips are never clipped early.
        let peak = level * MAX_TURBULENCE;
        if peak <= HEAT_CUTOFF {
            continue;
        }
        let reach = half * (1.0 - HEAT_CUTOFF / peak).max(0.0).sqrt();
        let first = ((center - reach - 0.5).floor() - 1.0).max(left as f32) as u32;
        let last = (((center + reach - 0.5).ceil() + 1.0)
            .min(right as f32)
            .max(0.0)) as u32;
        let ny = (py as f32 + scroll) * inv_turb_h;
        let row_offset = (py * width) as usize;
        let mut row_hit = false;
        for px in first..last {
            let across = ((px as f32 + 0.5 - center).abs() / half).min(1.0);
            let n = noise::value_noise(px as f32 * inv_turb_w, ny, 0, tongue.seed);
            let heat = ((1.0 - across * across) * level * (0.45 + 1.1 * n)).clamp(0.0, 1.0);
            if heat <= HEAT_CUTOFF {
                continue;
            }
            let fire_px = fire(heat);
            let pixel = &mut pixels[row_offset + px as usize];
            *pixel = add(*pixel, fire_px);
            if let Some((q, q_top, q_cols)) = glow_acc.as_mut() {
                let qy = (py / GLOW_STEP - *q_top) as usize;
                let slot = &mut q[qy * *q_cols + (px / GLOW_STEP) as usize];
                slot[0] += ((fire_px >> 24) & 0xff) as f32;
                slot[1] += ((fire_px >> 16) & 0xff) as f32;
                slot[2] += ((fire_px >> 8) & 0xff) as f32;
                slot[3] += (fire_px & 0xff) as f32;
            }
            row_hit = true;
        }
        if row_hit {
            lit_top = lit_top.min(py);
            lit_bottom = lit_bottom.max(py + 1);
        }
    }
    if lit_top >= lit_bottom {
        Rows::NONE
    } else {
        Rows {
            top: lit_top,
            bottom: lit_bottom,
        }
    }
}

/// Blurs the accumulated flame light in `scratch.q0` (covering quarter rows
/// `q_top..q_bottom`) with two separable radius-3 box passes and adds
/// `glow * GLOW_GAIN` directly onto `pixels`.
fn apply_glow(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    (q_top, q_bottom, full_q_rows): (u32, u32, usize),
    scratch: &mut GlowScratch,
) -> Rows {
    let q_cols = width.div_ceil(GLOW_STEP) as usize;
    let q_rows = (q_bottom - q_top) as usize;
    if q_cols == 0 || q_rows == 0 || full_q_rows == 0 {
        return Rows::NONE;
    }
    let q_len = q_cols * q_rows;
    scratch.q1.resize(q_len, [0.0; 4]);
    scratch.col_acc.resize(q_cols, [0.0; 4]);
    scratch.q_px.resize(q_len, 0);
    scratch.vert.resize(q_cols, 0);

    let inv_block = 1.0 / (GLOW_STEP * GLOW_STEP) as f32;
    let mut any_src = false;
    for cell in &mut scratch.q0 {
        if cell[0] == 0.0 {
            continue;
        }
        any_src = true;
        cell[0] = (cell[0] * inv_block).min(255.0);
        cell[1] = (cell[1] * inv_block).min(cell[0]);
        cell[2] = (cell[2] * inv_block).min(cell[0]);
        cell[3] = (cell[3] * inv_block).min(cell[0]);
    }
    if !any_src {
        return Rows::NONE;
    }

    box_blur_2d(
        &mut scratch.q0,
        &mut scratch.q1,
        &mut scratch.col_acc,
        q_cols,
        q_rows,
    );
    box_blur_2d(
        &mut scratch.q0,
        &mut scratch.q1,
        &mut scratch.col_acc,
        q_cols,
        q_rows,
    );

    for (dst, src) in scratch.q_px.iter_mut().zip(scratch.q0.iter()) {
        let a = (src[0] * GLOW_GAIN).round().clamp(0.0, 255.0);
        if a < 1.0 {
            *dst = 0;
            continue;
        }
        let r = (src[1] * GLOW_GAIN).round().clamp(0.0, a) as u32;
        let g = (src[2] * GLOW_GAIN).round().clamp(0.0, a) as u32;
        let b = (src[3] * GLOW_GAIN).round().clamp(0.0, a) as u32;
        *dst = ((a as u32) << 24) | (r << 16) | (g << 8) | b;
    }

    let out_top = (q_top * GLOW_STEP).saturating_sub(2);
    let out_bottom = (q_bottom * GLOW_STEP + 2).min(height);
    let mut lit_top = u32::MAX;
    let mut lit_bottom = 0u32;

    for py in out_top..out_bottom {
        let fy = ((py as f32 + 0.5) * 0.25 - 0.5).clamp(0.0, (full_q_rows - 1) as f32);
        let qy0 = fy.floor() as u32;
        let qy1 = (qy0 + 1).min(full_q_rows as u32 - 1);
        let wy = ((fy - qy0 as f32) * 256.0).round() as u32;

        if !build_glow_vert_row(
            &scratch.q_px,
            &mut scratch.vert,
            q_cols,
            (q_top, q_bottom),
            (qy0, qy1, wy),
        ) {
            continue;
        }

        let dst_start = py as usize * width as usize;
        let dst_row = &mut pixels[dst_start..dst_start + width as usize];
        if composite_glow_row(dst_row, &scratch.vert, q_cols) {
            lit_top = lit_top.min(py);
            lit_bottom = lit_bottom.max(py + 1);
        }
    }

    if lit_top >= lit_bottom {
        Rows::NONE
    } else {
        Rows {
            top: lit_top,
            bottom: lit_bottom,
        }
    }
}

fn box_blur_2d(
    buf: &mut [[f32; 4]],
    tmp: &mut [[f32; 4]],
    col_acc: &mut [[f32; 4]],
    cols: usize,
    rows: usize,
) {
    let inv_w = 1.0 / (GLOW_RADIUS * 2 + 1) as f32;
    for r in 0..rows {
        let src_row = &buf[r * cols..(r + 1) * cols];
        let dst_row = &mut tmp[r * cols..(r + 1) * cols];
        let mut acc = [0.0f32; 4];
        for &p in src_row.iter().take((GLOW_RADIUS + 1).min(cols)) {
            acc[0] += p[0];
            acc[1] += p[1];
            acc[2] += p[2];
            acc[3] += p[3];
        }
        for (c, dst) in dst_row.iter_mut().enumerate() {
            *dst = [
                acc[0] * inv_w,
                acc[1] * inv_w,
                acc[2] * inv_w,
                acc[3] * inv_w,
            ];
            let sub = c.checked_sub(GLOW_RADIUS);
            let add_idx = c + GLOW_RADIUS + 1;
            if let Some(s) = sub {
                let p = src_row[s];
                acc[0] -= p[0];
                acc[1] -= p[1];
                acc[2] -= p[2];
                acc[3] -= p[3];
            }
            if add_idx < cols {
                let p = src_row[add_idx];
                acc[0] += p[0];
                acc[1] += p[1];
                acc[2] += p[2];
                acc[3] += p[3];
            }
        }
    }

    col_acc.fill([0.0; 4]);
    for r in 0..=GLOW_RADIUS.min(rows.saturating_sub(1)) {
        let row = &tmp[r * cols..(r + 1) * cols];
        for (acc, p) in col_acc.iter_mut().zip(row.iter()) {
            acc[0] += p[0];
            acc[1] += p[1];
            acc[2] += p[2];
            acc[3] += p[3];
        }
    }
    for r in 0..rows {
        let dst_row = &mut buf[r * cols..(r + 1) * cols];
        for (dst, acc) in dst_row.iter_mut().zip(col_acc.iter()) {
            *dst = [
                acc[0] * inv_w,
                acc[1] * inv_w,
                acc[2] * inv_w,
                acc[3] * inv_w,
            ];
        }
        if let Some(sub_r) = r.checked_sub(GLOW_RADIUS) {
            let row = &tmp[sub_r * cols..(sub_r + 1) * cols];
            for (acc, p) in col_acc.iter_mut().zip(row.iter()) {
                acc[0] -= p[0];
                acc[1] -= p[1];
                acc[2] -= p[2];
                acc[3] -= p[3];
            }
        }
        let add_r = r + GLOW_RADIUS + 1;
        if add_r < rows {
            let row = &tmp[add_r * cols..(add_r + 1) * cols];
            for (acc, p) in col_acc.iter_mut().zip(row.iter()) {
                acc[0] += p[0];
                acc[1] += p[1];
                acc[2] += p[2];
                acc[3] += p[3];
            }
        }
    }
}

fn build_glow_vert_row(
    q_px: &[u32],
    vert: &mut [u32],
    q_cols: usize,
    (q_top, q_bottom): (u32, u32),
    (qy0, qy1, wy): (u32, u32, u32),
) -> bool {
    let row0 = (qy0 >= q_top && qy0 < q_bottom)
        .then(|| &q_px[(qy0 - q_top) as usize * q_cols..(qy0 - q_top + 1) as usize * q_cols]);
    let row1 = (qy1 >= q_top && qy1 < q_bottom)
        .then(|| &q_px[(qy1 - q_top) as usize * q_cols..(qy1 - q_top + 1) as usize * q_cols]);
    let mut any = 0u32;
    match (row0, row1) {
        (Some(r0), Some(r1)) => {
            for (dst, (&a, &b)) in vert.iter_mut().zip(r0.iter().zip(r1.iter())) {
                let v = if (a | b) == 0 { 0 } else { mix_swar(a, b, wy) };
                *dst = v;
                any |= v;
            }
        }
        (Some(r0), None) => {
            for (dst, &a) in vert.iter_mut().zip(r0.iter()) {
                let v = if a == 0 { 0 } else { mix_swar(a, 0, wy) };
                *dst = v;
                any |= v;
            }
        }
        (None, Some(r1)) => {
            for (dst, &b) in vert.iter_mut().zip(r1.iter()) {
                let v = if b == 0 { 0 } else { mix_swar(0, b, wy) };
                *dst = v;
                any |= v;
            }
        }
        (None, None) => {
            vert.fill(0);
        }
    }
    any != 0
}

fn composite_glow_row(dst_row: &mut [u32], vert: &[u32], q_cols: usize) -> bool {
    const WX: [u32; 4] = [32, 96, 160, 224];
    let width = dst_row.len();
    let max_qx = q_cols.saturating_sub(1);
    let mut row_hit = false;

    let lead = width.min(2);
    if lead > 0 {
        let v0 = vert[0];
        if v0 != 0 {
            row_hit = true;
            for dst in dst_row.iter_mut().take(lead) {
                *dst = add(*dst, v0);
            }
        }
    }

    let mut px = 2usize;
    let mut qx = 0usize;
    while px < width {
        let end = (px + 4).min(width);
        let left = vert[qx];
        let right = vert[(qx + 1).min(max_qx)];
        if (left | right) != 0 {
            row_hit = true;
            for (slot, i) in (px..end).enumerate() {
                let glow = mix_swar(left, right, WX[slot]);
                dst_row[i] = add(dst_row[i], glow);
            }
        }
        px += 4;
        qx = (qx + 1).min(max_qx);
    }
    row_hit
}

#[inline(always)]
fn mix_swar(a: u32, b: u32, w: u32) -> u32 {
    const MASK: u32 = 0x00FF_00FF;
    let inv = 256 - w;
    let rb = ((((a & MASK) * inv) + ((b & MASK) * w)) >> 8) & MASK;
    let ag = (((((a >> 8) & MASK) * inv) + (((b >> 8) & MASK) * w)) >> 8) & MASK;
    rb | (ag << 8)
}

/// A spark drifting up from a tongue, fading as it rises. Returns the rows
/// it lit.
fn paint_ember(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    tongue: &Tongue,
    time: f32,
    index: usize,
) -> Rows {
    let offset = tongue.phase * EMBER_PHASE_SHARE
        + noise::random(tongue.seed, index as u32) * EMBER_ORDER_SHARE;
    let along = (time * EMBER_RATE + offset).fract().abs();
    let drift = (time * EMBER_DRIFT_RATE + tongue.phase).sin();
    let x = tongue.x + drift * tongue.width * EMBER_DRIFT_SHARE;
    let y = tongue.y - tongue.height * (EMBER_START + EMBER_RISE * along);
    let fade = 1.0 - along;
    if x < 0.0 || y < 0.0 {
        return Rows::NONE;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= width || y >= height {
        return Rows::NONE;
    }
    let size = ember_px(tongue.scale);
    for dy in 0..size {
        for dx in 0..size {
            let (px, py) = (x + dx, y + dy);
            if px < width && py < height {
                let pixel = &mut pixels[(py * width + px) as usize];
                *pixel = add(*pixel, fire(EMBER_HEAT * fade));
            }
        }
    }
    Rows {
        top: y,
        bottom: (y + size).min(height),
    }
}

/// The side of an ember's square at `scale`, in pixels: two at 100 %, three
/// at 200 %, never less than one.
fn ember_px(scale: f32) -> u32 {
    (2.0 * scale).round().max(1.0) as u32
}

/// Fire's color at `heat` (0 to 1), premultiplied, faint where it is cool.
pub(crate) fn fire(heat: f32) -> u32 {
    let heat = heat.clamp(0.0, 1.0);
    let (r, g, b) = fire_color(heat);
    let alpha = (heat * FIRE_ALPHA_GAIN).min(1.0);
    let channel = |value: f32| (value * alpha).round() as u32;
    ((alpha * 255.0).round() as u32) << 24 | channel(r) << 16 | channel(g) << 8 | channel(b)
}

/// `FIRE_RAMP`'s color at `heat` (0 to 1), straight color.
fn fire_color(heat: f32) -> (f32, f32, f32) {
    let upper = FIRE_RAMP
        .iter()
        .position(|(at, _)| heat < *at)
        .unwrap_or(FIRE_RAMP.len() - 1)
        .max(1);
    let ((from_at, from), (to_at, to)) = (FIRE_RAMP[upper - 1], FIRE_RAMP[upper]);
    let t = (heat - from_at) / (to_at - from_at);
    (
        math::lerp(from.0, to.0, t),
        math::lerp(from.1, to.1, t),
        math::lerp(from.2, to.2, t),
    )
}

/// Adds two premultiplied pixels channel by channel, capping each byte at 255.
/// Because each source channel is at most its alpha, the sum stays premultiplied.
pub(crate) fn add(a: u32, b: u32) -> u32 {
    let [a0, a1, a2, a3] = a.to_ne_bytes();
    let [b0, b1, b2, b3] = b.to_ne_bytes();
    u32::from_ne_bytes([
        a0.saturating_add(b0),
        a1.saturating_add(b1),
        a2.saturating_add(b2),
        a3.saturating_add(b3),
    ])
}

/// A column's fixed flicker phase for `seed`, 0 to 2π.
fn phase_of(seed: u32, col: u32) -> f32 {
    math::unit(math::hash(col, seed, 0)) * std::f32::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::burn;

    fn base(col: u32, row: u32) -> FlameBase {
        FlameBase {
            col,
            row,
            heat: 1.0,
        }
    }

    #[test]
    fn the_overlay_leaves_room_beside_and_above_the_space() {
        let (side, top) = margins(96);
        assert_eq!((side, top), (24, 192));
        assert_eq!(overlay_size(300, 200, 96), (300 + 2 * side, 200 + top));
        assert_eq!(overlay_size(1, 1, 96), (1 + 2 * side, 1 + top));
        assert_eq!(margins(144), (36, 288), "1.5 times the 96-DPI margins");
        assert_eq!(margins(192), (48, 384));
        assert_eq!(margins(1), (0, 2), "rounds down to a whole pixel");
    }

    #[test]
    fn tongues_stand_on_their_bases_inside_the_overlay() {
        let (side, top) = margins(96);
        let cell = burn::cell_px(96);
        let mut out = Vec::new();
        tongues(&[base(0, 10), base(20, 4)], 200.0, 96, 0.5, 1, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].y, (top + 11 * cell) as f32);
        assert!(out[0].x > side as f32);
        for tongue in &out {
            assert!((HEIGHT_RANGE.0 * 0.4..=HEIGHT_RANGE.1).contains(&tongue.height));
            assert!(tongue.height < tongue.y, "room above to rise into");
        }
    }

    #[test]
    fn the_flames_grow_with_the_display_scale() {
        // The same space on a 200 % display: twice the pixels, twice the
        // cells, and the same base index, so the tongue should be twice the
        // size at twice the distance from the overlay's corner.
        let mut at_100 = Vec::new();
        let mut at_200 = Vec::new();
        tongues(&[base(4, 10)], 200.0, 96, 0.3, 1, &mut at_100);
        tongues(&[base(4, 10)], 400.0, 192, 0.3, 1, &mut at_200);
        let (small, large) = (at_100[0], at_200[0]);
        assert!((large.height - 2.0 * small.height).abs() < 1e-3);
        assert!((large.width - 2.0 * small.width).abs() < 1e-3);
        assert!((large.x - 2.0 * small.x).abs() < 1e-3);
        assert!((large.y - 2.0 * small.y).abs() < 1e-3);
        assert_eq!(ember_px(1.0), 2);
        assert_eq!(ember_px(2.0), 4);
        assert_eq!(ember_px(0.4), 1, "never a zero-pixel ember");
    }

    #[test]
    fn a_tongue_is_bright_at_its_foot_and_nothing_elsewhere() {
        let (width, height) = (120, 140);
        let mut pixels = vec![7; (width * height) as usize];
        let tongue = Tongue {
            x: 60.0,
            y: 100.0,
            height: 50.0,
            width: 20.0,
            heat: 1.0,
            phase: 0.0,
            scale: 1.0,
            seed: 1,
        };
        let mut drawn = Rows::ALL;
        paint(&mut pixels, width, height, &[tongue], 0.0, &mut drawn);
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        let foot_alpha = (95..100).map(|y| at(60, y) >> 24).max().unwrap_or(0);
        assert!(foot_alpha > 200, "the foot burns");
        assert_eq!(at(5, 5), 0, "far away stays clear");
        assert!(
            (at(60, 105) >> 24) > (at(60, 114) >> 24),
            "glow fades away below the foot"
        );
        assert_eq!(at(60, 130), 0, "nothing past the glow below the foot");
        for pixel in pixels {
            let alpha = pixel >> 24;
            for shift in [0, 8, 16] {
                assert!((pixel >> shift) & 0xFF <= alpha, "premultiplied");
            }
        }
    }

    #[test]
    fn the_same_time_paints_the_same_pixels() {
        let (width, height) = (80, 80);
        let tongue = Tongue {
            x: 40.0,
            y: 70.0,
            height: 40.0,
            width: 16.0,
            heat: 0.8,
            phase: 1.0,
            scale: 1.0,
            seed: 3,
        };
        let mut first = vec![0; (width * height) as usize];
        let mut second = vec![1; (width * height) as usize];
        let (mut drawn_first, mut drawn_second) = (Rows::ALL, Rows::ALL);
        paint(&mut first, width, height, &[tongue], 0.7, &mut drawn_first);
        paint(
            &mut second,
            width,
            height,
            &[tongue],
            0.7,
            &mut drawn_second,
        );
        assert_eq!(first, second);

        let mut later = vec![0; (width * height) as usize];
        let mut drawn_later = Rows::ALL;
        paint(&mut later, width, height, &[tongue], 1.1, &mut drawn_later);
        assert_ne!(first, later, "turbulence scrolls over time");
    }

    #[test]
    fn flames_at_the_edges_stay_inside_the_overlay() {
        let (width, height) = (40, 30);
        let mut pixels = vec![0; (width * height) as usize];
        // Feet on every edge and corner, where tongues and embers would
        // reach past the buffer.
        let feet = [
            (0.0, 0.0),
            (39.5, 0.0),
            (0.0, 29.9),
            (39.9, 29.9),
            (20.0, 15.0),
        ];
        let tongues: Vec<Tongue> = feet
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Tongue {
                x,
                y,
                height: 60.0,
                width: 25.0,
                heat: 1.0,
                phase: index as f32,
                scale: 1.0,
                seed: 2,
            })
            .collect();
        let mut drawn = Rows::ALL;
        for step in 0..50 {
            let time = step as f32 * 0.13;
            paint(&mut pixels, width, height, &tongues, time, &mut drawn);
        }
        assert_eq!(pixels.len(), (width * height) as usize);
    }

    #[test]
    fn the_fire_ramp_passes_through_each_stop() {
        for (at, color) in FIRE_RAMP {
            let (r, g, b) = fire_color(at);
            assert!(
                (r - color.0).abs() < 1e-3
                    && (g - color.1).abs() < 1e-3
                    && (b - color.2).abs() < 1e-3,
                "at {at}: {:?}",
                (r, g, b)
            );
        }
        let (_, g, _) = fire_color(0.2);
        assert!((g - 70.0).abs() < 1e-3, "halfway between the first two");
    }

    #[test]
    fn fire_runs_from_red_to_a_light_core() {
        let edge = fire(0.1);
        let core = fire(1.0);
        assert!(edge >> 24 < core >> 24);
        assert!((core & 0xFF) > (edge & 0xFF), "the core is whiter");
    }

    #[test]
    fn add_sums_channels_and_stays_premultiplied() {
        let half_a = 0x80_80_40_10;
        let half_b = 0x80_60_50_20;
        let sum = add(half_a, half_b);
        assert_eq!(sum, 0xFF_E0_90_30);
        let alpha = sum >> 24;
        for shift in [0, 8, 16] {
            let ch = (sum >> shift) & 0xFF;
            assert!(ch >= ((half_a >> shift) & 0xFF));
            assert!(ch >= ((half_b >> shift) & 0xFF));
            assert!(ch <= alpha);
        }
    }

    #[test]
    fn two_seeds_vary_tongue_heights_for_the_same_bases() {
        let bases = [base(2, 10), base(7, 10), base(12, 10)];
        let (mut first, mut second) = (Vec::new(), Vec::new());
        tongues(&bases, 240.0, 96, 0.4, 1, &mut first);
        tongues(&bases, 240.0, 96, 0.4, 2, &mut second);
        assert!(
            first
                .iter()
                .zip(&second)
                .any(|(a, b)| (a.height - b.height).abs() > 1e-3)
        );
    }

    const SHEET: (u32, u32) = (400, 200);

    /// Sets of tongues for a `SHEET`-sized buffer: a row of them, a clump
    /// that overlaps, feet on every edge and corner, and none.
    fn tongue_sets() -> Vec<Vec<Tongue>> {
        let (width, height) = (SHEET.0 as f32, SHEET.1 as f32);
        let row = (0..12).map(|index| {
            let rise = 30.0 + 3.0 * index as f32;
            Tongue {
                x: 20.0 + 31.0 * index as f32,
                y: 150.0 + 12.0 * (index % 3) as f32,
                height: rise,
                width: rise * WIDTH_SHARE,
                heat: 0.3 + 0.06 * index as f32,
                phase: 0.7 * index as f32,
                scale: 1.0,
                seed: 1,
            }
        });
        let clump = (0..5).map(|index| Tongue {
            x: 200.0 + 4.0 * index as f32,
            y: 120.0 - 3.0 * index as f32,
            height: 55.0,
            width: 24.0,
            heat: 1.0 - 0.15 * index as f32,
            phase: 1.3 * index as f32,
            scale: 1.0,
            seed: 1,
        });
        let feet = [
            (0.0, 0.0),
            (width - 0.5, 0.0),
            (0.0, height - 0.1),
            (width - 0.1, height - 0.1),
            (width / 2.0, height / 2.0),
        ];
        let edges = feet.iter().enumerate().map(|(index, &(x, y))| Tongue {
            x,
            y,
            height: 60.0,
            width: 25.0,
            heat: 1.0,
            phase: index as f32,
            scale: 1.0,
            seed: 1,
        });
        vec![row.collect(), clump.collect(), edges.collect(), Vec::new()]
    }

    #[test]
    fn the_rows_drawn_hold_every_lit_pixel() {
        let (width, height) = SHEET;
        for (index, set) in tongue_sets().iter().enumerate() {
            let mut pixels = vec![0; (width * height) as usize];
            let mut drawn = Rows::ALL;
            paint(&mut pixels, width, height, set, 0.37, &mut drawn);
            for (at, pixel) in pixels.iter().enumerate() {
                let row = at as u32 / width;
                assert!(
                    *pixel == 0 || (drawn.top..drawn.bottom).contains(&row),
                    "set {index}: row {row} is lit outside {drawn:?}"
                );
            }
            assert!(drawn.bottom <= height, "set {index}: {drawn:?}");
            assert_eq!(drawn == Rows::NONE, set.is_empty(), "set {index}");
        }
    }

    #[test]
    fn only_the_rows_last_drawn_are_cleared() {
        let (width, height) = (120, 120);
        let mut pixels = vec![0; (width * height) as usize];
        let mut drawn = Rows::ALL;
        let tongue = Tongue {
            x: 60.0,
            y: 80.0,
            height: 30.0,
            width: 12.0,
            heat: 1.0,
            phase: 0.0,
            scale: 1.0,
            seed: 1,
        };
        paint(&mut pixels, width, height, &[tongue], 0.0, &mut drawn);
        assert!(drawn.bottom < height, "the flames stop above the last row");
        // A pixel the frame did not draw, so the next one owes it nothing.
        let marker = (drawn.bottom * width + 5) as usize;
        pixels[marker] = 0xFFFF_FFFF;
        paint(&mut pixels, width, height, &[], 0.0, &mut drawn);
        assert_eq!(pixels[marker], 0xFFFF_FFFF, "rows not drawn are left alone");
        pixels[marker] = 0;
        assert!(
            pixels.iter().all(|pixel| *pixel == 0),
            "the flames are gone"
        );
        assert_eq!(drawn, Rows::NONE);
    }

    #[test]
    fn rows_join_into_one_run_and_stay_inside_the_buffer() {
        let run = |top, bottom| Rows { top, bottom };
        let joined = run(4, 9).union(run(20, 22));
        assert_eq!(joined, run(4, 22));
        assert_eq!((joined.top(), joined.bottom()), (4, 22));
        assert!(!joined.is_empty());
        assert_eq!(run(20, 22).union(run(4, 9)), run(4, 22));
        assert_eq!(run(4, 9).union(Rows::NONE), run(4, 9));
        assert_eq!(Rows::NONE.union(run(4, 9)), run(4, 9));
        assert_eq!(Rows::NONE.union(run(9, 4)), Rows::NONE, "an empty run");
        assert!(Rows::NONE.is_empty());

        // Three pixels wide, two rows.
        let mut pixels = [1; 6];
        run(1, 50).clear(&mut pixels, 3, 2);
        assert_eq!(pixels, [1, 1, 1, 0, 0, 0]);
        run(7, 9).clear(&mut pixels, 3, 2);
        run(9, 4).clear(&mut pixels, 3, 2);
        assert_eq!(pixels, [1, 1, 1, 0, 0, 0]);
        Rows::ALL.clear(&mut pixels, 3, 2);
        assert_eq!(pixels, [0; 6]);
    }

    #[test]
    fn empty_buffer_produces_no_glow() {
        let (width, height) = (64, 64);
        let mut pixels = vec![0u32; (width * height) as usize];
        let (q_top, q_bottom, full_q_rows) = glow_quarter_bounds(Rows::new(16, 24), height);
        let q_cols = width.div_ceil(GLOW_STEP) as usize;
        let mut scratch = GlowScratch {
            q0: vec![[0.0; 4]; q_cols * (q_bottom - q_top) as usize],
            ..GlowScratch::default()
        };
        let rows = apply_glow(
            &mut pixels,
            width,
            height,
            (q_top, q_bottom, full_q_rows),
            &mut scratch,
        );
        assert_eq!(rows, Rows::NONE);
        assert!(pixels.iter().all(|&px| px == 0));

        let mut drawn = Rows::NONE;
        paint(&mut pixels, width, height, &[], 0.0, &mut drawn);
        assert_eq!(drawn, Rows::NONE);
        assert!(pixels.iter().all(|&px| px == 0));
    }

    #[test]
    fn bright_spot_glows_evenly_and_keeps_pixels_premultiplied() {
        let (width, height) = (64, 64);
        let mut pixels = vec![0u32; (width * height) as usize];
        let (q_top, q_bottom, full_q_rows) = glow_quarter_bounds(Rows::new(28, 32), height);
        let q_cols = width.div_ceil(GLOW_STEP) as usize;
        let mut scratch = GlowScratch {
            q0: vec![[0.0; 4]; q_cols * (q_bottom - q_top) as usize],
            ..GlowScratch::default()
        };
        let c = fire(1.0);
        let qy = (7 - q_top) as usize;
        scratch.q0[qy * q_cols + 7] = [
            ((c >> 24) & 0xff) as f32 * 16.0,
            ((c >> 16) & 0xff) as f32 * 16.0,
            ((c >> 8) & 0xff) as f32 * 16.0,
            (c & 0xff) as f32 * 16.0,
        ];
        let rows = apply_glow(
            &mut pixels,
            width,
            height,
            (q_top, q_bottom, full_q_rows),
            &mut scratch,
        );
        assert!(!rows.is_empty());

        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        let left = at(22, 29) >> 24;
        let right = at(37, 29) >> 24;
        let above = at(29, 22) >> 24;
        let below = at(29, 37) >> 24;
        assert!(left > 0 && right > 0 && above > 0 && below > 0);
        assert_eq!(left, right);
        assert_eq!(above, below);
        assert_eq!(left, above);
        assert_eq!(at(0, 0), 0);
        assert_eq!(at(63, 63), 0);
        assert!(pixels.iter().all(|&px| {
            let a = px >> 24;
            ((px >> 16) & 0xff) <= a && ((px >> 8) & 0xff) <= a && (px & 0xff) <= a
        }));
    }

    #[test]
    fn flame_glow_warms_neighbours_outside_tongue_outline() {
        let (width, height) = (100, 100);
        let mut pixels = vec![0u32; (width * height) as usize];
        let mut drawn = Rows::ALL;
        let tongue = Tongue {
            x: 50.0,
            y: 60.0,
            height: 30.0,
            width: 16.0,
            heat: 1.0,
            phase: 0.0,
            scale: 1.0,
            seed: 1,
        };
        paint(&mut pixels, width, height, &[tongue], 0.2, &mut drawn);
        let below_foot = pixels[(64 * width + 50) as usize];
        assert!(
            (below_foot >> 24) > 0,
            "halo should reach below the tongue's foot"
        );
        assert!(pixels.iter().all(|&px| {
            let a = px >> 24;
            ((px >> 16) & 0xff) <= a && ((px >> 8) & 0xff) <= a && (px & 0xff) <= a
        }));
    }
}
