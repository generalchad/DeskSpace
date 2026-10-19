//! The clock space animations run on: one `messages::FRAME` per display
//! refresh to every animating window (ROADMAP D12).
//!
//! Frames used to come from a 15 ms `WM_TIMER`, which Windows rounds to its
//! 15.6 ms tick. That drifts against a 60 Hz refresh, so now and then two
//! frames landed in one refresh and one was never seen, or none landed and
//! one showed twice: every animation stuttered, Roll most of all (seen
//! 2026-10-19). A thread now waits on DWM's composition (`DwmFlush`) while
//! any window animates, and posts each one a frame right after it, so every
//! refresh gets exactly one.
//! https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::win32::messages;

/// How long a refresh takes when DWM cannot say: `DwmFlush` returns at once
/// when it has nothing to compose (no window changed) or fails (a locked
/// session), and the thread then waits this long instead of spinning.
const FALLBACK_REFRESH: Duration = Duration::from_millis(16);
/// A wait shorter than this did not wait for a composition.
const SHORTEST_WAIT: Duration = Duration::from_millis(4);

/// Process state: the animating windows and whether the thread runs. The
/// thread reads it after every composition, the UI thread on every change.
static CLOCK: Mutex<Clock> = Mutex::new(Clock {
    windows: Subscribers::new(),
    running: false,
});

struct Clock {
    windows: Subscribers,
    running: bool,
}

fn clock() -> MutexGuard<'static, Clock> {
    // The state stays consistent at every unlock, so a panic elsewhere while
    // it was held leaves nothing half done.
    CLOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sends `window` a frame after every composition until `unsubscribe`.
pub(crate) fn subscribe(window: HWND) {
    let mut clock = clock();
    clock.windows.add(window.0 as isize);
    if !clock.running {
        clock.running = true;
        if let Err(error) = thread::Builder::new().name("frame clock".into()).spawn(run) {
            clock.running = false;
            tracing::warn!(%error, "the frame clock did not start; animations will not move");
        }
    }
}

/// Stops `window`'s frames. The thread ends once no window is left.
pub(crate) fn unsubscribe(window: HWND) {
    clock().windows.remove(window.0 as isize);
}

/// Marks `window`'s frame as handled, so the next composition sends another.
pub(crate) fn delivered(window: HWND) {
    clock().windows.delivered(window.0 as isize);
}

fn run() {
    let mut due = Vec::new();
    loop {
        wait_for_composition();
        {
            let mut clock = clock();
            if clock.windows.is_empty() {
                clock.running = false;
                return;
            }
            clock.windows.take_due(&mut due);
        }
        for &window in &due {
            // SAFETY: posting copies two plain values into the window's
            // queue; a window that is gone makes it fail, nothing more.
            let posted = unsafe {
                PostMessageW(
                    Some(HWND(window as *mut core::ffi::c_void)),
                    messages::FRAME,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
            if posted.is_err() {
                clock().windows.remove(window);
            }
        }
    }
}

fn wait_for_composition() {
    let start = Instant::now();
    // SAFETY: takes no arguments; it only blocks this thread.
    let waited = unsafe { DwmFlush() }.is_ok();
    let elapsed = start.elapsed();
    if !waited || elapsed < SHORTEST_WAIT {
        thread::sleep(FALLBACK_REFRESH.saturating_sub(elapsed));
    }
}

/// The animating windows, each with whether a frame sent to it is still
/// waiting in its queue. A window whose UI thread is busy gets one frame,
/// not a backlog of them.
#[derive(Debug, Default)]
struct Subscribers {
    entries: Vec<(isize, bool)>,
}

impl Subscribers {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, window: isize) {
        if !self.entries.iter().any(|(each, _)| *each == window) {
            self.entries.push((window, false));
        }
    }

    fn remove(&mut self, window: isize) {
        self.entries.retain(|(each, _)| *each != window);
    }

    fn delivered(&mut self, window: isize) {
        if let Some(entry) = self.entries.iter_mut().find(|(each, _)| *each == window) {
            entry.1 = false;
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Fills `due` with the windows owed a frame, and marks them waiting.
    fn take_due(&mut self, due: &mut Vec<isize>) {
        due.clear();
        for (window, waiting) in &mut self.entries {
            if !*waiting {
                *waiting = true;
                due.push(*window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn due(windows: &mut Subscribers) -> Vec<isize> {
        let mut due = Vec::new();
        windows.take_due(&mut due);
        due
    }

    #[test]
    fn a_busy_window_gets_one_frame_not_a_backlog() {
        let mut windows = Subscribers::new();
        windows.add(1);
        windows.add(2);
        assert_eq!(due(&mut windows), [1, 2]);
        windows.delivered(2);
        assert_eq!(due(&mut windows), [2], "1 has not handled its frame yet");
        windows.delivered(1);
        windows.delivered(2);
        assert_eq!(due(&mut windows), [1, 2]);
    }

    #[test]
    fn a_window_subscribes_once_and_leaves_for_good() {
        let mut windows = Subscribers::new();
        windows.add(1);
        windows.add(1);
        assert_eq!(due(&mut windows), [1]);
        windows.remove(1);
        assert!(windows.is_empty());
        windows.delivered(1);
        assert!(due(&mut windows).is_empty());
    }
}
