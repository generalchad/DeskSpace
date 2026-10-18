//! Crash-safe restore of Explorer's desktop icons for the takeover
//! (ROADMAP D8).
//!
//! Hiding Explorer's `SysListView32` icon layer is only safe if a crash cannot
//! leave the desktop blank. Before the icons are hidden the app launches a
//! small dedicated watchdog (`deskspace-watchdog.exe <pid>`), which waits on the
//! main process and re-shows every listview unless the main process signaled a
//! clean exit. The clean-exit event is created before the icons are hidden and
//! set in the runtime teardown, right after the icons are restored, so the
//! watchdog acts only when the main process died without reaching its
//! teardown.
//!
//! The watchdog is a separate executable with a distinct name, launched
//! detached, so killing DeskSpace by image name, closing its console, or
//! ending its process tree leaves the watchdog running. It is best-effort: if
//! it cannot be launched, the takeover still runs as it did before the
//! watchdog existed. A named mutex makes a stray watchdog a no-op, so two of
//! them never fight over one parent.

use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError, HANDLE,
    SetLastError, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CreateEventW, CreateMutexW, DETACHED_PROCESS, INFINITE, OpenEventW,
    OpenMutexW, OpenProcess, PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    WaitForSingleObject,
};
use windows::core::PCWSTR;

use crate::win32::desktop_icons;
use crate::win32::system::wide;

/// Set on the watchdog copy that waits, so it does not re-parent again.
const REPARENT_ENV: &str = "DESKSPACE_WATCHDOG_REPARENTED";

/// The main process's clean-exit event handle, or 0 while none is armed.
static CLEAN_EXIT: AtomicIsize = AtomicIsize::new(0);
/// Whether a watchdog has been launched for this process.
static ARMED: AtomicBool = AtomicBool::new(false);

/// The parent pid this process was launched to watch, from the dedicated
/// watchdog's single argument. Pure, so the watchdog's `main` parses its
/// arguments before doing anything.
pub fn watchdog_pid(args: &[String]) -> Option<u32> {
    args.first()?.parse().ok()
}

/// Whether `args` ask this process to run as the takeover watchdog.
pub fn is_watchdog(args: &[String]) -> bool {
    watchdog_pid(args).is_some()
}

/// Launches the watchdog before Explorer's icons are hidden. Idempotent: the
/// first successful call arms it, and later calls are no-ops. Best-effort, so
/// a failure is logged and the takeover continues without crash safety.
pub fn arm() {
    if ARMED.swap(true, Ordering::SeqCst) {
        return;
    }
    let pid = std::process::id();
    let name = wide(&clean_exit_name(pid));
    // SAFETY: `name` is NUL-terminated; a manual-reset event that starts
    // unsignaled is what `signal_clean_exit` sets.
    let event = unsafe { CreateEventW(None, true, false, PCWSTR(name.as_ptr())) };
    let Ok(event) = event else {
        tracing::warn!("could not create the takeover clean-exit event");
        ARMED.store(false, Ordering::SeqCst);
        return;
    };
    CLEAN_EXIT.store(event.0 as isize, Ordering::SeqCst);
    match launch(pid) {
        Ok(()) => tracing::debug!(pid, "takeover watchdog launched"),
        Err(error) => {
            tracing::warn!(%error, "could not launch the takeover watchdog");
            CLEAN_EXIT.store(0, Ordering::SeqCst);
            // SAFETY: `event` is a live handle created above and owned here.
            let _ = unsafe { CloseHandle(event) };
            ARMED.store(false, Ordering::SeqCst);
        }
    }
}

/// Signals the watchdog that the process is exiting cleanly, with the icons
/// already restored. A no-op when no watchdog is armed.
pub fn signal_clean_exit() {
    let handle = CLEAN_EXIT.swap(0, Ordering::SeqCst);
    if handle == 0 {
        return;
    }
    // SAFETY: `handle` is the live event created in `arm`, owned here until it
    // is closed.
    unsafe {
        let _ = SetEvent(HANDLE(handle as *mut c_void));
        let _ = CloseHandle(HANDLE(handle as *mut c_void));
    }
}

/// Whether the takeover watchdog for this process is alive: it holds its
/// per-parent mutex (`mutex_name`) for as long as it runs. `Some(false)`
/// when it was never armed or has exited; `None` when the mutex cannot be
/// checked.
/// https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-openmutexw
pub fn is_running() -> Option<bool> {
    if !ARMED.load(Ordering::SeqCst) {
        return Some(false);
    }
    let name = wide(&mutex_name(std::process::id()));
    // SAFETY: `name` is NUL-terminated; an opened handle is closed at once.
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                Some(true)
            }
            Err(error) if error.code() == ERROR_FILE_NOT_FOUND.to_hresult() => Some(false),
            Err(_) => None,
        }
    }
}

/// Runs the watchdog for `pid` until that process exits, then re-shows
/// Explorer's icons unless it signaled the clean-exit event.
///
/// Re-parents itself once: the process the app launches spawns a detached copy
/// and exits, so the copy's recorded parent is gone and ending the app's
/// process tree (`taskkill /T`) cannot reach it. If the copy cannot be
/// launched, this process waits directly instead.
pub fn run_watchdog(pid: u32) {
    if std::env::var_os(REPARENT_ENV).is_none() {
        match respawn_reparented(pid) {
            Ok(()) => return,
            Err(error) => {
                tracing::debug!(%error, "watchdog: could not re-parent; waiting directly");
            }
        }
    }
    run_watchdog_loop(pid);
}

