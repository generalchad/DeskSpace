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
        let flicker = 0.78 + 0.22 * (time * 9.0 + phase).sin() * (time * 5.3 + phase * 1.7).cos();
        let height = tallest * (0.55 + 0.45 * base.heat) * flicker;
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
    for (index, tongue) in tongues.iter().enumerate().step_by(3) {
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
        let half = tongue.width * 0.5 * (1.0 - up).powf(0.6) * (0.55 + 0.45 * (1.0 - up));
        let sway = (up * 4.5 - time * 7.0 + tongue.phase).sin() * tongue.width * 0.3 * up;
        let center = tongue.x + sway;
        for px in left..right {
            let across = ((px as f32 + 0.5 - center).abs() / half.max(0.5)).min(1.0);
            let heat = (1.0 - across * across) * (1.0 - up).powf(0.8) * (0.6 + 0.4 * tongue.heat);
            if heat <= 0.02 {
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
    let rise = tongue.height * 2.2;
    let along = ((time * 0.9 + tongue.phase * 0.37 + index as f32 * 0.13).fract()).abs();
    let x = tongue.x + (time * 3.0 + tongue.phase).sin() * tongue.width * 0.6;
    let y = tongue.y - tongue.height * 0.4 - rise * along;
    let fade = 1.0 - along;
    if x < 0.0 || y < 0.0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let (px, py) = (x + dx, y + dy);
        if px < width && py < height {
            let pixel = &mut pixels[(py * width + px) as usize];
            *pixel = brightest(*pixel, fire(0.55 * fade));
        }
    }
}

/// Fire's color at `heat` (0 to 1), premultiplied: deep red and faint at
/// the edges, through orange, to a yellow-white core.
fn fire(heat: f32) -> u32 {
    let heat = heat.clamp(0.0, 1.0);
    let (r, g, b) = if heat < 0.4 {
        (200.0 + 55.0 * heat / 0.4, 40.0 + 60.0 * heat / 0.4, 10.0)
    } else if heat < 0.75 {
        let t = (heat - 0.4) / 0.35;
        (255.0, 100.0 + 110.0 * t, 10.0 + 30.0 * t)
    } else {
        let t = (heat - 0.75) / 0.25;
        (255.0, 210.0 + 40.0 * t, 40.0 + 150.0 * t)
    };
    let alpha = (heat * 1.6).min(1.0);
    let channel = |value: f32| (value * alpha).round() as u32;
    ((alpha * 255.0).round() as u32) << 24 | channel(r) << 16 | channel(g) << 8 | channel(b)
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
    fn fire_runs_from_red_to_a_light_core() {
        let edge = fire(0.1);
        let core = fire(1.0);
        assert!(edge >> 24 < core >> 24);
        assert!((core & 0xFF) > (edge & 0xFF), "the core is whiter");
        assert_eq!(brightest(0x10_20_30_40, 0x40_10_50_00), 0x40_20_50_40);
    }
}
