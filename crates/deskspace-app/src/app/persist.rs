//! `App` persistence: command dispatch and the debounced configuration write
//! (R1).

use std::path::Path;

use deskspace_core::geometry::Point;
use deskspace_core::{Command, CommandOutcome};
use deskspace_platform::{Ui, work_area_at};

use super::{App, Timer};

/// How long after the last change the configuration is written.
const AUTOSAVE_DELAY_MS: u32 = 700;

impl App {
    pub(super) fn dispatch(&mut self, command: Command, near: Point) -> CommandOutcome {
        let bounds = work_area_at(near);
        // Lay the command out on the grid of the space it changes (for a move,
        // the destination), so the result matches that space's monitor scale
        // and icon size even when another space overlaps it at `near`. A
        // command that names no existing space falls back to the space under
        // the action.
        let target = self
            .config
            .desktop
            .touched_by(&command)
            .spaces
            .first()
            .copied()
            .filter(|id| self.config.desktop.space(*id).is_some())
            .or_else(|| self.config.desktop.space_at(near));
        let layout = target.map_or_else(|| self.effective_layout(), |id| self.layout_for(id));
        let outcome = self
            .config
            .desktop
            .apply_with_layout(command, layout, bounds);
        if !outcome.is_applied() {
            tracing::debug!(?outcome, "command not applied");
        }
        outcome
    }

    /// Writes the configuration now. The first failure since the last
    /// success is shown to the user: release builds have no console, so a
    /// log line alone would go unseen until changes were lost.
    pub(super) fn save(&mut self, ui: &mut Ui) {
        match self.write_config() {
            Ok(()) => self.autosave.reported_failure = false,
            Err(error) => {
                if !std::mem::replace(&mut self.autosave.reported_failure, true) {
                    let text = save_failure_text(self.store.path(), &error);
                    ui.alert("DeskSpace could not save", &text);
                }
            }
        }
    }

    fn write_config(&self) -> Result<(), String> {
        match self.store.save(&self.config) {
            Ok(()) => {
                tracing::debug!(path = %self.store.path().display(), "configuration saved");
                Ok(())
            }
            Err(error) => {
                tracing::error!(%error, "failed to persist configuration");
                Err(error.to_string())
            }
        }
    }

    /// Defers a configuration write until the user pauses, restarting the
    /// timer on every change so a burst of them writes once.
    pub(super) fn schedule_save(&mut self, ui: &mut Ui) {
        self.autosave.mark();
        Self::set_timer(ui, Timer::Autosave, AUTOSAVE_DELAY_MS);
    }

    /// Writes any deferred change now and stops the autosave timer. Used when
    /// the timer fires, so nothing is lost.
    pub(super) fn flush_save(&mut self, ui: &mut Ui) {
        if self.autosave.take() {
            Self::clear_timer(ui, Timer::Autosave);
        }
        self.save(ui);
    }

    /// `flush_save` for a shutdown (closing, or quitting to install an
    /// update): a failure is only logged, because a modal box must not block
    /// the exit or an unattended install.
    pub(super) fn flush_save_on_close(&mut self, ui: &mut Ui) {
        if self.autosave.take() {
            Self::clear_timer(ui, Timer::Autosave);
        }
        let _ = self.write_config();
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

/// What the user is told when the configuration cannot be written.
fn save_failure_text(path: &Path, error: &str) -> String {
    format!(
        "DeskSpace could not save its settings and spaces to\n{}\n\n{error}\n\n\
         Changes made since the last successful save may be lost when \
         DeskSpace closes.",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_failure_text_names_the_path_and_the_error() {
        let text = save_failure_text(Path::new("C:/cfg/deskspace.toml"), "Access is denied.");
        assert!(text.contains("C:/cfg/deskspace.toml"));
        assert!(text.contains("Access is denied."));
        assert!(text.contains("may be lost"));
    }
}
