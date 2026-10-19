//! A space's shadow as pixels (ROADMAP D15): the soft shadow a rounded
//! rectangle casts a little below itself, cut out wherever the space covers
//! it, so a translucent space is not darkened from behind.
//! `win32/shadow_window.rs` shows it on a window of its own under the space.
//!
//! The blur is a Gaussian reaching three standard deviations. Over a
//! rounded rectangle it has a closed form along one axis and is sampled
//! along the other (Evan Wallace, "Fast Rounded Rectangle Shadows",
//! https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/). Away
//! from the corners the rectangle is a plain box, whose blur is the product
//! of one profile per axis. So the sampled form is needed only at the
//! corners, and for any shadow big enough, those come from one tile made per
//! look and mirrored four ways. A resize, or a frame of Roll, then costs a
//! profile per row and column and a copy.

use std::f32::consts::{FRAC_1_SQRT_2, PI};

use deskspace_core::geometry::Rect;

/// Samples along the sampled axis. The blur is smooth, so few are enough.
const SAMPLES: usize = 8;
/// What an outlined shadow's edge is drawn in: opaque magenta, premultiplied
/// BGRA.
const OUTLINE: u32 = 0xFFFF_00FF;

/// How a shadow looks on one window, in that window's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowLook {
    /// How far the blur reaches past the space, at least 1.
    pub reach: i32,
    /// The space's corner radius.
    pub radius: f32,
}

impl ShadowLook {
    /// How far the shadow falls below the space: a quarter of its reach, as
    /// under a light above the screen.
    pub fn drop(&self) -> i32 {
        self.reach / 4
    }

    /// Where the shadow's pixels go for a space at `space`: grown by the
    /// reach on every side, and moved down by `drop`.
    pub fn rect(&self, space: Rect) -> Rect {
        Rect::new(
            space.x - self.reach,
            space.y - self.reach + self.drop(),
            space.width + 2 * self.reach,
            space.height + 2 * self.reach,
        )
    }

    /// The Gaussian's standard deviation.
    fn sigma(&self) -> f32 {
        self.reach.max(1) as f32 / 3.0
    }

    /// How far in from the shadow's outer edge its corner reaches: the
    /// margin, the corner, and the blur's reach inside the space. Past it,
    /// along either axis, the space's edge is straight.
    fn span(&self) -> usize {
        (2 * self.reach.max(1)) as usize + self.radius.max(0.0).ceil() as usize
    }
}

/// What a shadow is drawn from: its look's corner tile, and room for the
/// per-size profiles, so drawing allocates nothing once it has grown.
#[derive(Debug, Clone)]
pub struct ShadowPieces {
    look: ShadowLook,
    span: usize,
    /// The top-left corner's blur, `span` by `span`, row by row, for a
    /// space too big for its other corners to reach it.
    corner: Vec<f32>,
    /// One axis's blur at each column and row of the last size drawn.
    columns: Vec<f32>,
    rows: Vec<f32>,
}

