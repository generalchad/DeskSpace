//! Burn's flames (ROADMAP D12, amended 2026-10-18): tongues of fire that
//! stand on the burning edge (`burn::BurnField::flame_bases`) and lick up
//! past the space's frame, shaded on the CPU into premultiplied pixels for a
//! layered overlay window. Pure, so the layout and the shading are tested.

use super::burn::{self, FlameBase};
use super::math;

/// How much room the overlay keeps beside and above the space for flames to
/// rise into, in pixels.
pub const MARGIN_SIDE: u32 = 24;
pub const MARGIN_TOP: u32 = 72;
/// A flame every this many burn cells along the front.
pub const SPACING: u32 = 5;

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
}

/// The tongues standing on `bases` at `time` (seconds) for a space
/// `space_height` tall, into `tongues`. The space's top-left is at
/// (`MARGIN_SIDE`, `MARGIN_TOP`) in the overlay.
pub fn tongues(bases: &[FlameBase], space_height: f32, time: f32, tongues: &mut Vec<Tongue>) {
    tongues.clear();
    let tallest = (space_height * HEIGHT_SHARE).clamp(HEIGHT_RANGE.0, HEIGHT_RANGE.1);
    tongues.extend(bases.iter().map(|base| {
        let phase = phase_of(base.col);
        let waves = (time * FLICKER_RATES.0 + phase).sin()
            * (time * FLICKER_RATES.1 + phase * FLICKER_PHASE_SPREAD).cos();
        let flicker = FLICKER_BASE + FLICKER_DEPTH * waves;
        let height = tallest * (1.0 - HEAT_HEIGHT + HEAT_HEIGHT * base.heat) * flicker;
        Tongue {
            x: MARGIN_SIDE as f32 + (base.col * burn::CELL) as f32 + burn::CELL as f32 / 2.0,
            y: MARGIN_TOP as f32 + (base.row * burn::CELL) as f32 + burn::CELL as f32,
            height,
            width: height * WIDTH_SHARE,
            heat: base.heat,
            phase,
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
    const NONE: Self = Self { top: 0, bottom: 0 };

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
    fn clear(self, pixels: &mut [u32], width: u32, height: u32) {
        if self.is_empty() {
            return;
        }
        let start_of = |row: u32| row.min(height) as usize * width as usize;
        pixels[start_of(self.top)..start_of(self.bottom)].fill(0);
    }
}

/// Shades `tongues` at `time` (seconds) into `pixels`, `width` by `height`
/// premultiplied BGRA (`0xAARRGGBB`). Overlapping fire keeps the brightest
/// of each channel.
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
    drawn.clear(pixels, width, height);
    let mut now = Rows::NONE;
    for tongue in tongues {
        now = now.union(paint_tongue(pixels, width, height, tongue, time));
    }
    for (index, tongue) in tongues.iter().enumerate().step_by(EMBER_EVERY) {
        now = now.union(paint_ember(pixels, width, height, tongue, time, index));
    }
    *drawn = now;
}

/// Shades one tongue, and returns the rows it reaches.
fn paint_tongue(pixels: &mut [u32], width: u32, height: u32, tongue: &Tongue, time: f32) -> Rows {
    let left = (tongue.x - tongue.width).floor().max(0.0) as u32;
    let right = ((tongue.x + tongue.width).ceil() as u32).min(width);
    let top = (tongue.y - tongue.height).floor().max(0.0) as u32;
    let bottom = (tongue.y.ceil() as u32).min(height);
    let brightness = 1.0 - HEAT_BRIGHTNESS + HEAT_BRIGHTNESS * tongue.heat;
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
        for px in left..right {
            let across = ((px as f32 + 0.5 - center).abs() / half).min(1.0);
            // Kept as two factors: one product of the row's falloff and the
            // tongue's brightness would round differently in its last bit.
            let heat = (1.0 - across * across) * falloff * brightness;
            if heat <= HEAT_CUTOFF {
                continue;
            }
            let pixel = &mut pixels[(py * width + px) as usize];
            *pixel = brightest(*pixel, fire(heat));
        }
    }
    Rows { top, bottom }
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
    let offset = tongue.phase * EMBER_PHASE_SHARE + index as f32 * EMBER_ORDER_SHARE;
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
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let (px, py) = (x + dx, y + dy);
        if px < width && py < height {
            let pixel = &mut pixels[(py * width + px) as usize];
            *pixel = brightest(*pixel, fire(EMBER_HEAT * fade));
        }
    }
    Rows {
        top: y,
        bottom: (y + 2).min(height),
    }
}

/// Fire's color at `heat` (0 to 1), premultiplied, faint where it is cool.
fn fire(heat: f32) -> u32 {
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

/// The brighter of two premultiplied pixels, channel by channel, so it stays
/// premultiplied.
fn brightest(a: u32, b: u32) -> u32 {
    (0..4)
        .map(|byte| byte * 8)
        .map(|shift| ((a >> shift) & 0xFF).max((b >> shift) & 0xFF) << shift)
        .sum()
}

/// A column's fixed flicker phase, 0 to 2π.
fn phase_of(col: u32) -> f32 {
    math::unit(math::hash(col, 0, 0)) * std::f32::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(col: u32, row: u32) -> FlameBase {
        FlameBase {
            col,
            row,
            heat: 1.0,
        }
    }

    #[test]
    fn tongues_stand_on_their_bases_inside_the_overlay() {
        let mut out = Vec::new();
        tongues(&[base(0, 10), base(20, 4)], 200.0, 0.5, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].y, (MARGIN_TOP + 11 * burn::CELL) as f32);
        assert!(out[0].x > MARGIN_SIDE as f32);
        for tongue in &out {
            assert!((HEIGHT_RANGE.0 * 0.4..=HEIGHT_RANGE.1).contains(&tongue.height));
            assert!(tongue.height < tongue.y, "room above to rise into");
        }
    }

    #[test]
    fn a_tongue_is_bright_at_its_foot_and_nothing_elsewhere() {
        let (width, height) = (120, 120);
        let mut pixels = vec![7; (width * height) as usize];
        let tongue = Tongue {
            x: 60.0,
            y: 100.0,
            height: 50.0,
            width: 20.0,
            heat: 1.0,
            phase: 0.0,
        };
        let mut drawn = Rows::ALL;
        paint(&mut pixels, width, height, &[tongue], 0.0, &mut drawn);
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        assert!(at(60, 97) >> 24 > 200, "the foot burns");
        assert_eq!(at(5, 5), 0, "far away stays clear");
        assert_eq!(at(60, 110), 0, "nothing below the foot");
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
        assert_eq!(brightest(0x10_20_30_40, 0x40_10_50_00), 0x40_20_50_40);
    }

    /// `paint` as it was while it cleared the whole buffer and worked out
    /// the row's and the tongue's values for every pixel: the pixels every
    /// frame still has to give.
    fn reference_paint(pixels: &mut [u32], width: u32, height: u32, tongues: &[Tongue], time: f32) {
        pixels.fill(0);
        for tongue in tongues {
            reference_tongue(pixels, width, height, tongue, time);
        }
        for (index, tongue) in tongues.iter().enumerate().step_by(EMBER_EVERY) {
            reference_ember(pixels, width, height, tongue, time, index);
        }
    }

    fn reference_tongue(pixels: &mut [u32], width: u32, height: u32, tongue: &Tongue, time: f32) {
        let left = (tongue.x - tongue.width).floor().max(0.0) as u32;
        let right = ((tongue.x + tongue.width).ceil() as u32).min(width);
        let top = (tongue.y - tongue.height).floor().max(0.0) as u32;
        let bottom = (tongue.y.ceil() as u32).min(height);
        for py in top..bottom {
            let up = (tongue.y - py as f32) / tongue.height;
            if !(0.0..1.0).contains(&up) {
                continue;
            }
            let below = 1.0 - up;
            let half = tongue.width * 0.5 * below.powf(TAPER) * (1.0 - NECK + NECK * below);
            let wave = (up * SWAY_WAVES - time * SWAY_RATE + tongue.phase).sin();
            let center = tongue.x + wave * tongue.width * SWAY_SHARE * up;
            let brightness = 1.0 - HEAT_BRIGHTNESS + HEAT_BRIGHTNESS * tongue.heat;
            for px in left..right {
                let across = ((px as f32 + 0.5 - center).abs() / half.max(0.5)).min(1.0);
                let heat = (1.0 - across * across) * below.powf(TIP_FALLOFF) * brightness;
                if heat <= HEAT_CUTOFF {
                    continue;
                }
                let pixel = &mut pixels[(py * width + px) as usize];
                *pixel = brightest(*pixel, fire(heat));
            }
        }
    }

    fn reference_ember(
        pixels: &mut [u32],
        width: u32,
        height: u32,
        tongue: &Tongue,
        time: f32,
        index: usize,
    ) {
        let offset = tongue.phase * EMBER_PHASE_SHARE + index as f32 * EMBER_ORDER_SHARE;
        let along = (time * EMBER_RATE + offset).fract().abs();
        let drift = (time * EMBER_DRIFT_RATE + tongue.phase).sin();
        let x = tongue.x + drift * tongue.width * EMBER_DRIFT_SHARE;
        let y = tongue.y - tongue.height * (EMBER_START + EMBER_RISE * along);
        let fade = 1.0 - along;
        if x < 0.0 || y < 0.0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (px, py) = (x + dx, y + dy);
            if px < width && py < height {
                let pixel = &mut pixels[(py * width + px) as usize];
                *pixel = brightest(*pixel, fire(EMBER_HEAT * fade));
            }
        }
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
            }
        });
        let clump = (0..5).map(|index| Tongue {
            x: 200.0 + 4.0 * index as f32,
            y: 120.0 - 3.0 * index as f32,
            height: 55.0,
            width: 24.0,
            heat: 1.0 - 0.15 * index as f32,
            phase: 1.3 * index as f32,
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
        });
        vec![row.collect(), clump.collect(), edges.collect(), Vec::new()]
    }

    #[test]
    fn a_frame_gives_the_reference_pixels_whatever_was_drawn_before() {
        let (width, height) = SHEET;
        let sets = tongue_sets();
        let mut expected = vec![0; (width * height) as usize];
        for (index, set) in sets.iter().enumerate() {
            for (before, earlier) in sets.iter().enumerate() {
                for time in [0.0, 0.37, 1.9, 12.5] {
                    let mut pixels = vec![7; (width * height) as usize];
                    let mut drawn = Rows::ALL;
                    paint(&mut pixels, width, height, earlier, time + 0.4, &mut drawn);
                    paint(&mut pixels, width, height, set, time, &mut drawn);
                    reference_paint(&mut expected, width, height, set, time);
                    assert!(
                        pixels == expected,
                        "set {index} at {time} s, after set {before}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_burn_through_one_buffer_gives_the_reference_frames() {
        let space = (300.0, 200.0);
        let (width, height) = (300 + 2 * MARGIN_SIDE, 200 + MARGIN_TOP);
        let field = burn::BurnField::for_space(space, 5, 0);
        let (mut bases, mut flames) = (Vec::new(), Vec::new());
        let mut pixels = vec![7; (width * height) as usize];
        let mut expected = vec![0; (width * height) as usize];
        let mut drawn = Rows::ALL;
        let mut burning = 0;
        for frame in 0..60 {
            let time = frame as f32 / 60.0;
            field.flame_bases(burn::front(frame as f32 / 59.0), SPACING, &mut bases);
            tongues(&bases, space.1, time, &mut flames);
            burning += usize::from(!flames.is_empty());
            paint(&mut pixels, width, height, &flames, time, &mut drawn);
            reference_paint(&mut expected, width, height, &flames, time);
            assert!(pixels == expected, "frame {frame}");
        }
        assert!(burning > 30, "most frames have flames to compare");
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
            y: 100.0,
            height: 30.0,
            width: 12.0,
            heat: 1.0,
            phase: 0.0,
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
}
