//! `App` persistence: command dispatch and the debounced configuration write
//! (R1).

use deskspace_core::geometry::Point;
use deskspace_core::{Command, CommandOutcome};
use deskspace_platform::{Ui, work_area_at};

use super::{App, Timer};

/// How long after the last change the configuration is written.
const AUTOSAVE_DELAY_MS: u32 = 700;

impl App {
    pub(super) fn dispatch(&mut self, command: Command, near: Point) -> CommandOutcome {
        let bounds = work_area_at(near);
        // Use the grid of the space under the action so a command that moves or
        // arranges items lands on the same monitor's scale.
        let layout = self
            .config
            .desktop
            .space_at(near)
            .map_or_else(|| self.effective_layout(), |id| self.layout_for(id));
        let outcome = self
            .config
            .desktop
            .apply_with_layout(command, layout, bounds);
        if !outcome.is_applied() {
            tracing::debug!(?outcome, "command not applied");
        }
        outcome
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
        Self::set_timer(ui, Timer::Autosave, AUTOSAVE_DELAY_MS);
    }

    /// Writes any deferred change now and stops the autosave timer. Used when
    /// the timer fires and when the app closes, so nothing is lost.
    pub(super) fn flush_save(&mut self, ui: &mut Ui) {
        if self.autosave.take() {
            Self::clear_timer(ui, Timer::Autosave);
        }
        self.save();
    }

    /// Applies `command`, then, when it applied, repaints what it changed
    /// (`Desktop::touched_by`) and schedules a save. The one path for a change
    /// whose repaint and save need nothing special.
    pub(super) fn commit(&mut self, ui: &mut Ui, command: Command, at: Point) -> CommandOutcome {
        let touched = self.config.desktop.touched_by(&command);
        let outcome = self.dispatch(command, at);
        if !outcome.is_applied() {
            return outcome;
        }
        // A deleted space is gone; `refresh_space` skips it.
        for id in touched.spaces {
            self.refresh_space(ui, id);
        }
        if touched.layer {
            self.refresh_desktop_layer(ui);
        }
        self.schedule_save(ui);
        outcome
    }
}
