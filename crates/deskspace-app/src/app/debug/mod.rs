//! The debug window (`--debug`, ROADMAP D14): its panel and actions, and
//! the feed that moves recorded log lines into its console.
//!
//! A control is a `DebugId` (`ids.rs`), a `DEBUG_CONTROLS` row
//! (`table.rs`), and an arm in both `debug_control` (what it shows) and
//! `on_debug_control` (what it does). Each section's actions live in a file
//! named for it.

mod animations;
mod config;
mod diagnostics;
mod ids;
mod input;
mod simulate;
mod spaces;
mod system;
mod table;
mod takeover;
mod test_spaces;
mod windows;

use std::path::Path;
use std::time::Instant;

use deskspace_core::SpaceId;

use deskspace_platform::{
    DebugControl, DebugControlKind, DebugEvent, DebugPanel, DebugSection, LogLevel, Ui,
};

use super::{App, Timer};
use crate::debug_log::DebugLog;
use ids::DebugId;
use table::{DEBUG_CONTROLS, DebugSectionId};
use test_spaces::STRESS_SIZES;

/// How often new log lines are moved into the console, in milliseconds.
const DEBUG_LOG_MS: u32 = 100;
/// How often the panel's values are read again, in milliseconds.
const DEBUG_REFRESH_MS: u32 = 1000;

/// The debug window's session state; present only with `--debug`. Nothing
/// here is saved (ROADMAP D14).
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
    /// The chosen `STRESS_SIZES` index.
    stress_size: usize,
    /// The space an effect preview plays on; a deleted one falls back to the
    /// first space.
    preview_space: Option<SpaceId>,
    /// The chosen `PREVIEW_KINDS` index.
    preview_kind: usize,
    /// The configuration file's permissions before the debug window made it
    /// read-only, put back when it is turned off or DeskSpace closes.
    writable: Option<std::fs::Permissions>,
    /// Whether every event the app receives is logged (`trace_event`).
    log_events: bool,
    /// How many marker lines "Mark the log" has written.
    marks: u32,
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
            stress_size: 0,
            preview_space: None,
            preview_kind: 0,
            writable: None,
            log_events: false,
            marks: 0,
        }
    }
}

/// What the user did to a control. A control's kind decides which it
/// sends: a `Button` only presses, a `Toggle` only toggles, and a `Choice`
/// only chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugInput {
    Press,
    Toggle(bool),
    Choose(usize),
}

