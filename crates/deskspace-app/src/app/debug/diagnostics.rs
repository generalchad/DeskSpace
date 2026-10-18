//! The debug window's Diagnostics section: the build, the model, resources,
//! and updates, and a report of them with the recent log to paste into a
//! bug report.

use std::time::Duration;

use deskspace_platform::{LogLine, Ui, format_line, process_stats};

use crate::app::App;
use crate::app::files::stored_paths;

/// How many recent log lines "Copy diagnostics" includes.
const REPORT_LINES: usize = 200;
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

impl App {
    /// The version, the build profile, and the executable.
    pub(super) fn build_text(&self) -> String {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let exe = std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("unknown ({error})"));
        format!(
            "DeskSpace {} ({profile} build)\n{exe}",
            env!("CARGO_PKG_VERSION")
        )
    }

    /// How much the model holds.
    pub(super) fn model_text(&self) -> String {
        let desktop = &self.config.desktop;
        let stored: usize = desktop
            .spaces()
            .iter()
            .map(|space| stored_paths(space, &self.storage).len())
            .sum();
        format!(
            "Spaces: {}\nItems in spaces: {}\nStored files: {stored}\nDesktop-layer items: {}",
            desktop.spaces().len(),
            desktop.space_item_count(),
            desktop.layer().item_count(),
        )
    }

    /// Uptime, memory, handles, and the icon cache, read anew each refresh.
    pub(super) fn resources_text(&self) -> String {
        let uptime = self
            .debug
            .as_ref()
            .map_or(Duration::ZERO, |tools| tools.started.elapsed());
        let stats = process_stats();
        format!(
            "Uptime: {}\nWorking set: {:.1} MB\nGDI objects: {}\nUSER objects: {}\nIcon cache: {} icons",
            uptime_text(uptime),
            stats.working_set as f64 / BYTES_PER_MB,
            stats.gdi_objects,
            stats.user_objects,
            self.icons.len(),
        )
    }

    /// Whether updates can be checked and installed, and what is waiting.
    pub(super) fn updates_text(&self) -> String {
        let yes_no = |value: bool| if value { "yes" } else { "no" };
        format!(
            "Update feed configured: {}\nInstalled copy: {}\nReady to install: {}",
            yes_no(deskspace_update::is_configured()),
            yes_no(deskspace_update::is_installed()),
            self.update.ready_version().unwrap_or("none"),
        )
    }

    /// Copy diagnostics: every readout above and the recent log, as one text
    /// on the clipboard.
    pub(super) fn copy_diagnostics(&mut self, ui: &mut Ui) {
        let lines = self
            .debug
            .as_ref()
            .map(|tools| tools.log.buffer.since(0))
            .unwrap_or_default();
        let recent = &lines[lines.len().saturating_sub(REPORT_LINES)..];
        let sections = [
            ("Build", self.build_text()),
            ("Model", self.model_text()),
            ("Resources", self.resources_text()),
            ("Updates", self.updates_text()),
            ("Theme", self.theme_text()),
            ("Explorer", self.explorer_text()),
            ("Hotkeys", self.hotkeys_text()),
            ("Hooks", self.hooks_text(ui)),
        ];
        let sections: Vec<(&str, &str)> = sections
            .iter()
            .map(|(title, text)| (*title, text.as_str()))
            .collect();
        ui.set_clipboard_text(&diagnostics_text(&sections, recent));
        tracing::info!(lines = recent.len(), "debug: copied diagnostics");
    }
}

/// `elapsed` as `1h 02m 03s`, or `2m 03s` under an hour.
fn uptime_text(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

/// The report "Copy diagnostics" writes: a heading before each readout, then
/// the log lines, one per line.
fn diagnostics_text(sections: &[(&str, &str)], lines: &[LogLine]) -> String {
    let mut text = String::new();
    for (title, body) in sections {
        text.push_str(&format!("== {title} ==\r\n"));
        text.push_str(&body.lines().collect::<Vec<_>>().join("\r\n"));
        text.push_str("\r\n\r\n");
    }
    text.push_str(&format!("== Log (last {} lines) ==\r\n", lines.len()));
    for line in lines {
        text.push_str(&format_line(line));
        text.push_str("\r\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use deskspace_platform::LogLevel;

    use super::*;

    #[test]
    fn the_report_heads_each_readout_and_ends_with_the_log() {
        let line = LogLine {
            seq: 1,
            millis: 1_500,
            level: LogLevel::Info,
            target: "deskspace".into(),
            message: "spaces created".into(),
        };
        let text = diagnostics_text(
            &[("Build", "v1\nC:\\x.exe"), ("Model", "Spaces: 2")],
            &[line],
        );
        assert_eq!(
            text,
            "== Build ==\r\nv1\r\nC:\\x.exe\r\n\r\n== Model ==\r\nSpaces: 2\r\n\r\n\
             == Log (last 1 lines) ==\r\n00:01.500 INFO  deskspace spaces created\r\n"
        );
    }

    #[test]
    fn uptime_shows_hours_only_when_there_are_some() {
        assert_eq!(uptime_text(Duration::from_secs(123)), "2m 03s");
        assert_eq!(uptime_text(Duration::from_secs(3723)), "1h 02m 03s");
    }
}
