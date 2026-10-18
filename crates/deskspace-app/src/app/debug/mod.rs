//! The debug window (`--debug`, ROADMAP D14): its panel and actions, and
//! the feed that moves recorded log lines into its console.
//!
//! A control is a `DebugId` (`ids.rs`), a `DEBUG_CONTROLS` row
//! (`table.rs`), and an arm in both `debug_control` (what it shows) and
//! `on_debug_control` (what it does).

mod ids;
mod table;

use std::time::Instant;

use deskspace_platform::{
    DebugControl, DebugControlKind, DebugEvent, DebugPanel, DebugSection, LogLevel, Ui,
};

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
        }
    }

    /// What control `id` does with `input`.
    fn on_debug_control(&mut self, ui: &mut Ui, id: DebugId, input: DebugInput) {
        match (id, input) {
            (DebugId::SpacesVisible, DebugInput::Toggle(on)) => {
                self.set_visible(ui, on);
                tracing::info!(visible = on, "debug: spaces shown or hidden");
            }
            (DebugId::SpacesVisible, DebugInput::Press | DebugInput::Choose(_)) => {}
        }
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