impl App {
    /// What control `id` shows now.
    fn debug_control(&self, ui: &Ui, id: DebugId) -> DebugControlKind {
        let has_spaces = !self.config.desktop.spaces().is_empty();
        match id {
            DebugId::SpacesVisible => DebugControlKind::Toggle { on: self.visible },
            DebugId::OnTop => DebugControlKind::Toggle { on: ui.is_on_top() },
            DebugId::WindowTable => self.window_table(ui),
            DebugId::MonitorTable => self.monitor_table(),
            DebugId::CreateTestSpaces
            | DebugId::CreateStress
            | DebugId::DumpConfig
            | DebugId::OpenConfigFolder
            | DebugId::OpenStorageFolder
            | DebugId::CopyDiagnostics => DebugControlKind::Button { enabled: true },
            DebugId::Build => DebugControlKind::Readout {
                text: self.build_text(),
            },
            DebugId::Model => DebugControlKind::Readout {
                text: self.model_text(),
            },
            DebugId::Resources => DebugControlKind::Readout {
                text: self.resources_text(),
            },
            DebugId::Updates => DebugControlKind::Readout {
                text: self.updates_text(),
            },
            DebugId::Theme => DebugControlKind::Readout {
                text: self.theme_text(),
            },
            DebugId::Explorer => DebugControlKind::Readout {
                text: self.explorer_text(),
            },
            DebugId::Hotkeys => DebugControlKind::Readout {
                text: self.hotkeys_text(),
            },
            DebugId::Hooks => DebugControlKind::Readout {
                text: self.hooks_text(ui),
            },
            DebugId::Speed => self.speed_choice(),
            DebugId::ThemeChanged
            | DebugId::AnimationsChanged
            | DebugId::ExplorerRestarted
            | DebugId::CrashNow
            | DebugId::SaveNow
            | DebugId::ClearIconCache => DebugControlKind::Button { enabled: true },
            DebugId::ReflowAll => DebugControlKind::Button {
                enabled: has_spaces,
            },
            DebugId::SpaceTable => self.space_table(),
            DebugId::MarkLog => DebugControlKind::Button { enabled: true },
            DebugId::LogEvents => DebugControlKind::Toggle {
                on: self.debug.as_ref().is_some_and(|tools| tools.log_events),
            },
            DebugId::InputState => DebugControlKind::Readout {
                text: self.input_text(),
            },
            DebugId::ReadOnlyConfig => DebugControlKind::Toggle {
                on: self.config_read_only(),
            },
            DebugId::Takeover => DebugControlKind::Toggle {
                on: self.desktop_takeover(),
            },
            DebugId::ResyncLayer => DebugControlKind::Button {
                enabled: self.desktop_takeover(),
            },
            DebugId::TakeoverState => DebugControlKind::Readout {
                text: self.takeover_text(),
            },
            DebugId::PreviewSpace => self.preview_space_choice(),
            DebugId::PreviewKind => self.preview_kind_choice(),
            DebugId::PreviewPlay => DebugControlKind::Button {
                enabled: has_spaces,
            },
            DebugId::IgnoreWindows => DebugControlKind::Toggle {
                on: self.animation_tweak.ignore_windows,
            },
            DebugId::ConfigPath => DebugControlKind::Readout {
                text: self.config_path_text(),
            },
            DebugId::StoragePath => DebugControlKind::Readout {
                text: self.storage_path_text(),
            },
            DebugId::DeleteAllSpaces
            | DebugId::UnfoldAll
            | DebugId::UnlockAll
            | DebugId::RefreshAll
            | DebugId::PruneNow => DebugControlKind::Button {
                enabled: has_spaces,
            },
            DebugId::StressSize => DebugControlKind::Choice {
                options: STRESS_SIZES.iter().map(|size| size.to_string()).collect(),
                selected: self.debug.as_ref().map_or(0, |tools| tools.stress_size),
            },
        }
    }

