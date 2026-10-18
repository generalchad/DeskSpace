//! The debug window (`--debug`, ROADMAP D14): its panel and actions, and
//! the feed that moves recorded log lines into its console.
//!
//! A control is a `DebugId` (`ids.rs`), a `DEBUG_CONTROLS` row
//! (`table.rs`), and an arm in both `debug_control` (what it shows) and
//! `on_debug_control` (what it does).

mod ids;
mod table;

use std::path::Path;
use std::time::Instant;

use deskspace_platform::{
    DebugControl, DebugControlKind, DebugEvent, DebugPanel, DebugSection, LogLevel, Ui,
};

use super::files::{StoredFiles, stored_paths};
use super::{App, Timer};
use crate::debug_log::DebugLog;
use ids::DebugId;
use table::{DEBUG_CONTROLS, DebugSectionId};

/// How often new log lines are moved into the console, in milliseconds.
const DEBUG_LOG_MS: u32 = 100;
/// How often the panel's values are read again, in milliseconds.
const DEBUG_REFRESH_MS: u32 = 1000;

/// The debug window's session state; present only with `--debug`.
pub(crate) struct DebugTools {
    log: DebugLog,
    /// The `seq` of the last log line handed to the window.
    sent: u64,
    /// No `--config` was given, so actions touch the real configuration.
    real_config: bool,
    started: Instant,
    /// The console layer's level, as last set from the console.
    console_level: LogLevel,
    /// The panel last pushed to the window, so an unchanged one is not sent.
    shown: Option<DebugPanel>,
}

impl DebugTools {
    pub(crate) fn new(log: DebugLog, real_config: bool) -> Self {
        Self {
            log,
            sent: 0,
            real_config,
            started: Instant::now(),
            console_level: LogLevel::Debug,
            shown: None,
        }
    }
}

/// What the user did to a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugInput {
    Press,
    Toggle(bool),
    Choose(usize),
}

impl App {
    /// Opens the debug window, or brings it to the front, and starts the
    /// log feed and the panel refresh. A reopened window gets the whole ring
    /// again; one already open keeps what it has.
    pub(super) fn open_debug(&mut self, ui: &mut Ui) {
        let panel = self.debug_panel();
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        if !ui.is_debug_open() {
            tools.sent = 0;
        }
        tracing::info!(
            real_config = tools.real_config,
            uptime_ms = tools.started.elapsed().as_millis(),
            "debug: window opened"
        );
        ui.open_debug(&panel);
        tools.shown = Some(panel);
        Self::set_timer(ui, Timer::DebugLog, DEBUG_LOG_MS);
        Self::set_timer(ui, Timer::DebugRefresh, DEBUG_REFRESH_MS);
    }

    /// One section per `DebugSectionId` that has rows, each control from its
    /// row and its current value.
    fn debug_panel(&self) -> DebugPanel {
        let sections = DebugSectionId::ALL
            .into_iter()
            .filter_map(|section| {
                let controls: Vec<DebugControl> = DEBUG_CONTROLS
                    .iter()
                    .filter(|row| row.section == section)
                    .map(|row| DebugControl {
                        id: row.id.id(),
                        label: row.label.to_string(),
                        kind: self.debug_control(row.id),
                    })
                    .collect();
                (!controls.is_empty()).then(|| DebugSection {
                    title: section.title().to_string(),
                    controls,
                })
            })
            .collect();
        DebugPanel {
            sections,
            console_level: self
                .debug
                .as_ref()
                .map_or(LogLevel::Debug, |tools| tools.console_level),
        }
    }

    /// What control `id` shows now.
    fn debug_control(&self, id: DebugId) -> DebugControlKind {
        match id {
            DebugId::SpacesVisible => DebugControlKind::Toggle { on: self.visible },
            DebugId::DeleteAllSpaces => DebugControlKind::Button {
                enabled: !self.config.desktop.spaces().is_empty(),
            },
        }
    }

    /// What control `id` does with `input`.
    fn on_debug_control(&mut self, ui: &mut Ui, id: DebugId, input: DebugInput) {
        match (id, input) {
            (DebugId::SpacesVisible, DebugInput::Toggle(on)) => {
                self.set_visible(ui, on);
                tracing::info!(visible = on, "debug: spaces shown or hidden");
            }
            (DebugId::DeleteAllSpaces, DebugInput::Press) => self.delete_all_spaces(ui),
            (DebugId::SpacesVisible, DebugInput::Press | DebugInput::Choose(_))
            | (DebugId::DeleteAllSpaces, DebugInput::Toggle(_) | DebugInput::Choose(_)) => {}
        }
    }

    /// Asks `question` before a destructive debug action, naming the
    /// configuration file and warning when it is the real one.
    fn debug_confirm(&self, ui: &mut Ui, question: &str) -> bool {
        let real = self.debug.as_ref().is_some_and(|tools| tools.real_config);
        ui.confirm(
            "DeskSpace Debug",
            &debug_question(question, self.store.path(), real),
        )
    }

