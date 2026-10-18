//! The debug window (`--debug`, ROADMAP D14): its panel and actions, and
//! the feed that moves recorded log lines into its console.

use std::time::Instant;

use deskspace_platform::{DebugEvent, DebugPanel, LogLevel, Ui};

use super::{App, Timer};
use crate::debug_log::DebugLog;

/// How often new log lines are moved into the console, in milliseconds.
const DEBUG_LOG_MS: u32 = 100;

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
}

impl DebugTools {
    pub(crate) fn new(log: DebugLog, real_config: bool) -> Self {
        Self {
            log,
            sent: 0,
            real_config,
            started: Instant::now(),
            console_level: LogLevel::Debug,
        }
    }
}

impl App {
    /// Opens the debug window, or brings it to the front, and starts the
    /// log feed. A reopened window gets the whole ring again; one already
    /// open keeps what it has.
    pub(super) fn open_debug(&mut self, ui: &mut Ui) {
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
        ui.open_debug(&self.debug_panel());
        Self::set_timer(ui, Timer::DebugLog, DEBUG_LOG_MS);
    }

    fn debug_panel(&self) -> DebugPanel {
        DebugPanel {
            sections: Vec::new(),
            console_level: self
                .debug
                .as_ref()
                .map_or(LogLevel::Debug, |debug| debug.console_level),
        }
    }

    /// `Timer::DebugLog`: moves the lines recorded since the last tick into
    /// the console. A timer rather than a waker, so a line logged while
    /// draining waits for the next tick instead of feeding itself.
    pub(super) fn feed_debug_log(&mut self, ui: &mut Ui) {
        let Some(debug) = self.debug.as_mut().filter(|_| ui.is_debug_open()) else {
            Self::clear_timer(ui, Timer::DebugLog);
            return;
        };
        let lines = debug.log.buffer.since(debug.sent);
        if let Some(last) = lines.last() {
            debug.sent = last.seq;
        }
        ui.append_debug_log(&lines);
    }

    pub(super) fn on_debug(&mut self, ui: &mut Ui, event: DebugEvent) {
        let Some(debug) = self.debug.as_mut() else {
            return;
        };
        match event {
            DebugEvent::ConsoleLevel { level } => {
                debug.console_level = level;
                debug.log.level.set(level);
            }
            DebugEvent::Closed => Self::clear_timer(ui, Timer::DebugLog),
            // TODO(X5): the panel's controls.
            DebugEvent::Pressed { .. } | DebugEvent::Toggled { .. } | DebugEvent::Chose { .. } => {}
        }
    }
}
