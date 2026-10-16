//! Update checking and installation (ROADMAP F-45; tasks E10/E11 in
//! `docs/archive/TASKS-done.md`).
//!
//! The work runs on a worker thread and hands its result back through the
//! platform's `Waker` (`Event::Wake`); the app never blocks on the network
//! except while it shows the modal prompt. A manual check is started from the
//! tray's "Check for updates…" or the Settings button; the opt-in automatic
//! check runs about once a day.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

use deskspace_platform::Ui;

use super::{App, UPDATE_INSTALLED_WAKE, UPDATE_TIMER, UPDATE_WAKE, tray_tooltip};

/// The first automatic check runs shortly after start, then hourly while the
/// app is open (each hourly tick checks whether a day has passed).
const UPDATE_FIRST_DELAY_MS: u32 = 60_000;
const UPDATE_TICK_MS: u32 = 3_600_000;
/// How often an automatic check actually contacts GitHub.
const UPDATE_INTERVAL_SECS: u64 = 24 * 60 * 60;
/// How much of the release notes to show in the prompt.
const NOTES_LIMIT: usize = 600;

/// A result handed back from an update worker thread.
enum Message {
    /// Boxed: a found update is much larger than the other variant.
    Checked(Box<Result<Option<deskspace_update::Available>, String>>),
    Installed(Result<(), String>),
}

/// Update state, owned by `App`.
#[derive(Default)]
pub(super) struct UpdateState {
    /// The update a completed check found, shared with the install worker so
    /// the app does not have to check again.
    available: Option<Arc<deskspace_update::Available>>,
    /// The result channel of the in-flight check or install; `Some` while one
    /// runs, so only one is in flight at a time.
    receiver: Option<Receiver<Message>>,
    /// Whether the in-flight check was started by the user (report up-to-date
    /// and errors) or by the automatic timer (stay quiet).
    manual: bool,
}

impl UpdateState {
    fn busy(&self) -> bool {
        self.receiver.is_some()
    }
}

impl App {
    /// Arms the automatic update check when the setting is on and this build
    /// can check. Called once on `Started`; the timer reschedules itself.
    pub(super) fn start_auto_update_checks(&mut self, ui: &mut Ui) {
        if self.config.window.check_for_updates && deskspace_update::is_configured() {
            ui.set_timer(UPDATE_TIMER, UPDATE_FIRST_DELAY_MS);
        }
    }

    /// The hourly tick: check whether a day has passed and start a check. The
    /// timer is re-armed either way so the schedule keeps ticking.
    pub(super) fn on_update_timer(&mut self, ui: &mut Ui) {
        if !self.config.window.check_for_updates {
            ui.clear_timer(UPDATE_TIMER);
            return;
        }
        ui.set_timer(UPDATE_TIMER, UPDATE_TICK_MS);
        if check_due(self.config.window.last_update_check, now_unix()) {
            self.start_update_check(ui, false);
        }
    }

    /// Starts a check on a worker thread. Does nothing while one is already in
    /// flight.
    pub(super) fn start_update_check(&mut self, ui: &mut Ui, manual: bool) {
        if self.update.busy() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.update.receiver = Some(receiver);
        self.update.manual = manual;
        let waker = ui.waker();
        std::thread::spawn(move || {
            let result = deskspace_update::check().map_err(|error| error.to_string());
            let _ = sender.send(Message::Checked(Box::new(result)));
            waker.wake(UPDATE_WAKE);
        });
    }

    /// Handles the result of a check.
    pub(super) fn on_update_checked(&mut self, ui: &mut Ui) {
        let Some(receiver) = self.update.receiver.take() else {
            return;
        };
        let manual = std::mem::take(&mut self.update.manual);
        let Ok(Message::Checked(result)) = receiver.recv() else {
            return;
        };
        self.config.window.last_update_check = Some(now_unix());
        self.schedule_save(ui);
        match *result {
            Err(error) => {
                if manual {
                    ui.choose("Update check failed", &error, &["OK"], 0);
                } else {
                    tracing::warn!(%error, "automatic update check failed");
                }
            }
            Ok(None) => {
                if manual {
                    ui.choose(
                        "DeskSpace is up to date",
                        "You are running the latest version.",
                        &["OK"],
                        0,
                    );
                }
            }
            Ok(Some(available)) => self.offer_update(ui, available, manual),
        }
    }

