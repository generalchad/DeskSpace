//! `App` persistence: command dispatch and the debounced configuration write
//! (R1).

use deskspace_core::geometry::Point;
use deskspace_core::{Command, SpaceId};
use deskspace_platform::{Ui, work_area_at};

use super::{AUTOSAVE_DELAY_MS, AUTOSAVE_TIMER, App};

impl App {
    pub(super) fn dispatch(&mut self, command: Command, near: Point) -> bool {
        let bounds = work_area_at(near);
        // Use the grid of the space under the action so a command that moves or
        // arranges items lands on the same monitor's scale.
        let layout = self
            .config
            .desktop
            .space_at(near)
            .map_or_else(|| self.effective_layout(), |id| self.layout_for(id));
        self.config
            .desktop
            .apply_with_layout(command, layout, bounds)
            .is_applied()
    }

    pub(super) fn save(&self) {
        if let Err(error) = self.store.save(&self.config) {
            tracing::error!(%error, "failed to persist configuration");
        } else {
            tracing::debug!(path = %self.store.path().display(), "configuration saved");
        }
    }

    /// Defers a configuration write until the user pauses, restarting the
    /// timer on every change so a burst of them writes once.
    pub(super) fn schedule_save(&mut self, ui: &mut Ui) {
        self.autosave.mark();
        ui.set_timer(AUTOSAVE_TIMER, AUTOSAVE_DELAY_MS);
    }

    /// Writes any deferred change now and stops the autosave timer. Used when
    /// the timer fires and when the app closes, so nothing is lost.
    pub(super) fn flush_save(&mut self, ui: &mut Ui) {
        if self.autosave.take() {
            ui.clear_timer(AUTOSAVE_TIMER);
        }
        self.save();
    }

    /// Applies `command`, then repaints the space and schedules a save when the
    /// domain changed. Returns whether the command applied.
    pub(super) fn apply_to_space(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        command: Command,
        at: Point,
    ) -> bool {
        if self.dispatch(command, at) {
            self.refresh_space(ui, id);
            self.schedule_save(ui);
            true
        } else {
            false
        }
    }
}
