//! The title icon as a button while a space's title is renamed (ROADMAP D16,
//! D26): a press on it opens the icon picker and leaves the rename open
//! behind it.

use deskspace_core::SpaceId;
use deskspace_core::geometry::Point;
use deskspace_platform::{MouseButton, SpaceEvent, Ui};

use super::rename::EditTarget;
use super::{App, Surface};

impl App {
    /// Whether `event` is a left press on the title icon button of `surface`.
    /// When it is, opens the icon picker and returns true, so the caller does
    /// not commit the rename.
    pub(super) fn title_icon_pressed(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        event: &SpaceEvent,
    ) -> bool {
        let SpaceEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        else {
            return false;
        };
        let Surface::Space(id) = surface else {
            return false;
        };
        if !self.title_icon_button_at(id, Point::new(*x, *y)) {
            return false;
        }
        self.pick_space_icon(ui, id);
        true
    }

    /// Whether `point` is on the title icon button of `id`, which exists only
    /// while that space's title is being renamed.
    pub(super) fn title_icon_button_at(&self, id: SpaceId, point: Point) -> bool {
        let renaming = self.edit.session().is_some_and(|session| {
            session.surface == Surface::Space(id) && session.target == EditTarget::Title
        });
        renaming
            && self
                .config
                .desktop
                .space(id)
                .is_some_and(|space| crate::scene::title_icon_at(space, self.is_browsed(id), point))
    }
}