    /// What control `id` does with `input`. A button's arm ignores `input`,
    /// since a button only presses.
    fn on_debug_control(&mut self, ui: &mut Ui, id: DebugId, input: DebugInput) {
        match id {
            DebugId::SpacesVisible => {
                if let DebugInput::Toggle(on) = input {
                    self.set_visible(ui, on);
                    tracing::info!(visible = on, "debug: spaces shown or hidden");
                }
            }
            DebugId::OnTop => {
                if let DebugInput::Toggle(on) = input {
                    self.set_on_top(ui, on);
                }
            }
            DebugId::WindowTable | DebugId::MonitorTable => {}
            DebugId::CreateTestSpaces => self.create_test_spaces(ui),
            DebugId::StressSize => {
                if let (DebugInput::Choose(index), Some(tools)) = (input, self.debug.as_mut())
                    && index < STRESS_SIZES.len()
                {
                    tools.stress_size = index;
                }
            }
            DebugId::CreateStress => self.create_stress_space(ui),
            DebugId::UnfoldAll => self.unfold_all(ui),
            DebugId::UnlockAll => self.unlock_all(ui),
            DebugId::RefreshAll => {
                self.refresh_spaces(ui);
                tracing::info!("debug: refreshed every space");
            }
            DebugId::PruneNow => {
                self.prune_all_missing(ui);
                tracing::info!("debug: pruned missing items");
            }
            DebugId::DeleteAllSpaces => self.delete_all_spaces(ui),
            DebugId::ConfigPath
            | DebugId::StoragePath
            | DebugId::Build
            | DebugId::Model
            | DebugId::Resources
            | DebugId::Updates
            | DebugId::Theme
            | DebugId::Explorer
            | DebugId::Hotkeys
            | DebugId::Hooks => {}
            DebugId::CopyDiagnostics => self.copy_diagnostics(ui),
            DebugId::Speed => {
                if let DebugInput::Choose(index) = input {
                    self.set_speed(index);
                }
            }
            DebugId::PreviewSpace => {
                if let DebugInput::Choose(index) = input {
                    self.choose_preview_space(index);
                }
            }
            DebugId::PreviewKind => {
                if let (DebugInput::Choose(index), Some(tools)) = (input, self.debug.as_mut()) {
                    tools.preview_kind = index;
                }
            }
            DebugId::PreviewPlay => self.play_preview(ui),
            DebugId::Takeover => {
                if let DebugInput::Toggle(on) = input {
                    self.set_session_takeover(ui, on);
                    tracing::info!(on, "debug: desktop takeover for this session");
                }
            }
            DebugId::ResyncLayer => {
                self.resync_desktop_layer(ui);
                tracing::info!("debug: re-synced the desktop layer");
            }
            DebugId::TakeoverState => {}
            DebugId::ThemeChanged => self.simulate_theme_change(ui),
            DebugId::ExplorerRestarted => self.simulate_explorer_restart(ui),
            DebugId::ReadOnlyConfig => {
                if let DebugInput::Toggle(on) = input {
                    self.set_config_read_only(ui, on);
                }
            }
            DebugId::CrashNow => self.crash_now(ui),
            DebugId::SpaceTable | DebugId::InputState => {}
            DebugId::MarkLog => self.mark_log(),
            DebugId::LogEvents => {
                if let DebugInput::Toggle(on) = input {
                    self.set_log_events(on);
                }
            }
            DebugId::ReflowAll => {
                self.relayout_all(ui);
                tracing::info!("debug: re-flowed every space");
            }
            DebugId::SaveNow => {
                self.flush_save(ui);
                tracing::info!("debug: saved now");
            }
            DebugId::ClearIconCache => self.clear_icon_cache(ui),
            DebugId::AnimationsChanged => self.simulate_animations_change(ui),
            DebugId::IgnoreWindows => {
                if let DebugInput::Toggle(on) = input {
                    self.animation_tweak.ignore_windows = on;
                    tracing::info!(on, "debug: ignore Windows' animation switch");
                }
            }
            DebugId::DumpConfig => self.dump_config(),
            DebugId::OpenConfigFolder => self.open_config_folder(ui),
            DebugId::OpenStorageFolder => self.open_storage_folder(ui),
        }
    }

    /// Opens the debug window, or brings it to the front, and starts the
    /// log feed and the panel refresh. A reopened window gets the whole ring
    /// again; one already open keeps what it has.
    pub(super) fn open_debug(&mut self, ui: &mut Ui) {
        let panel = self.debug_panel(ui);
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
    fn debug_panel(&self, ui: &Ui) -> DebugPanel {
        let sections = DebugSectionId::ALL
            .into_iter()
            .filter_map(|section| {
                let controls: Vec<DebugControl> = DEBUG_CONTROLS
                    .iter()
                    .filter(|row| row.section == section)
                    .map(|row| DebugControl {
                        id: row.id.id(),
                        label: row.label.to_string(),
                        kind: self.debug_control(ui, row.id),
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

    /// Asks `question` before a destructive debug action, naming the
    /// configuration file and warning when it is the real one.
    fn debug_confirm(&self, ui: &mut Ui, question: &str) -> bool {
        let real = self.debug.as_ref().is_some_and(|tools| tools.real_config);
        ui.confirm(
            "DeskSpace Debug",
            &debug_question(question, self.store.path(), real),
        )
    }

    /// Sends the panel to the window when its values changed since the last
    /// push.
    fn push_debug_panel(&mut self, ui: &mut Ui) {
        let panel = self.debug_panel(ui);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_debug_question_names_the_config_and_warns_when_it_is_real() {
        let config = Path::new(r"C:\Temp\deskspace.toml");
        let scratch = debug_question("Delete?", config, false);
        assert_eq!(
            scratch,
            "Delete?\n\nConfiguration: C:\\Temp\\deskspace.toml"
        );
        let real = debug_question("Delete?", config, true);
        assert!(real.starts_with(&scratch));
        assert!(real.ends_with("This is your real configuration, not a scratch --config."));
    }
}