/// Spawns a detached copy of this watchdog that re-parents away from the app.
fn respawn_reparented(pid: u32) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .arg(pid.to_string())
        .env(REPARENT_ENV, "1")
        .creation_flags((DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB).0)
        .spawn()?;
    Ok(())
}

fn run_watchdog_loop(pid: u32) {
    if !claim_single_instance(pid) {
        return;
    }
    // Open the parent before waiting, so the wait is valid even if the parent
    // exits before this runs.
    // SAFETY: plain process query; `process` is closed below.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) };
    let Ok(process) = process else {
        // The parent is gone and cannot have signaled a clean exit.
        tracing::debug!(pid, "watchdog: parent not found; restoring the icons");
        desktop_icons::restore();
        return;
    };
    let name = wide(&clean_exit_name(pid));
    // SAFETY: `name` is NUL-terminated. Opening the event before the parent
    // exits keeps the object alive if the parent's handle closes first.
    let event = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name.as_ptr())) };
    // SAFETY: `process` is a live handle from `OpenProcess`.
    unsafe {
        let _ = WaitForSingleObject(process, INFINITE);
    }
    let clean_exit = event.is_ok_and(|event| {
        // SAFETY: `event` is a live handle; a zero timeout polls its state.
        let signaled = unsafe { WaitForSingleObject(event, 0) } == WAIT_OBJECT_0;
        // SAFETY: `event` was opened above and is owned here.
        let _ = unsafe { CloseHandle(event) };
        signaled
    });
    if should_restore(clean_exit) {
        tracing::info!(
            pid,
            "watchdog: DeskSpace crashed; restoring the desktop icons"
        );
        desktop_icons::restore();
    } else {
        tracing::debug!(pid, "watchdog: clean exit");
    }
    // SAFETY: `process` is a live handle from `OpenProcess`.
    let _ = unsafe { CloseHandle(process) };
}

/// Whether the watchdog must re-show the icons. It restores only when the
/// clean-exit event was absent or unsignaled. Pure.
fn should_restore(clean_exit_signaled: bool) -> bool {
    !clean_exit_signaled
}

/// The `Local\` event the main process signals on a clean exit. The pid keeps
/// two instances from sharing one.
fn clean_exit_name(pid: u32) -> String {
    format!("Local\\DeskSpace.WatchdogCleanExit.{pid}")
}

/// The `Local\` mutex that makes a second watchdog for the same parent a no-op.
fn mutex_name(pid: u32) -> String {
    format!("Local\\DeskSpace.Watchdog.{pid}")
}

/// Claims the per-parent mutex; false when another watchdog already holds it,
/// so two watchdogs for one parent never fight. The handle is kept open for the
/// rest of the process (`HANDLE` has no `Drop`), which keeps the name taken.
fn claim_single_instance(pid: u32) -> bool {
    let name = wide(&mutex_name(pid));
    // SAFETY: `name` is NUL-terminated. Last error is cleared first, because
    // `CreateMutexW` reports an existing mutex only through it.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        match CreateMutexW(None, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(handle);
                    return false;
                }
                true
            }
            Err(_) => false,
        }
    }
}

/// The watchdog executable beside the running one: `deskspace-watchdog.exe`
/// next to `deskspace.exe`. Derived from the current executable's name so a
/// renamed build still finds its watchdog.
fn watchdog_exe() -> std::io::Result<std::path::PathBuf> {
    let current = std::env::current_exe()?;
    let directory = current.parent().unwrap_or(std::path::Path::new("."));
    let stem = current
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("deskspace");
    Ok(directory.join(format!("{stem}-watchdog{}", std::env::consts::EXE_SUFFIX)))
}

/// Spawns the watchdog detached, so closing DeskSpace's console or ending it
/// by image name leaves the watchdog running.
fn launch(pid: u32) -> std::io::Result<()> {
    let exe = watchdog_exe()?;
    Command::new(exe)
        .arg(pid.to_string())
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn watchdog_pid_reads_the_single_argument() {
        assert_eq!(watchdog_pid(&args(&["4321"])), Some(4321));
        assert!(is_watchdog(&args(&["1"])));
    }

    #[test]
    fn watchdog_pid_rejects_missing_or_bad_pids() {
        assert_eq!(watchdog_pid(&args(&[])), None);
        assert_eq!(watchdog_pid(&args(&["abc"])), None);
        assert_eq!(watchdog_pid(&args(&["--watchdog"])), None);
        assert!(!is_watchdog(&args(&["abc"])));
    }

    #[test]
    fn should_restore_only_without_a_clean_exit() {
        assert!(should_restore(false));
        assert!(!should_restore(true));
    }

    #[test]
    fn object_names_are_distinct_and_per_parent() {
        assert_ne!(clean_exit_name(1), clean_exit_name(2));
        assert_ne!(clean_exit_name(1), mutex_name(1));
        assert!(clean_exit_name(1).starts_with("Local\\"));
    }
}
