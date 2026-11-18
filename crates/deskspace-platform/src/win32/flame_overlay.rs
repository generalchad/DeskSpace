//! Burn's flames on screen (ROADMAP D12, amended 2026-10-18). They rise past
//! the space's frame, and the space's own window carries the blur over its
//! whole rect, so they are drawn on a layered overlay of their own
//! (`LayeredOverlay`), living as long as one Burn plays (`space_animation`).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::HWND;

use crate::render::burn::BurnField;
use crate::render::flames::{self, Rows, SPACING, Tongue};
use crate::win32::layered_overlay::LayeredOverlay;

pub(crate) struct FlameOverlay {
    overlay: LayeredOverlay,
    /// The same field the painter burns the space with, so the flames stand
    /// on its edge.
    field: BurnField,
    bases: Vec<crate::render::burn::FlameBase>,
    tongues: Vec<Tongue>,
    /// The rows of the pixel buffer the last frame drew, which the next one clears.
    drawn: Rows,
    space_height: f32,
    /// The display the space is on: its cells, margins, and flames scale
    /// with it.
    dpi: u32,
    seed: u32,
}

impl FlameOverlay {
    /// The flames for `space`'s window `owner` at `rect` (screen
    /// coordinates) on a display of `dpi`, burning with `detail` and `seed`.
    /// `None` for a desktop-hosted space, a child window with nothing to
    /// float above, or when Windows refuses the window or its bitmap.
    pub(crate) fn create(owner: HWND, rect: Rect, dpi: u32, detail: u8, seed: u32) -> Option<Self> {
        let overlay = LayeredOverlay::create(owner, overlay_rect(rect, dpi))?;
        Some(Self {
            overlay,
            field: BurnField::for_space((rect.width as f32, rect.height as f32), detail, seed, dpi),
            bases: Vec::new(),
            tongues: Vec::new(),
            drawn: Rows::ALL,
            space_height: rect.height as f32,
            dpi,
            seed,
        })
    }

    /// Follows the space to `rect`, where its window now ends up while it
    /// burns; the flames move at their next draw. Returns false for a rect
    /// of another size, which needs new flames on a field of its own.
    pub(crate) fn follow(&mut self, rect: Rect) -> bool {
        self.overlay.follow(overlay_rect(rect, self.dpi))
    }

    /// Draws the flames for a burn `gone` of the way through, `time` seconds
    /// into any clock.
    pub(crate) fn draw(&mut self, gone: f32, time: f32) {
        let front = self.field.front(gone);
        self.field.flame_bases(front, SPACING, &mut self.bases);
        flames::tongues(
            &self.bases,
            self.space_height,
            self.dpi,
            time,
            self.seed,
            &mut self.tongues,
        );
        let (width, height) = self.overlay.pixel_size();
        let previous = self.drawn;
        flames::paint(
            self.overlay.pixels(),
            width,
            height,
            &self.tongues,
            time,
            &mut self.drawn,
        );
        self.overlay.present(previous.union(self.drawn));
    }
}

/// Where the overlay goes for a space at `rect` (screen coordinates) on a
/// display of `dpi`: with room beside and above the space for the flames to
/// rise into.
fn overlay_rect(rect: Rect, dpi: u32) -> Rect {
    let (side, top) = flames::margins(dpi);
    let (width, height) = flames::overlay_size(rect.width as u32, rect.height as u32, dpi);
    Rect::new(
        rect.x - side as i32,
        rect.y - top as i32,
        width as i32,
        height as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_leaves_room_beside_and_above_its_space() {
        let rect = Rect::new(300, 200, 240, 160);
        let (side, top) = flames::margins(96);
        let expanded = overlay_rect(rect, 96);
        assert_eq!(
            (expanded.x, expanded.y),
            (300 - side as i32, 200 - top as i32)
        );
        assert_eq!(
            (expanded.width, expanded.height),
            (240 + 2 * side as i32, 160 + top as i32)
        );
        let moved = overlay_rect(rect.offset(50, -20), 96);
        assert_eq!((moved.x, moved.y), (expanded.x + 50, expanded.y - 20));
        assert_eq!(
            (moved.width, moved.height),
            (expanded.width, expanded.height),
            "a moved space keeps its overlay's size"
        );
        let narrowed = overlay_rect(Rect::new(300, 200, 100, 160), 96);
        assert_ne!(
            (narrowed.width, narrowed.height),
            (expanded.width, expanded.height)
        );
        let scaled = overlay_rect(rect, 192);
        assert_eq!(
            (scaled.width, scaled.height),
            (
                240 + 2 * flames::margins(192).0 as i32,
                160 + flames::margins(192).1 as i32
            ),
            "a 200 % display leaves twice the room"
        );
    }
}
