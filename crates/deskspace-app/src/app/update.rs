//! Update checking and installation (ROADMAP F-45 and D6; tasks E10/E11 in
//! `docs/archive/TASKS-done.md`).
//!
//! The work runs on a worker thread and hands its result back through the
//! platform's `Waker` (`Event::Wake`); the app never blocks on the network
//! except while it shows the modal prompt. A manual check is started from the
//! tray's "Check for updates…" or the Settings button; the opt-in automatic
//! check runs about once a day.
//!
//! An update is installed in three steps: download and verify the package on
//! a worker thread, quit through the normal teardown (save, restore
//! Explorer's icons, tell the watchdog the exit was clean), and then, once the
//! message loop has ended, run the installer (`InstallOnExit`, called by
//! `bootstrap::run`). With "Install updates automatically" on, the downloaded
//! package waits for a quiet moment (`is_quiet_moment`) and installs silently;
//! the tray offers to restart sooner.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use deskspace_platform::{Ui, idle_time, user_is_presenting};
use deskspace_update::{Available, InstallMode, Package};

use super::{
    App, UPDATE_DOWNLOADED_WAKE, UPDATE_IDLE_TIMER, UPDATE_TIMER, UPDATE_WAKE, tray_tooltip,
};

/// The first automatic check runs shortly after start, then hourly while the
/// app is open (each hourly tick checks whether a day has passed).
const UPDATE_FIRST_DELAY_MS: u32 = 60_000;
const UPDATE_TICK_MS: u32 = 3_600_000;
/// How often an automatic check actually contacts GitHub.
const UPDATE_INTERVAL_SECS: u64 = 24 * 60 * 60;
/// How often a downloaded automatic update looks for a quiet moment.
const UPDATE_IDLE_POLL_MS: u32 = 60_000;
/// How long the user must have been away before an automatic update restarts
/// DeskSpace.
const INSTALL_AFTER_IDLE: Duration = Duration::from_secs(10 * 60);
/// How much of the release notes to show in the prompt.
const NOTES_LIMIT: usize = 600;

/// A result handed back from an update worker thread.
enum Message {
    /// Boxed: a found update is much larger than the other variant.
    Checked(Box<Result<Option<Available>, String>>),
    Downloaded(Box<Result<Package, String>>),
}

/// Why a download was started, which decides what happens once it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadReason {
    /// The user chose "Update and restart": install at once, and report
    /// failures.
    UserAsked,
    /// "Install updates automatically": wait for a quiet moment, and stay
    /// quiet about failures.
    Automatic,
}

/// The package to install once the message loop has ended, shared between
/// the app (which fills it before quitting) and `bootstrap::run` (which runs
/// it after `deskspace_platform::run` returns).
#[derive(Clone, Default)]
pub struct InstallOnExit(Rc<RefCell<Option<(Package, InstallMode)>>>);

impl InstallOnExit {
    /// Runs the queued installer, if any. On success the updater ends the
    /// process, so this returns only when nothing was queued or it failed.
    pub fn install(&self) -> anyhow::Result<()> {
        let Some((package, mode)) = self.0.borrow_mut().take() else {
            return Ok(());
        };
        tracing::info!(version = package.version, ?mode, "installing update");
        deskspace_update::install(package, mode)?;
        Ok(())
    }

    fn queue(&self, package: Package, mode: InstallMode) {
        *self.0.borrow_mut() = Some((package, mode));
    }
}

/// Update state, owned by `App`.
#[derive(Default)]
pub(super) struct UpdateState {
    /// The update a completed check found, shared with the download worker
    /// so the app does not have to check again.
    available: Option<Arc<Available>>,
    /// The result channel of the in-flight check or download; `Some` while
    /// one runs, so only one is in flight at a time.
    receiver: Option<Receiver<Message>>,
    /// Whether the in-flight check was started by the user (report up-to-date
    /// and errors) or by the automatic timer (stay quiet).
    manual: bool,
    /// Why the in-flight download was started.
    download_reason: Option<DownloadReason>,
    /// A downloaded automatic update waiting for a quiet moment.
    ready: Option<Package>,
    on_exit: InstallOnExit,
}

