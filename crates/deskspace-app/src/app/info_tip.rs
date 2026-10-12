//! `App` info tips: the delayed item tooltip (R1).

use deskspace_core::geometry::Point;
use deskspace_platform::Ui;

use super::{App, INFO_TIP_TIMER};

impl App {
    /// Shows the info tip for the hovered item, once the pointer has rested on
    /// it, as Explorer does.
    pub(super) fn show_info_tip(&mut self, ui: &mut Ui) {
        ui.clear_timer(INFO_TIP_TIMER);
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
        ui.clear_timer(INFO_TIP_TIMER);
    }
}
