//! `App` info tips: the delayed item tooltip (R1).

use deskspace_core::geometry::Point;
use deskspace_platform::Ui;

use super::{App, Surface, Timer};

impl App {
    /// Shows the info tip for the hovered item, once the pointer has rested on
    /// it, as Explorer does. A status-bar button shows its own tip instead
    /// (W19), and a lens's title bar the folder it displays (W27).
    pub(super) fn show_info_tip(&mut self, ui: &mut Ui) {
        Self::clear_timer(ui, Timer::InfoTip);
        if let Some((Surface::Space(id), index)) = self.pointer.status_hover
            && let Some(space) = self.config.desktop.space(id)
            && let (Some(text), Some(rect)) = (
                crate::scene::status_tip(index),
                crate::scene::status_button_rect(space, index),
            )
        {
            let origin = space.rect().origin();
            ui.show_text_tip(&text, Point::new(origin.x + rect.x, origin.y + rect.y));
            return;
        }
        // The tip was armed before the rename began, so it is held back again
        // here: it would cover the title being edited.
        if let Some(id) = self.pointer.title_hover
            && self.edit.surface() != Some(Surface::Space(id))
            && let Some(space) = self.config.desktop.space(id)
            && let Some(shown) = self.lens_folder(id)
            && let Some(text) =
                crate::scene::title_tip(space, &shown, self.unavailable_lenses.contains(&id))
        {
            ui.show_text_tip(&text, space.rect().origin());
            return;
        }
        let Some((surface, item)) = self.pointer.hovered() else {
            return;
        };
        let Some(entry) = self.config.desktop.item(item) else {
            return;
        };
        let path = entry.path.clone();
        let cell = self.surface_cell(surface, entry);
        ui.show_info_tip(&path, Point::new(cell.x, cell.y));
    }

    /// Hides any info tip and cancels its delay, for example when a click or
    /// drag interrupts the hover.
    pub(super) fn dismiss_info_tip(&mut self, ui: &mut Ui) {
        ui.hide_info_tip();
        Self::clear_timer(ui, Timer::InfoTip);
    }
}