    /// Delete all spaces (ROADMAP D14): every space goes as Delete space
    /// would, but stored files go to the Recycle Bin instead of the desktop.
    fn delete_all_spaces(&mut self, ui: &mut Ui) {
        let spaces = self.config.desktop.spaces();
        let items = spaces.iter().map(|space| space.items().len()).sum();
        let stored = spaces
            .iter()
            .map(|space| stored_paths(space, &self.storage).len())
            .sum();
        let ids: Vec<_> = spaces.iter().map(|space| space.id).collect();
        let question = delete_all_question(ids.len(), items, stored);
        if !self.debug_confirm(ui, &question) {
            return;
        }
        for id in &ids {
            self.delete_space_now(ui, *id, StoredFiles::ToRecycleBin);
        }
        let kept = ids
            .iter()
            .filter(|id| self.config.desktop.space(**id).is_some())
            .count();
        tracing::info!(
            deleted = ids.len() - kept,
            kept,
            "debug: deleted all spaces"
        );
    }

    /// Sends the panel to the window when its values changed since the last
    /// push.
    fn push_debug_panel(&mut self, ui: &mut Ui) {
        let panel = self.debug_panel();
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        if tools.shown.as_ref() == Some(&panel) {
            return;
        }
        ui.update_debug(&panel);
        tools.shown = Some(panel);
    }

    /// `Timer::DebugRefresh`: reads the panel's values again while the
    /// window is open, so it follows changes made elsewhere.
    pub(super) fn refresh_debug(&mut self, ui: &mut Ui) {
        if self.debug.is_none() || !ui.is_debug_open() {
            Self::clear_timer(ui, Timer::DebugRefresh);
            return;
        }
        self.push_debug_panel(ui);
    }

    /// `Timer::DebugLog`: moves the lines recorded since the last tick into
    /// the console. A timer rather than a waker, so a line logged while
    /// draining waits for the next tick instead of feeding itself.
    pub(super) fn feed_debug_log(&mut self, ui: &mut Ui) {
        let Some(tools) = self.debug.as_mut().filter(|_| ui.is_debug_open()) else {
            Self::clear_timer(ui, Timer::DebugLog);
            return;
        };
        let lines = tools.log.buffer.since(tools.sent);
        if let Some(last) = lines.last() {
            tools.sent = last.seq;
        }
        ui.append_debug_log(&lines);
    }

    pub(super) fn on_debug(&mut self, ui: &mut Ui, event: DebugEvent) {
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        let (id, input) = match event {
            DebugEvent::ConsoleLevel { level } => {
                tools.console_level = level;
                tools.log.level.set(level);
                return;
            }
            DebugEvent::Closed => {
                tools.shown = None;
                Self::clear_timer(ui, Timer::DebugLog);
                Self::clear_timer(ui, Timer::DebugRefresh);
                return;
            }
            DebugEvent::Pressed { id } => (id, DebugInput::Press),
            DebugEvent::Toggled { id, on } => (id, DebugInput::Toggle(on)),
            DebugEvent::Chose { id, index } => (id, DebugInput::Choose(index)),
        };
        if let Some(id) = DebugId::from_id(id) {
            self.on_debug_control(ui, id, input);
        }
        self.push_debug_panel(ui);
    }
}

/// A destructive action's question with the configuration file under it,
/// and a warning when that is the real configuration.
fn debug_question(question: &str, config: &Path, real: bool) -> String {
    let mut text = format!("{question}\n\nConfiguration: {}", config.display());
    if real {
        text.push_str("\n\nThis is your real configuration, not a scratch --config.");
    }
    text
}

/// `count` and `noun`, plural unless `count` is 1.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Delete all spaces' question: how many spaces and items go, and where the
/// files go.
fn delete_all_question(spaces: usize, items: usize, stored: usize) -> String {
    let (ask, hold) = if spaces == 1 {
        ("Delete the only space?".to_string(), "It holds")
    } else {
        (format!("Delete all {spaces} spaces?"), "They hold")
    };
    let go = if stored == 1 { "goes" } else { "go" };
    format!(
        "{ask}\n\n{hold} {}. {} {go} to the Recycle Bin; items shown in place \
         are only removed from their spaces.",
        counted(items, "item"),
        counted(stored, "stored file"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_debug_question_names_the_config_and_warns_when_it_is_real() {
        let scratch = debug_question("Delete?", Path::new(r"C:\Temp\deskspace.toml"), false);
        assert_eq!(
            scratch,
            "Delete?\n\nConfiguration: C:\\Temp\\deskspace.toml"
        );
        let real = debug_question("Delete?", Path::new(r"C:\Temp\deskspace.toml"), true);
        assert!(real.starts_with(&scratch));
        assert!(real.ends_with("This is your real configuration, not a scratch --config."));
    }

    #[test]
    fn delete_all_says_how_many_and_where_files_go() {
        assert_eq!(
            delete_all_question(3, 12, 5),
            "Delete all 3 spaces?\n\nThey hold 12 items. 5 stored files go to the \
             Recycle Bin; items shown in place are only removed from their spaces."
        );
        assert_eq!(
            delete_all_question(1, 1, 0),
            "Delete the only space?\n\nIt holds 1 item. 0 stored files go to the \
             Recycle Bin; items shown in place are only removed from their spaces."
        );
        assert!(delete_all_question(2, 2, 1).contains("1 stored file goes"));
    }
}
