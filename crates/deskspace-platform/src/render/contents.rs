//! A space's text and icons as separate sprites for Smoke (ROADMAP D12):
//! they ride the same current as the smoke, then land in their own places.
//! Pure: the platform renders the sprites once (`d2d::contents`).

use deskspace_core::geometry::Rect;

use super::smoke::{Canvas, SmokeField};
use super::snapshot::Snapshot;

/// One sprite: the part of the contents it shows and where it belongs, in
/// space pixels. The title bar's text is the `title` sprite; every item is
/// its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    pub target: Rect,
    pub title: bool,
}

/// The space's text and icons, transparent everywhere else, at its pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contents {
    pixels: Snapshot,
    sprites: Vec<Sprite>,
}

impl Contents {
    pub fn new(pixels: Snapshot, sprites: Vec<Sprite>) -> Self {
        Self { pixels, sprites }
    }

    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }
}

/// Draws `contents` over `canvas` at `shown` (0 gone, 1 there), each sprite
/// riding `field`'s current and settling into its target. `canvas` already
/// holds the smoke.
pub fn paint(canvas: Canvas<'_>, contents: &Contents, field: &SmokeField, shown: f32) {
    let Canvas {
        pixels,
        width,
        height,
        origin,
    } = canvas;
    let shown = shown.clamp(0.0, 1.0);
    let alpha = smoothstep(0.25, 0.9, shown);
    if alpha <= 0.0 {
        return;
    }
    let gone = 1.0 - shown;
    for sprite in contents.sprites() {
        let target = sprite.target;
        if target.width <= 0 || target.height <= 0 {
            continue;
        }
        let center_x = target.x as f32 + target.width as f32 / 2.0;
        let center_y = target.y as f32 + target.height as f32 / 2.0;
        let (sway, lift) = field.flow(center_x, center_y, gone);
        let dest_x = (center_x + origin.0 as f32 + sway * gone).clamp(0.0, width as f32);
        // The title starts high enough to leave the overlay; keep it at the
        // top edge until the current brings it in.
        let dest_y = (center_y + origin.1 as f32 - lift * gone).max(0.0);
        let scale = if sprite.title {
            1.0
        } else {
            0.94 + 0.06 * shown
        };
        for y in 0..target.height {
            for x in 0..target.width {
                let source = contents.pixels.pixel(target.x + x, target.y + y);
                if source >> 24 == 0 {
                    continue;
                }
                let at_x = dest_x + (x as f32 + 0.5 - target.width as f32 / 2.0) * scale;
                let at_y = dest_y + (y as f32 + 0.5 - target.height as f32 / 2.0) * scale;
                let (at_x, at_y) = (at_x.round() as i32, at_y.round() as i32);
                if at_x < 0 || at_y < 0 || at_x >= width as i32 || at_y >= height as i32 {
                    continue;
                }
                blend(
                    &mut pixels[(at_y as u32 * width + at_x as u32) as usize],
                    source,
                    alpha,
                );
            }
        }
    }
}

/// A sprite pixel over the smoke: both premultiplied, `alpha` scales the
/// sprite.
fn blend(destination: &mut u32, source: u32, alpha: f32) {
    let source_alpha = f32::from((source >> 24) as u8) / 255.0 * alpha;
    let destination_alpha = (f32::from((*destination >> 24) as u8) / 255.0) * (1.0 - source_alpha);
    let channel = |shift: u32| {
        let source = f32::from(((source >> shift) & 0xFF) as u8) * alpha;
        let destination = f32::from(((*destination >> shift) & 0xFF) as u8) * (1.0 - source_alpha);
        (source + destination).round().clamp(0.0, 255.0) as u32
    };
    let alpha = ((source_alpha + destination_alpha) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u32;
    *destination = alpha << 24 | channel(16) << 16 | channel(8) << 8 | channel(0);
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::smoke;
    use deskspace_core::theme::Color;

    fn contents() -> Contents {
        let (width, height) = (80, 60);
        let mut pixels = vec![0u32; (width * height) as usize];
        for y in 10..14 {
            for x in 10..14 {
                pixels[(y * width + x) as usize] = 0xFFFF_0000;
            }
        }
        Contents::new(
            Snapshot::new(width, height, pixels.into_boxed_slice()),
            vec![Sprite {
                target: Rect::new(10, 10, 4, 4),
                title: false,
            }],
        )
    }

    fn painted(contents: &Contents, shown: f32) -> (Vec<u32>, u32) {
        let field = SmokeField::new(80, 60, 5, 7);
        let (width, height) = (160, 160);
        let mut pixels = vec![0u32; (width * height) as usize];
        smoke::paint(
            Canvas {
                pixels: &mut pixels,
                width,
                height,
                origin: (40, 60),
            },
            smoke::Plume {
                field: &field,
                tint: Color::rgba(20, 60, 200, 200),
                gone: 1.0 - shown,
            },
            &mut smoke::Scratch::default(),
        );
        paint(
            Canvas {
                pixels: &mut pixels,
                width,
                height,
                origin: (40, 60),
            },
            contents,
            &field,
            shown,
        );
        (pixels, width)
    }

    #[test]
    fn a_sprite_lands_on_its_target() {
        let contents = contents();
        let (pixels, width) = painted(&contents, 1.0);
        // The red square is at (10, 10) in the space, which starts at (40, 60).
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        assert_eq!((at(52, 72) >> 16) & 0xFF, 0xFF, "red lands in place");
        // The title sprite (none here) never draws.
        assert_eq!(contents.sprites().len(), 1);
    }

    #[test]
    fn a_sprite_is_elsewhere_and_fainter_before_it_lands() {
        let contents = contents();
        let (early, width) = painted(&contents, 0.5);
        let (landed, _) = painted(&contents, 1.0);
        let at = |pixels: &[u32], x: u32, y: u32| pixels[(y * width + x) as usize];
        assert!(at(&early, 52, 72) >> 24 < at(&landed, 52, 72) >> 24);
        let moved = (0..160).any(|y| {
            (0..160).any(|x| {
                let pixel = at(&early, x, y);
                pixel >> 24 > 0 && (x, y) != (52, 72)
            })
        });
        assert!(moved, "the sprite rides the current");
    }

    #[test]
    fn the_flight_is_stable_and_leaves_premultiplied_pixels() {
        let contents = contents();
        let (first, _) = painted(&contents, 0.6);
        let (again, _) = painted(&contents, 0.6);
        assert_eq!(first, again);
        for pixel in first {
            let alpha = pixel >> 24;
            for shift in [0, 8, 16] {
                assert!((pixel >> shift) & 0xFF <= alpha, "{pixel:#010X}");
            }
        }
    }

    #[test]
    fn the_flight_ends_exactly_on_the_target() {
        let (pixels, width) = painted(&contents(), 1.0);
        let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
        // The item's center lands on its exact spot, with no offset left.
        assert_eq!((at(52, 72) >> 16) & 0xFF, 0xFF);
    }
}
