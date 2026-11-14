//! Burn's flames on screen (ROADMAP D12, amended 2026-10-18). They rise past
//! the space's frame, and the space's own window carries the blur over its
//! whole rect, so they are drawn on a layered overlay of their own
//! (`LayeredOverlay`), living as long as one Burn plays (`space_animation`).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::HWND;

use crate::render::burn::{BurnField, FlameBase};
use crate::render::flames::{self, MARGIN_SIDE, MARGIN_TOP, Rows, SPACING, Tongue};
use crate::win32::layered_overlay::LayeredOverlay;

pub(crate) struct FlameOverlay {
    overlay: LayeredOverlay,
    /// The same field the painter burns the space with, so the flames stand
    /// on its edge.
    field: BurnField,
    bases: Vec<FlameBase>,
    tongues: Vec<Tongue>,
    /// The rows of the pixel buffer the last frame drew, which the next one clears.
    drawn: Rows,
    space_height: f32,
}

impl FlameOverlay {
    /// The flames for `space`'s window `owner` at `rect` (screen
    /// coordinates), burning with `detail`. `None` for a desktop-hosted
    /// space, a child window with nothing to float above, or when Windows
    /// refuses the window or its bitmap.
    pub(crate) fn create(owner: HWND, rect: Rect, detail: u8) -> Option<Self> {
        let overlay = LayeredOverlay::create(owner, overlay_rect(rect))?;
        Some(Self {
            overlay,
            field: BurnField::for_size((rect.width as f32, rect.height as f32), detail),
            bases: Vec::new(),
            tongues: Vec::new(),
            drawn: Rows::ALL,
            space_height: rect.height as f32,
        })
    }

    /// Follows the space to `rect`, where its window now ends up while it
    /// burns; the flames move at their next draw. Returns false for a rect
    /// of another size, which needs new flames on a field of its own.
    pub(crate) fn follow(&mut self, rect: Rect) -> bool {
        self.overlay.follow(overlay_rect(rect))
    }

    /// Draws the flames for a burn at `front`, `time` seconds into any clock.
    pub(crate) fn draw(&mut self, front: f32, time: f32) {
        self.field.flame_bases(front, SPACING, &mut self.bases);
        flames::tongues(&self.bases, self.space_height, time, &mut self.tongues);
        let (width, height) = self.overlay.pixel_size();
        flames::paint(
            self.overlay.pixels(),
            width,
            height,
            &self.tongues,
            time,
            &mut self.drawn,
        );
        self.overlay.present();
    }
}

/// Where the overlay goes for a space at `rect` (screen coordinates): with
/// room beside and above the space for the flames to rise into.
fn overlay_rect(rect: Rect) -> Rect {
    Rect::new(
        rect.x - MARGIN_SIDE as i32,
        rect.y - MARGIN_TOP as i32,
        rect.width + 2 * MARGIN_SIDE as i32,
        rect.height + MARGIN_TOP as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_leaves_room_beside_and_above_its_space() {
        let rect = Rect::new(300, 200, 240, 160);
        let expanded = overlay_rect(rect);
        assert_eq!((expanded.x, expanded.y), (300 - 24, 200 - 72));
        assert_eq!((expanded.width, expanded.height), (240 + 48, 160 + 72));
        let moved = overlay_rect(rect.offset(50, -20));
        assert_eq!((moved.x, moved.y), (expanded.x + 50, expanded.y - 20));
        assert_eq!(
            (moved.width, moved.height),
            (expanded.width, expanded.height),
            "a moved space keeps its overlay's size"
        );
        let narrowed = overlay_rect(Rect::new(300, 200, 100, 160));
        assert_ne!(
            (narrowed.width, narrowed.height),
            (expanded.width, expanded.height)
        );
    }
}
