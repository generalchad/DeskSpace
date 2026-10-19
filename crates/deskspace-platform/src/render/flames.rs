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

/// Shades `tongues` at `time` (seconds) into `pixels`, `width` by `height`
/// premultiplied BGRA (`0xAARRGGBB`), cleared first. Overlapping fire keeps
/// the brightest of each channel.
pub fn paint(pixels: &mut [u32], width: u32, height: u32, tongues: &[Tongue], time: f32) {
    pixels.fill(0);
    for tongue in tongues {
        paint_tongue(pixels, width, height, tongue, time);
    }
    for (index, tongue) in tongues.iter().enumerate().step_by(EMBER_EVERY) {
        paint_ember(pixels, width, height, tongue, time, index);
    }
}

fn paint_tongue(pixels: &mut [u32], width: u32, height: u32, tongue: &Tongue, time: f32) {
    let left = (tongue.x - tongue.width).floor().max(0.0) as u32;
    let right = ((tongue.x + tongue.width).ceil() as u32).min(width);
    let top = (tongue.y - tongue.height).floor().max(0.0) as u32;
    let bottom = (tongue.y.ceil() as u32).min(height);
    for py in top..bottom {
        // 0 at the foot, 1 at the tip.
        let up = (tongue.y - py as f32) / tongue.height;
        if !(0.0..1.0).contains(&up) {
            continue;
        }
        // The tongue narrows to its tip and sways more the higher it gets.
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

/// A spark drifting up from a tongue, fading as it rises.
fn paint_ember(
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
        paint(&mut pixels, width, height, &[tongue], 0.0);
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
        paint(&mut first, width, height, &[tongue], 0.7);
        paint(&mut second, width, height, &[tongue], 0.7);
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
        for step in 0..50 {
            paint(&mut pixels, width, height, &tongues, step as f32 * 0.13);
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
}