    /// Offers a found update. An automatic check stays quiet about a version
    /// the user skipped.
    fn offer_update(&mut self, ui: &mut Ui, available: deskspace_update::Available, manual: bool) {
        if !manual
            && self.config.window.skipped_version.as_deref() == Some(available.version.as_str())
        {
            return;
        }
        let installed = installed_build();
        let actions: &[&str] = if installed {
            &["Update and restart", "Later", "Skip this version"]
        } else {
            &["Open release page", "Later", "Skip this version"]
        };
        let notes = truncated(&available.notes, NOTES_LIMIT);
        let text = format!("DeskSpace {} is available.\n\n{notes}", available.version);
        let Some(choice) = ui.choose("Update available", &text, actions, 0) else {
            return;
        };
        match choice {
            0 if installed => {
                self.update.available = Some(Arc::new(available));
                self.start_update_install(ui);
            }
            0 => ui.open_url(&available.page_url),
            2 => {
                self.config.window.skipped_version = Some(available.version.clone());
                self.schedule_save(ui);
            }
            _ => {}
        }
    }

    /// Downloads and installs the update on a worker thread. The installer
    /// replaces the running app and relaunches it.
    fn start_update_install(&mut self, ui: &mut Ui) {
        if self.update.busy() {
            return;
        }
        let Some(available) = self.update.available.clone() else {
            return;
        };
        // Persist now: the installer terminates this process.
        self.flush_save(ui);
        ui.set_tray_tooltip("Downloading update\u{2026}");
        let (sender, receiver) = mpsc::channel();
        self.update.receiver = Some(receiver);
        let waker = ui.waker();
        std::thread::spawn(move || {
            let result = deskspace_update::download_and_install(&available)
                .map_err(|error| error.to_string());
            let _ = sender.send(Message::Installed(result));
            waker.wake(UPDATE_INSTALLED_WAKE);
        });
    }

    /// Handles the result of a download-and-install.
    pub(super) fn on_update_installed(&mut self, ui: &mut Ui) {
        let Some(receiver) = self.update.receiver.take() else {
            return;
        };
        let Ok(Message::Installed(result)) = receiver.recv() else {
            return;
        };
        match result {
            // The installer relaunches DeskSpace; if it returned without doing
            // so, exit once it is done.
            Ok(()) => std::process::exit(0),
            Err(error) => {
                tracing::error!(%error, "update install failed");
                ui.set_tray_tooltip(&tray_tooltip(&self.config.desktop));
                ui.choose("Update failed", &error, &["OK"], 0);
            }
        }
    }
}

/// Whether an automatic check is due, given the last one's Unix time and now.
fn check_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now.saturating_sub(last) >= UPDATE_INTERVAL_SECS)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Whether this build is the installed one (under `%LOCALAPPDATA%\DeskSpace.App`)
/// rather than a portable copy. A portable build only notifies.
fn installed_build() -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let root = Path::new(&local).join("DeskSpace.App");
    std::env::current_exe().is_ok_and(|exe| exe.starts_with(&root))
}

fn truncated(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max_chars).collect();
    out.push('\u{2026}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_old_check_is_due() {
        assert!(check_due(None, 1_000));
        assert!(!check_due(Some(1_000), 1_000));
        assert!(!check_due(Some(1_000), 1_000 + UPDATE_INTERVAL_SECS - 1));
        assert!(check_due(Some(1_000), 1_000 + UPDATE_INTERVAL_SECS));
        // A clock that jumped back (last in the future) is not due, and must
        // not panic on the subtraction.
        assert!(!check_due(Some(10_000), 1_000));
    }

    #[test]
    fn notes_are_trimmed_and_bounded() {
        assert_eq!(truncated("  hi  ", 10), "hi");
        let long = "a".repeat(100);
        let cut = truncated(&long, 10);
        assert_eq!(cut.chars().count(), 11, "10 chars plus the ellipsis");
        assert!(cut.ends_with('\u{2026}'));
    }
}
