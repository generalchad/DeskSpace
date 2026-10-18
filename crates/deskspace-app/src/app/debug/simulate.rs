//! The debug window's Simulate section: Windows events on demand, a config
//! file that will not save, and a crash, to check how DeskSpace copes.

use deskspace_platform::{Event, EventHandler, Ui};

use crate::app::App;

impl App {
    /// Delivers the event Windows sends when it switches dark and light.
    pub(super) fn simulate_theme_change(&mut self, ui: &mut Ui) {
        tracing::info!("debug: simulating a Windows theme change");
        self.on_event(Event::SystemThemeChanged, ui);
    }

    /// Posts `TaskbarCreated`, the message Explorer broadcasts when it
    /// restarts.
    pub(super) fn simulate_explorer_restart(&mut self, ui: &mut Ui) {
        tracing::info!("debug: simulating an Explorer restart");
        ui.simulate_taskbar_created();
    }

    /// Whether the configuration file is read-only now.
    pub(super) fn config_read_only(&self) -> bool {
        std::fs::metadata(self.store.path())
            .map(|metadata| metadata.permissions().readonly())
            .unwrap_or(false)
    }

    /// Makes the configuration file read-only (asking first), so every save
    /// fails, or puts back the permissions it had.
    pub(super) fn set_config_read_only(&mut self, ui: &mut Ui, on: bool) {
        if !on {
            self.restore_config_writable();
            return;
        }
        if self.config_read_only() {
            return;
        }
        let question = "Make the configuration file read-only?\n\nEvery save fails \
                        until you turn this off; exiting DeskSpace turns it off.";
        if !self.debug_confirm(ui, question) {
            return;
        }
        let path = self.store.path().to_path_buf();
        let original = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.permissions(),
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "debug: the configuration file cannot be read");
                return;
            }
        };
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        match std::fs::set_permissions(&path, read_only) {
            Ok(()) => {
                if let Some(tools) = self.debug.as_mut() {
                    tools.writable = Some(original);
                }
                tracing::info!(path = %path.display(), "debug: the configuration file is read-only");
            }
            Err(error) => {
                tracing::warn!(%error, "debug: could not make the configuration read-only")
            }
        }
    }

    /// Puts back the configuration file's permissions if the debug window
    /// made it read-only. Called on close before the last save, so a
    /// session never leaves the file read-only.
    pub(crate) fn restore_config_writable(&mut self) {
        let Some(original) = self.debug.as_mut().and_then(|tools| tools.writable.take()) else {
            return;
        };
        match std::fs::set_permissions(self.store.path(), original) {
            Ok(()) => tracing::info!("debug: the configuration file is writable again"),
            Err(error) => {
                tracing::warn!(%error, "debug: could not make the configuration writable")
            }
        }
    }

    /// Crashes DeskSpace (asking first) where a real bug would: in an event
    /// handler, inside the window procedure. The panic hook logs it.
    pub(super) fn crash_now(&mut self, ui: &mut Ui) {
        let question = "Crash DeskSpace now?\n\nIt ends at once, as after a bug; with \
                        takeover on, the watchdog should bring Explorer's icons back.";
        if !self.debug_confirm(ui, question) {
            return;
        }
        tracing::info!("debug: crashing on request");
        panic!("crash requested from the debug window");
    }
}
