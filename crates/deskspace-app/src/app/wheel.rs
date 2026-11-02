//! The mouse wheel: Ctrl zooms, Shift or the tilt wheel scrolls a Details
//! header sideways, and every other notch scrolls the view's own axis (D24).

use deskspace_core::geometry::Point;
use deskspace_core::{SpaceId, ViewMode};

use deskspace_platform::Ui;

use super::{App, Surface, WHEEL_DELTA};

/// What one wheel notch does on a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelAction {
    Zoom,
    /// Scroll a Details header's columns sideways.
    Sideways,
    Vertical,
    Ignore,
}

/// One wheel event: where the pointer is, its notch, and the keys and axis it
/// came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WheelInput {
    pub(super) point: Point,
    pub(super) delta: i32,
    pub(super) ctrl: bool,
    pub(super) shift: bool,
    pub(super) tilt: bool,
}

/// The wheel's action, from plain data so a test can decide it without a `Ui`.
/// Ctrl always zooms; Shift, or a tilt wheel, reaches Details' second axis only
/// there. The desktop layer and a tilt outside Details are ignored.
fn wheel_action(mode: Option<ViewMode>, ctrl: bool, shift: bool, tilt: bool) -> WheelAction {
    if ctrl {
        return WheelAction::Zoom;
    }
    match mode {
        None => WheelAction::Ignore,
        Some(ViewMode::Details) if shift || tilt => WheelAction::Sideways,
        Some(_) if tilt => WheelAction::Ignore,
        Some(_) => WheelAction::Vertical,
    }
}

impl App {
    pub(super) fn on_wheel(&mut self, ui: &mut Ui, surface: Surface, input: WheelInput) {
        let mode = match surface {
            Surface::Space(id) => self.config.desktop.space(id).map(|space| space.view().mode),
            Surface::Desktop => None,
        };
        match wheel_action(mode, input.ctrl, input.shift, input.tilt) {
            WheelAction::Zoom => self.adjust_icon_size(ui, surface, input.delta),
            WheelAction::Sideways => {
                if let Surface::Space(id) = surface {
                    // A tilt's positive delta is to the right, and
                    // `scroll_by_x` subtracts its delta like the vertical
                    // wheel, so turn it around.
                    let delta = if input.tilt {
                        -input.delta
                    } else {
                        input.delta
                    };
                    self.scroll_by_x(ui, id, input.point, delta);
                }
            }
            WheelAction::Vertical => {
                if let Surface::Space(id) = surface {
                    self.scroll_by(ui, id, input.point, input.delta);
                }
            }
            WheelAction::Ignore => {}
        }
    }

    pub(super) fn scroll_by(&mut self, ui: &mut Ui, id: SpaceId, point: Point, delta: i32) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let Some(layout) = self.view_layout(id) else {
            return;
        };
        let max = layout.max_scroll(space.content_rect().size());
        let current = self.scroll_of(id);
        let next = (current - delta * layout.scroll_step() / WHEEL_DELTA).clamp(0, max);
        if next == current {
            return;
        }
        self.view_cache.set_scroll(id, next);
        // Items moved under a still cursor, so the hovered one changed too.
        let surface = Surface::Space(id);
        self.pointer.hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        self.refresh_space(ui, id);
    }

    /// Scrolls space `id`'s Details columns sideways by one notch of `delta`.
    pub(super) fn scroll_by_x(&mut self, ui: &mut Ui, id: SpaceId, point: Point, delta: i32) {
        let Some(layout) = self.view_layout(id) else {
            return;
        };
        let max = self.details_scroll_limit(id);
        if max == 0 {
            return;
        }
        let current = self.view_cache.scroll_x_of(id).clamp(0, max);
        let next = (current - delta * layout.scroll_step() / WHEEL_DELTA).clamp(0, max);
        if next == current {
            return;
        }
        self.view_cache.set_scroll_x(id, next);
        // Items moved under a still cursor, so the hovered one changed too.
        let surface = Surface::Space(id);
        self.pointer.hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        self.refresh_space(ui, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_zooms_scrolls_sideways_or_scrolls_the_view() {
        let details = Some(ViewMode::Details);
        assert_eq!(wheel_action(details, true, false, false), WheelAction::Zoom);
        assert_eq!(
            wheel_action(details, false, true, false),
            WheelAction::Sideways
        );
        assert_eq!(
            wheel_action(details, false, false, true),
            WheelAction::Sideways,
            "the tilt wheel scrolls Details sideways"
        );
        assert_eq!(
            wheel_action(details, false, false, false),
            WheelAction::Vertical
        );
        assert_eq!(
            wheel_action(Some(ViewMode::List), false, true, false),
            WheelAction::Vertical,
            "only Details has a second axis"
        );
        assert_eq!(
            wheel_action(Some(ViewMode::List), false, false, true),
            WheelAction::Ignore,
            "a tilt outside Details has no axis"
        );
        assert_eq!(wheel_action(None, false, false, false), WheelAction::Ignore);
        assert_eq!(wheel_action(None, true, false, false), WheelAction::Zoom);
    }
}
