//! The debug window's Takeover section: whether DeskSpace has the desktop,
//! what its layer holds, and whether the watchdog stands by.

use deskspace_platform::watchdog_running;

use crate::app::App;

impl App {
    pub(super) fn takeover_text(&self) -> String {
        let watchdog = match watchdog_running() {
            Some(true) => "running",
            Some(false) => "not running",
            None => "unknown",
        };
        format!(
            "Takeover: {}\nDesktop layer: {}\nLayer items: {}\nWatchdog: {watchdog}",
            if self.desktop_takeover() {
                "active"
            } else {
                "off"
            },
            if self.layer_active() {
                "drawn"
            } else {
                "not drawn"
            },
            self.config.desktop.layer().item_count(),
        )
    }
}