impl UpdateState {
    fn busy(&self) -> bool {
        self.receiver.is_some()
    }

    /// The version of a downloaded update waiting to be installed.
    pub(super) fn ready_version(&self) -> Option<&str> {
        self.ready.as_ref().map(|package| package.version.as_str())
    }
}

impl App {
    /// The handle `bootstrap::run` uses to install a queued update after the
    /// message loop ends.
    pub fn install_on_exit(&self) -> InstallOnExit {
        self.update.on_exit.clone()
    }

    /// Arms or clears the automatic update check to match the setting. Called
    /// on `Started` and when Settings applies a change; the timer reschedules
    /// itself.
    pub(super) fn sync_update_schedule(&mut self, ui: &mut Ui) {
        if self.config.window.check_for_updates && deskspace_update::is_configured() {
            ui.set_timer(UPDATE_TIMER, UPDATE_FIRST_DELAY_MS);
        } else {
            ui.clear_timer(UPDATE_TIMER);
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
    /// flight. A manual check with an update already downloaded offers to
    /// install it instead of checking again.
    pub(super) fn start_update_check(&mut self, ui: &mut Ui, manual: bool) {
        if self.update.busy() {
            return;
        }
        if self.update.ready.is_some() {
            if manual {
                self.offer_ready_update(ui);
            }
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

    /// Offers a found update, or downloads it without asking when automatic
    /// installs are on. An automatic check stays quiet about a version the
    /// user skipped.
    fn offer_update(&mut self, ui: &mut Ui, available: Available, manual: bool) {
        if !manual
            && self.config.window.skipped_version.as_deref() == Some(available.version.as_str())
        {
            return;
        }
        let installed = deskspace_update::is_installed();
        if !manual && installed && self.config.window.install_updates {
            tracing::info!(version = available.version, "downloading update");
            self.update.available = Some(Arc::new(available));
            self.start_update_download(ui, DownloadReason::Automatic);
            return;
        }
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
                self.start_update_download(ui, DownloadReason::UserAsked);
            }
            0 => ui.open_url(&available.page_url),
            2 => {
                self.config.window.skipped_version = Some(available.version.clone());
                self.schedule_save(ui);
            }
            _ => {}
        }
    }

    /// Asks whether to install the downloaded update now.
    fn offer_ready_update(&mut self, ui: &mut Ui) {
        let Some(version) = self.update.ready_version() else {
            return;
        };
        let text = format!(
            "DeskSpace {version} is downloaded. It installs on its own the next time you \
             are away, or you can restart now."
        );
        let choice = ui.choose("Update ready", &text, &["Restart now", "Later"], 0);
        if choice == Some(0) {
            self.install_ready_update(ui, InstallMode::Passive);
        }
    }

    /// Downloads and verifies the update on a worker thread.
    fn start_update_download(&mut self, ui: &mut Ui, reason: DownloadReason) {
        if self.update.busy() {
            return;
        }
        let Some(available) = self.update.available.clone() else {
            return;
        };
        if reason == DownloadReason::UserAsked {
            ui.set_tray_tooltip("Downloading update\u{2026}");
        }
        let (sender, receiver) = mpsc::channel();
        self.update.receiver = Some(receiver);
        self.update.download_reason = Some(reason);
        let waker = ui.waker();
        std::thread::spawn(move || {
            let result = deskspace_update::download(&available).map_err(|error| error.to_string());
            let _ = sender.send(Message::Downloaded(Box::new(result)));
            waker.wake(UPDATE_DOWNLOADED_WAKE);
        });
    }

    /// Handles the result of a download: install at once when the user asked,
    /// otherwise keep the package until a quiet moment.
    pub(super) fn on_update_downloaded(&mut self, ui: &mut Ui) {
        let Some(receiver) = self.update.receiver.take() else {
            return;
        };
        let reason = self.update.download_reason.take();
        let Ok(Message::Downloaded(result)) = receiver.recv() else {
            return;
        };
        self.update.available = None;
        match (*result, reason) {
            (Ok(package), Some(DownloadReason::UserAsked)) => {
                self.install_and_quit(ui, package, InstallMode::Passive);
            }
            (Ok(package), _) => {
                tracing::info!(version = package.version, "update ready to install");
                self.update.ready = Some(package);
                self.refresh_tray(ui);
                ui.set_timer(UPDATE_IDLE_TIMER, UPDATE_IDLE_POLL_MS);
            }
            (Err(error), Some(DownloadReason::UserAsked)) => {
                tracing::error!(%error, "update download failed");
                self.refresh_tray(ui);
                ui.choose("Update failed", &error, &["OK"], 0);
            }
            (Err(error), _) => tracing::warn!(%error, "automatic update download failed"),
        }
    }

    /// The idle poll: install the downloaded update once the user is away.
    /// Turning automatic installs off stops the poll but keeps the package,
    /// so the tray can still offer it.
    pub(super) fn on_update_idle_timer(&mut self, ui: &mut Ui) {
        if self.update.ready.is_none() || !self.config.window.install_updates {
            ui.clear_timer(UPDATE_IDLE_TIMER);
            return;
        }
        // An open settings window or a rename would lose unapplied edits.
        let mid_task = self.settings.is_some() || self.edit.is_active();
        if !mid_task && is_quiet_moment(idle_time(), user_is_presenting()) {
            self.install_ready_update(ui, InstallMode::Silent);
        }
    }

    /// Installs the downloaded update (the tray's "Restart to update" entry,
    /// or a quiet moment).
    pub(super) fn install_ready_update(&mut self, ui: &mut Ui, mode: InstallMode) {
        if let Some(package) = self.update.ready.take() {
            ui.clear_timer(UPDATE_IDLE_TIMER);
            self.install_and_quit(ui, package, mode);
        }
    }

    /// Queues the installer and quits; `bootstrap::run` runs it once the
    /// teardown has restored the desktop.
    fn install_and_quit(&mut self, ui: &mut Ui, package: Package, mode: InstallMode) {
        self.flush_save(ui);
        self.update.on_exit.queue(package, mode);
        ui.quit();
    }

    /// The tray tooltip: the space summary, plus a waiting update.
    pub(super) fn tray_text(&self) -> String {
        let summary = tray_tooltip(&self.config.desktop);
        match self.update.ready_version() {
            Some(version) => format!("{summary}\nUpdate {version} ready to install"),
            None => summary,
        }
    }
}

/// Whether an automatic check is due, given the last one's Unix time and now.
fn check_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now.saturating_sub(last) >= UPDATE_INTERVAL_SECS)
}

/// Whether DeskSpace may restart itself for an automatic update: the user has
/// been away long enough and is not watching or presenting something.
fn is_quiet_moment(idle: Duration, presenting: bool) -> bool {
    idle >= INSTALL_AFTER_IDLE && !presenting
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
    fn an_automatic_install_waits_until_the_user_is_away() {
        assert!(!is_quiet_moment(Duration::ZERO, false));
        assert!(!is_quiet_moment(
            INSTALL_AFTER_IDLE - Duration::from_secs(1),
            false
        ));
        assert!(is_quiet_moment(INSTALL_AFTER_IDLE, false));
        // A full-screen video or presentation sends no input, but is not away.
        assert!(!is_quiet_moment(INSTALL_AFTER_IDLE * 6, true));
    }

    #[test]
    fn nothing_queued_installs_nothing() {
        assert!(InstallOnExit::default().install().is_ok());
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