impl ShadowPieces {
    pub fn new(look: ShadowLook) -> Self {
        let span = look.span();
        // Any box at least two spans across: its other corners are out of
        // the blur's reach.
        let reach = look.reach.max(1) as f32;
        let side = 2.0 * span as f32;
        let caster = Caster {
            left: reach,
            top: reach,
            right: side - reach,
            bottom: side - reach,
            radius: look.radius,
            sigma: look.sigma(),
        };
        let corner = (0..span * span)
            .map(|index| caster.blur_at(pixel_center(index % span), pixel_center(index / span)))
            .collect();
        Self {
            look,
            span,
            corner,
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    pub fn look(&self) -> ShadowLook {
        self.look
    }

    /// Draws the shadow of a space `width` by `height` into `pixels`:
    /// premultiplied BGRA rows `stride` pixels apart, covering
    /// `look.rect`. With `outline`, its edge is drawn opaque, so the window
    /// can be seen.
    pub fn paint(
        &mut self,
        (width, height): (i32, i32),
        pixels: &mut [u32],
        stride: usize,
        outline: bool,
    ) {
        let reach = self.look.reach.max(1);
        let (shadow_width, shadow_height) = (
            (width.max(0) + 2 * reach) as usize,
            (height.max(0) + 2 * reach) as usize,
        );
        let radius = self
            .look
            .radius
            .clamp(0.0, width.min(height).max(0) as f32 / 2.0);
        let margin = reach as f32;
        let caster = Caster {
            left: margin,
            top: margin,
            right: shadow_width as f32 - margin,
            bottom: shadow_height as f32 - margin,
            radius,
            sigma: self.look.sigma(),
        };
        let drop = self.look.drop() as f32;
        let cover = Cover {
            left: caster.left,
            top: caster.top - drop,
            right: caster.right,
            bottom: caster.bottom - drop,
            radius,
        };
        fill_profile(
            &mut self.columns,
            shadow_width,
            caster.left,
            caster.right,
            caster.sigma,
        );
        fill_profile(
            &mut self.rows,
            shadow_height,
            caster.top,
            caster.bottom,
            caster.sigma,
        );
        let tiled = shadow_width >= 2 * self.span && shadow_height >= 2 * self.span;

        for y in 0..shadow_height {
            let row = &mut pixels[y * stride..y * stride + shadow_width];
            let from_edge_y = y.min(shadow_height - 1 - y);
            for (x, pixel) in row.iter_mut().enumerate() {
                let from_edge_x = x.min(shadow_width - 1 - x);
                let (center_x, center_y) = (pixel_center(x), pixel_center(y));
                let covered = cover.coverage(center_x, center_y);
                let blur = if covered >= 1.0 {
                    0.0
                } else if from_edge_x >= self.span || from_edge_y >= self.span {
                    self.columns[x] * self.rows[y]
                } else if tiled {
                    self.corner[from_edge_y * self.span + from_edge_x]
                } else {
                    caster.blur_at(center_x, center_y)
                };
                let edge = from_edge_x == 0 || from_edge_y == 0;
                *pixel = if outline && edge {
                    OUTLINE
                } else {
                    alpha_pixel(blur * (1.0 - covered))
                };
            }
        }
    }
}

/// The middle of pixel `index`.
fn pixel_center(index: usize) -> f32 {
    index as f32 + 0.5
}

/// Black at `alpha`, 0 to 1, premultiplied.
fn alpha_pixel(alpha: f32) -> u32 {
    ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24
}

/// The blur across a box from `low` to `high`, at each of `count` pixels
/// along one axis.
fn fill_profile(profile: &mut Vec<f32>, count: usize, low: f32, high: f32, sigma: f32) {
    let scale = FRAC_1_SQRT_2 / sigma;
    profile.clear();
    profile.extend((0..count).map(|index| {
        let at = pixel_center(index);
        0.5 * (erf((at - low) * scale) - erf((at - high) * scale))
    }));
}

/// The rounded box that casts the shadow, in the shadow's pixels.
#[derive(Debug, Clone, Copy)]
struct Caster {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    sigma: f32,
}

impl Caster {
    /// The blurred box at a point, 0 to 1: Wallace's sampled form.
    fn blur_at(&self, x: f32, y: f32) -> f32 {
        let half = (
            (self.right - self.left) / 2.0,
            (self.bottom - self.top) / 2.0,
        );
        let x = x - (self.left + self.right) / 2.0;
        let y = y - (self.top + self.bottom) / 2.0;
        let reach = 3.0 * self.sigma;
        // Only the box's own rows add anything.
        let (low, high) = (y - half.1, y + half.1);
        let start = (-reach).clamp(low, high);
        let end = reach.clamp(low, high);
        let step = (end - start) / SAMPLES as f32;
        (0..SAMPLES)
            .map(|sample| {
                let offset = start + step * (sample as f32 + 0.5);
                self.blur_along_x(x, y - offset, half) * gaussian(offset, self.sigma) * step
            })
            .sum::<f32>()
            .clamp(0.0, 1.0)
    }

    /// The blur along x of the box's row at `y`, which its corners narrow;
    /// both relative to the box's center.
    fn blur_along_x(&self, x: f32, y: f32, half: (f32, f32)) -> f32 {
        let radius = self.radius;
        let inside_corner = (half.1 - radius - y.abs()).min(0.0);
        let row_half = half.0 - radius
            + (radius * radius - inside_corner * inside_corner)
                .max(0.0)
                .sqrt();
        let scale = FRAC_1_SQRT_2 / self.sigma;
        0.5 * (erf((x + row_half) * scale) - erf((x - row_half) * scale))
    }
}

/// The space itself over its shadow, in the shadow's pixels.
#[derive(Debug, Clone, Copy)]
struct Cover {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
}

impl Cover {
    /// How much of the pixel centered at a point the space covers, 0 to 1,
    /// from its distance to the rounded edge, so the cut is anti-aliased.
    fn coverage(&self, x: f32, y: f32) -> f32 {
        if x < self.left - 1.0
            || x > self.right + 1.0
            || y < self.top - 1.0
            || y > self.bottom + 1.0
        {
            return 0.0;
        }
        let half = (
            (self.right - self.left) / 2.0,
            (self.bottom - self.top) / 2.0,
        );
        let inner = (half.0 - self.radius, half.1 - self.radius);
        let qx = (x - (self.left + self.right) / 2.0).abs() - inner.0;
        let qy = (y - (self.top + self.bottom) / 2.0).abs() - inner.1;
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let distance = outside + qx.max(qy).min(0.0) - self.radius;
        (0.5 - distance).clamp(0.0, 1.0)
    }
}

/// The normal distribution's density at `x`.
fn gaussian(x: f32, sigma: f32) -> f32 {
    (-(x * x) / (2.0 * sigma * sigma)).exp() / ((2.0 * PI).sqrt() * sigma)
}

/// The error function, to about 5e-4 (Abramowitz and Stegun 7.1.27), as in
/// Wallace's shader.
fn erf(x: f32) -> f32 {
    let a = x.abs();
    let t = 1.0 + (0.278_393 + (0.230_389 + 0.078_108 * a * a) * a) * a;
    let t = t * t;
    x.signum() * (1.0 - 1.0 / (t * t))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: ShadowLook = ShadowLook {
        reach: 16,
        radius: 8.0,
    };

    /// The shadow of a `width` by `height` space as alphas, 0 to 255, row
    /// by row.
    fn painted(look: ShadowLook, width: i32, height: i32, outline: bool) -> (Vec<u8>, usize) {
        let rect = look.rect(Rect::new(0, 0, width, height));
        let stride = rect.width as usize + 3;
        let mut pixels = vec![0x1234_5678; stride * rect.height as usize];
        ShadowPieces::new(look).paint((width, height), &mut pixels, stride, outline);
        let alphas = pixels
            .chunks(stride)
            .flat_map(|row| {
                row[..rect.width as usize]
                    .iter()
                    .map(|pixel| (pixel >> 24) as u8)
            })
            .collect();
        (alphas, rect.width as usize)
    }

    fn at(alphas: &[u8], width: usize, x: usize, y: usize) -> u8 {
        alphas[y * width + x]
    }

    #[test]
    fn the_shadow_reaches_past_the_space_and_falls_a_little_below_it() {
        let space = Rect::new(100, 50, 200, 120);
        let rect = LOOK.rect(space);
        assert_eq!(rect, Rect::new(84, 38, 232, 152));
        assert_eq!(LOOK.drop(), 4);
    }

    #[test]
    fn it_fades_to_nothing_at_its_edge_and_is_cut_out_under_the_space() {
        let (alphas, width) = painted(LOOK, 200, 120, false);
        let height = alphas.len() / width;
        assert!(at(&alphas, width, 0, height / 2) <= 2, "the outer edge");
        assert!(at(&alphas, width, width / 2, 0) <= 2);
        let middle = at(&alphas, width, width / 2, height / 2);
        assert_eq!(middle, 0, "nothing under the space");
        // Just past the space's left edge, about half the blur.
        let beside = at(&alphas, width, 15, height / 2);
        assert!((100..=160).contains(&beside), "{beside}");
    }

    #[test]
    fn it_is_darker_below_the_space_than_above() {
        let (alphas, width) = painted(LOOK, 200, 120, false);
        // The space's top and bottom edges, in the shadow's pixels.
        let top = (LOOK.reach - LOOK.drop()) as usize;
        let bottom = top + 120;
        let column = width / 2;
        let above = at(&alphas, width, column, top - 3);
        let below = at(&alphas, width, column, bottom + 2);
        assert!(below > above, "{below} > {above}");
    }

    #[test]
    fn it_is_the_same_on_the_left_and_the_right() {
        let (alphas, width) = painted(LOOK, 200, 120, false);
        for (index, row) in alphas.chunks(width).enumerate() {
            let mirrored: Vec<u8> = row.iter().rev().copied().collect();
            assert_eq!(row, mirrored.as_slice(), "row {index}");
        }
    }

    #[test]
    fn the_corner_tile_draws_what_the_sampled_form_does() {
        let (width, height) = (200, 120);
        let (alphas, shadow_width) = painted(LOOK, width, height, false);
        // The sampled form, pixel by pixel, on the same space.
        let pieces = ShadowPieces::new(LOOK);
        let margin = LOOK.reach as f32;
        let caster = Caster {
            left: margin,
            top: margin,
            right: shadow_width as f32 - margin,
            bottom: (alphas.len() / shadow_width) as f32 - margin,
            radius: LOOK.radius,
            sigma: LOOK.sigma(),
        };
        for y in 0..pieces.span {
            for x in 0..pieces.span {
                let expected = alpha_pixel(caster.blur_at(pixel_center(x), pixel_center(y))) >> 24;
                let cover = Cover {
                    left: caster.left,
                    top: caster.top - LOOK.drop() as f32,
                    right: caster.right,
                    bottom: caster.bottom - LOOK.drop() as f32,
                    radius: LOOK.radius,
                };
                let covered = cover.coverage(pixel_center(x), pixel_center(y));
                let expected = (expected as f32 * (1.0 - covered)).round() as i32;
                let got = i32::from(at(&alphas, shadow_width, x, y));
                assert!(
                    (got - expected).abs() <= 2,
                    "({x}, {y}): {got} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn a_space_rolled_to_a_line_still_casts_a_whole_shadow() {
        for height in [1, 3] {
            let (alphas, width) = painted(LOOK, 300, height, false);
            assert_eq!(alphas.len(), width * (height + 32) as usize);
            assert!(alphas.iter().any(|alpha| *alpha > 0), "height {height}");
        }
    }

    #[test]
    fn an_outline_marks_the_edge_and_leaves_the_rest() {
        let (outlined, width) = painted(LOOK, 100, 60, true);
        let (plain, _) = painted(LOOK, 100, 60, false);
        let height = outlined.len() / width;
        assert_eq!(at(&outlined, width, 0, 10), 255);
        assert_eq!(at(&outlined, width, width - 1, height - 1), 255);
        assert_eq!(
            at(&outlined, width, 20, 20),
            at(&plain, width, 20, 20),
            "inside the edge, the shadow as it is"
        );
    }

    #[test]
    fn drawing_writes_only_its_own_pixels() {
        let rect = LOOK.rect(Rect::new(0, 0, 50, 40));
        let stride = rect.width as usize + 5;
        let mut pixels = vec![7; stride * rect.height as usize];
        ShadowPieces::new(LOOK).paint((50, 40), &mut pixels, stride, false);
        for row in pixels.chunks(stride) {
            assert!(row[rect.width as usize..].iter().all(|pixel| *pixel == 7));
        }
    }
}
