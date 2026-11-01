//! Turns console Ctrl+C, Ctrl+Break and console-window close into an orderly
//! `WM_CLOSE`, so the app gets `CloseRequested` and can save before exiting.
//!
//! Console control handlers run on a thread the system creates. For
//! `CTRL_CLOSE_EVENT` the process is terminated as soon as the handler
//! returns (or after a ~5 s timeout), so the handler waits for the message loop
//! to finish before returning.
//! https://learn.microsoft.com/en-us/windows/console/handlerroutine

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
use windows::core::BOOL;

/// Stays under the system's ~5 s `CTRL_CLOSE_EVENT` deadline.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(4500);

// Process state: the controller window the console handler posts `WM_CLOSE`
// to. The handler runs on a system thread with no context, so it cannot be
// passed one. `install` sets it and `message_loop_finished` clears it.
static TARGET: AtomicIsize = AtomicIsize::new(0);
// Process state: whether the message loop has ended, for the handler's bounded
// wait. The handler gets no context to wait on otherwise. Set by
// `message_loop_finished`; `install` resets it at startup.
static LOOP_FINISHED: Mutex<bool> = Mutex::new(false);
// Process state: wakes the handler's wait when `LOOP_FINISHED` changes. A bare
// mutex cannot wake a handler that is not polled; only `notify_all` does.
static LOOP_FINISHED_CHANGED: Condvar = Condvar::new();

pub(crate) fn install(window: HWND) {
    TARGET.store(window.0 as isize, Ordering::SeqCst);
    *LOOP_FINISHED.lock().unwrap_or_else(PoisonError::into_inner) = false;
    // SAFETY: `on_console_control` matches PHANDLER_ROUTINE and only touches
    // thread-safe statics and PostMessageW, which may be called from any thread.
    if let Err(error) = unsafe { SetConsoleCtrlHandler(Some(on_console_control), true) } {
        tracing::debug!(%error, "console control handler not installed");
    }
}

pub(crate) fn message_loop_finished() {
    TARGET.store(0, Ordering::SeqCst);
    *LOOP_FINISHED.lock().unwrap_or_else(PoisonError::into_inner) = true;
    LOOP_FINISHED_CHANGED.notify_all();
}

unsafe extern "system" fn on_console_control(control: u32) -> BOOL {
    if !matches!(control, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT) {
        return false.into();
    }
    let target = TARGET.load(Ordering::SeqCst);
    if target == 0 {
        return false.into();
    }
    // SAFETY: PostMessageW is thread-safe; a stale HWND only makes it fail.
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut core::ffi::c_void)),
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0),
        )
    };
    if posted.is_err() {
        return false.into();
    }
    let finished = LOOP_FINISHED.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = LOOP_FINISHED_CHANGED
        .wait_timeout_while(finished, SHUTDOWN_GRACE, |finished| !*finished)
        .unwrap_or_else(PoisonError::into_inner);
    true.into()
}
