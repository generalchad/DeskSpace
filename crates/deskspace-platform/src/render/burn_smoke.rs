//! Burn's optional smoke (ROADMAP D12): a neutral, warm gray plume rising from
//! the burning edge, shaded into the same overlay as the flames. Pure, so the
//! density and shading are tested.

use super::burn::{self, BurnField};
use super::noise;
use super::smoke::{self, Canvas};

/// The smoke's thickness around the burning edge, in field units.
const THICKNESS: f32 = 0.22;
/// How much of the plume is smoke at its thickest.
const OPACITY: f32 = 0.62;

/// What the overlay keeps between frames, so Burn's smoke allocates nothing.
#[derive(Default)]
pub struct Scratch {
    cells: Vec<u32>,
}

/// Shades the smoke of `field` at `front` and `time` (seconds) into `canvas`,
/// premultiplied BGRA. Clears `canvas` when there is no smoke.
pub fn paint(canvas: Canvas<'_>, field: &BurnField, front: f32, time: f32, scratch: &mut Scratch) {
    let Canvas {
        pixels,
        width,
        height,
        origin,
    } = canvas;
    let progress = ((front + burn::BANDS) / (1.0 + burn::BANDS)).clamp(0.0, 1.0);
    let emission = smoothstep(0.0, 0.15, progress) * (1.0 - smoothstep(0.85, 1.0, progress));
    if emission <= 0.0 {
        pixels.fill(0);
        return;
    }
    let step = smoke::overlay_cell(width, height);
    let cols = width.div_ceil(step);
    let rows = height.div_ceil(step);
    scratch.cells.resize((cols * rows) as usize, 0);
    let space_width = field.cols() as f32 * burn::CELL as f32;
    let space_height = field.rows() as f32 * burn::CELL as f32;
    let scale = 18.0;
    for row in 0..rows {
        for col in 0..cols {
            let x = (col as f32 + 0.5) * step as f32;
            let y = (row as f32 + 0.5) * step as f32;
            let sx = x - origin.0 as f32;
            let sy = y - origin.1 as f32;
            let above = (space_height - sy).max(0.0) / space_height;
            let lift = above.min(1.4) * space_height * 0.28;
            let drift = noise::fractal(sx / scale + 3.1, (sy - time * 18.0) / scale, 6.0);
            let sway = (drift - 0.5) * space_width * 0.28;
            let turbulence = noise::fractal(
                (sx + sway) / (scale * 0.6),
                (sy - time * 26.0) / (scale * 0.6),
                5.0,
            );
            let distance = field.value_at(sx - sway, sy + lift) - front;
            let band = (-(distance * distance) / (THICKNESS * THICKNESS)).exp();
            let fade = 1.0 - smoothstep(1.0, 1.5, above);
            let density = (emission * band * (0.4 + 0.6 * turbulence) * fade * OPACITY).min(0.75);
            scratch.cells[(row * cols + col) as usize] = pixel(density, distance);
        }
    }
    smoke::upscale(pixels, width, height, &scratch.cells, cols, rows, step);
}

/// A smoke cell's premultiplied pixel: warm gray, warmer right at the ember
/// edge and paler as it thins.
fn pixel(density: f32, distance: f32) -> u32 {
    if density <= 0.0 {
        return 0;
    }
    let warm = (1.0 - (distance.abs() / 0.12).min(1.0)) * 0.18;
    let thin = 1.0 - density;
    let red = (0.58 + warm * 0.25 + 0.18 * thin).min(1.0);
    let green = (0.56 - warm * 0.02 + 0.18 * thin).min(1.0);
    let blue = (0.53 - warm * 0.12 + 0.20 * thin).min(1.0);
    let alpha = density.clamp(0.0, 1.0);
    let channel = |value: f32| (value * alpha * 255.0).round() as u32;
    ((alpha * 255.0).round() as u32) << 24
        | channel(red) << 16
        | channel(green) << 8
        | channel(blue)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(front: f32) -> Vec<u32> {
        let field = BurnField::new(80, 60, 5);
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
            &field,
            front,
            0.5,
            &mut scratch,
        );
        pixels
    }

    #[test]
    fn nothing_smokes_before_or_after_the_burn() {
        for front in [-burn::BANDS, 1.0] {
            assert!(painted(front).iter().all(|pixel| *pixel == 0), "{front}");
        }
    }

    #[test]
    fn smoke_is_neutral_and_premultiplied() {
        let pixels = painted(burn::front(0.5));
        let smoke: Vec<u32> = pixels.into_iter().filter(|pixel| *pixel != 0).collect();
        assert!(!smoke.is_empty(), "the burn smokes");
        for pixel in smoke {
            let alpha = pixel >> 24;
            for shift in [0, 8, 16] {
                assert!((pixel >> shift) & 0xFF <= alpha, "{pixel:#010X}");
            }
            let (red, green, blue) = ((pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF);
            assert!(
                red.abs_diff(green) < 30 && green.abs_diff(blue) < 30,
                "gray"
            );
        }
    }
}
