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

// Process state: the animating windows and whether the frame thread runs. The
// thread reads it after every composition and the UI thread on every change;
// it lives for the process.
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

/// A window's frames: it gets one after every composition while this
/// lives. An animation holds it, so its frames stop when it ends, however
/// it ends, the window's destruction included.
#[derive(Debug)]
pub(crate) struct Subscription(WindowKey);

impl Subscription {
    pub(crate) fn new(window: HWND) -> Self {
        let key = WindowKey::of(window);
        let mut clock = clock();
        clock.windows.add(key);
        if !clock.running {
            clock.running = true;
            if let Err(error) = thread::Builder::new().name("frame clock".into()).spawn(run) {
                clock.running = false;
                tracing::warn!(%error, "the frame clock did not start; animations will not move");
            }
        }
        Self(key)
    }
}

impl Drop for Subscription {
    /// The thread ends once no window is left.
    fn drop(&mut self) {
        clock().windows.remove(self.0);
    }
}

/// Marks `window`'s frame as handled, so the next composition sends another.
pub(crate) fn delivered(window: HWND) {
    clock().windows.delivered(WindowKey::of(window));
}

/// A window's handle as a plain value, which the clock's thread can hold:
/// `HWND` is a pointer, so it is not `Send`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowKey(isize);

impl WindowKey {
    fn of(window: HWND) -> Self {
        Self(window.0 as isize)
    }

    fn window(self) -> HWND {
        HWND(self.0 as *mut core::ffi::c_void)
    }
}

fn run() {
    let mut due = Vec::new();
    loop {
        wait_for_composition();
        if !clock().after_composition(&mut due) {
            return;
        }
        for &key in &due {
            // SAFETY: posting copies two plain values into the window's
            // queue; a window that is gone makes it fail, nothing more.
            let posted =
                unsafe { PostMessageW(Some(key.window()), messages::FRAME, WPARAM(0), LPARAM(0)) };
            if posted.is_err() {
                clock().windows.remove(key);
            }
        }
    }
}

impl Clock {
    /// What the thread does after a composition: fills `due` with the
    /// windows owed a frame and goes on, or, with no window left, stops.
    fn after_composition(&mut self, due: &mut Vec<WindowKey>) -> bool {
        if self.windows.is_empty() {
            self.running = false;
            return false;
        }
        self.windows.take_due(due);
        true
    }
}

fn wait_for_composition() {
    let start = Instant::now();
    // SAFETY: takes no arguments; it only blocks this thread.
    let waited = unsafe { DwmFlush() }.is_ok();
    if let Some(rest) = rest_after(waited, start.elapsed()) {
        thread::sleep(rest);
    }
}

/// How much longer to wait after `DwmFlush` returned (`waited`) after
/// `elapsed`: nothing when it waited for a composition, else the rest of a
/// refresh, so the thread never spins.
fn rest_after(waited: bool, elapsed: Duration) -> Option<Duration> {
    (!waited || elapsed < SHORTEST_WAIT).then(|| FALLBACK_REFRESH.saturating_sub(elapsed))
}

/// The animating windows, each with whether a frame sent to it is still
/// waiting in its queue. A window whose UI thread is busy gets one frame,
/// not a backlog of them.
#[derive(Debug, Default)]
struct Subscribers {
    entries: Vec<(WindowKey, bool)>,
}

impl Subscribers {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, window: WindowKey) {
        if !self.entries.iter().any(|(each, _)| *each == window) {
            self.entries.push((window, false));
        }
    }

    fn remove(&mut self, window: WindowKey) {
        self.entries.retain(|(each, _)| *each != window);
    }

    fn delivered(&mut self, window: WindowKey) {
        if let Some(entry) = self.entries.iter_mut().find(|(each, _)| *each == window) {
            entry.1 = false;
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Fills `due` with the windows owed a frame, and marks them waiting.
    fn take_due(&mut self, due: &mut Vec<WindowKey>) {
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

    const ONE: WindowKey = WindowKey(1);
    const TWO: WindowKey = WindowKey(2);

    fn due(windows: &mut Subscribers) -> Vec<WindowKey> {
        let mut due = Vec::new();
        windows.take_due(&mut due);
        due
    }

    #[test]
    fn a_busy_window_gets_one_frame_not_a_backlog() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        windows.add(TWO);
        assert_eq!(due(&mut windows), [ONE, TWO]);
        windows.delivered(TWO);
        assert_eq!(due(&mut windows), [TWO], "1 has not handled its frame yet");
        windows.delivered(ONE);
        windows.delivered(TWO);
        assert_eq!(due(&mut windows), [ONE, TWO]);
    }

    #[test]
    fn a_window_subscribes_once_and_leaves_for_good() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        windows.add(ONE);
        assert_eq!(due(&mut windows), [ONE]);
        windows.remove(ONE);
        assert!(windows.is_empty());
        windows.delivered(ONE);
        assert!(due(&mut windows).is_empty());
    }

    #[test]
    fn the_thread_stops_once_no_window_is_left() {
        let mut clock = Clock {
            windows: Subscribers::new(),
            running: true,
        };
        let mut due = Vec::new();
        clock.windows.add(ONE);
        assert!(clock.after_composition(&mut due));
        assert_eq!(due, [ONE]);
        clock.windows.remove(ONE);
        assert!(!clock.after_composition(&mut due));
        assert!(!clock.running, "a new subscription starts a new thread");
    }

    #[test]
    fn the_thread_rests_out_a_refresh_only_when_dwm_did_not_wait() {
        let waited = Duration::from_millis(7);
        assert_eq!(rest_after(true, waited), None);
        let early = Duration::from_millis(1);
        assert_eq!(rest_after(true, early), Some(FALLBACK_REFRESH - early));
        assert_eq!(rest_after(false, early), Some(FALLBACK_REFRESH - early));
        assert_eq!(
            rest_after(false, Duration::from_millis(40)),
            Some(Duration::ZERO)
        );
    }
}
