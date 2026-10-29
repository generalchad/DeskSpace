//! The windows an effect draws on beside the space's own (ROADMAP D12,
//! amended 2026-10-19): what an effect draws past the space's frame, where
//! the space's own window would put blur behind it. Burn's flames are one;
//! T21's lamp will be another. `kind_for` says which effect draws on
//! which, with a match that holds every effect to an answer.

use deskspace_core::animation::Effect;
use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::HWND;

use crate::render::{Mask, Play, TransitionFrame};
use crate::win32::flame_overlay::FlameOverlay;

/// Which overlay an effect draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayKind {
    /// Burn's flames, rising past the space's frame.
    Flames,
}

/// The overlay `effect` draws on, if any. A new effect is a compile error
/// here until it says.
fn kind_for(effect: Effect) -> Option<OverlayKind> {
    match effect {
        Effect::Burn => Some(OverlayKind::Flames),
        Effect::None
        | Effect::Roll
        | Effect::Fade
        | Effect::Pixelate
        | Effect::Gradient
        | Effect::Dissolve
        | Effect::Fly
        | Effect::Crt => None,
    }
}

/// An overlay window, made when its effect starts and dropped, window and
/// all, when it ends.
pub(crate) enum Overlay {
    Flames(FlameOverlay),
}

impl Overlay {
    /// The overlay `play`'s effect draws on, for the space window `owner` at
    /// `rect` (screen coordinates). `None` for an effect with none, or when
    /// Windows refuses the window.
    pub(crate) fn for_play(owner: HWND, rect: Rect, play: &Play) -> Option<Self> {
        match kind_for(play.effect)? {
            OverlayKind::Flames => FlameOverlay::create(owner, rect, play.detail).map(Self::Flames),
        }
    }

    /// Draws `frame`, `time` seconds into any clock.
    pub(crate) fn draw(&mut self, frame: &TransitionFrame, time: f32) {
        match self {
            Self::Flames(flames) => {
                if let Mask::Burn { front, .. } = frame.mask {
                    flames.draw(front, time);
                }
            }
        }
    }

    /// Follows the space to `rect`, where its window now ends up. Returns
    /// false when the overlay must be made again for it (another size).
    pub(crate) fn follow(&mut self, rect: Rect) -> bool {
        match self {
            Self::Flames(flames) => flames.follow(rect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::EFFECTS;

    #[test]
    fn only_burn_draws_on_an_overlay() {
        for spec in EFFECTS {
            let expected = (spec.effect == Effect::Burn).then_some(OverlayKind::Flames);
            assert_eq!(kind_for(spec.effect), expected, "{:?}", spec.effect);
        }
    }
}
