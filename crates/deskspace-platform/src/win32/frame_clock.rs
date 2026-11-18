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
//!
//! Frames go out in rounds: a new round only once every window has finished
//! the last. A composition space's drawing reaches the screen only when the
//! compositor commits, and it commits only when the UI thread's queue runs
//! dry. Each window used to get its next frame as soon as it began handling
//! the last, so when a round took longer than a refresh (several spaces
//! burning at once) a frame always waited in the queue: the spaces froze
//! mid-burn while their flames, on layered windows that need no commit, rose
//! on (seen 2026-11-18). Waiting for the whole round leaves the queue empty
//! until the next composition, every round.

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

/// Whether any window is playing an animation now. The frame thread shares
/// this lock, so it is taken for the read alone and released at once.
pub(crate) fn any_animating() -> bool {
    !clock().windows.is_empty()
}

/// A window's frames: it gets one after every composition while any
/// subscription to it lives. A window may hold several, so one reason to
/// animate ending does not stop another; the frames stop when the last one
/// is dropped, however it ends, the window's destruction included.
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

/// Handles `window`'s frame with `draw`, and only then marks it handled, so
/// the next round waits for the work and not just its start.
pub(crate) fn handle(window: HWND, draw: impl FnOnce()) {
    draw();
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
                clock().windows.undeliverable(key);
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

/// The animating windows, each with how many subscriptions it holds and
/// whether a frame sent to it is still unhandled. A window gets one frame per
/// round, not a backlog of them, however many subscriptions it has, and no
/// round starts while any frame of the last is unhandled.
#[derive(Debug, Default)]
struct Subscribers {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    window: WindowKey,
    subscriptions: u32,
    waiting: bool,
}

impl Subscribers {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, window: WindowKey) {
        if let Some(entry) = self.entry(window) {
            entry.subscriptions += 1;
        } else {
            self.entries.push(Entry {
                window,
                subscriptions: 1,
                waiting: false,
            });
        }
    }

    /// Drops one subscription. The window leaves at its last one.
    fn remove(&mut self, window: WindowKey) {
        let Some(index) = self.index(window) else {
            return;
        };
        let entry = &mut self.entries[index];
        entry.subscriptions -= 1;
        if entry.subscriptions == 0 {
            self.entries.remove(index);
        }
    }

    fn delivered(&mut self, window: WindowKey) {
        if let Some(entry) = self.entry(window) {
            entry.waiting = false;
        }
    }

    /// A frame could not be posted to `window`, which is gone: it no longer
    /// holds up the round, and drops a subscription.
    fn undeliverable(&mut self, window: WindowKey) {
        self.delivered(window);
        self.remove(window);
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Fills `due` with the next round, every window, and marks them
    /// waiting; or leaves it empty while any window has not finished the
    /// last round.
    fn take_due(&mut self, due: &mut Vec<WindowKey>) {
        due.clear();
        if self.entries.iter().any(|entry| entry.waiting) {
            return;
        }
        for entry in &mut self.entries {
            entry.waiting = true;
            due.push(entry.window);
        }
    }

    fn index(&self, window: WindowKey) -> Option<usize> {
        self.entries.iter().position(|entry| entry.window == window)
    }

    fn entry(&mut self, window: WindowKey) -> Option<&mut Entry> {
        let index = self.index(window)?;
        self.entries.get_mut(index)
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
    fn a_round_waits_for_every_window_to_finish_the_last() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        windows.add(TWO);
        assert_eq!(due(&mut windows), [ONE, TWO]);
        windows.delivered(TWO);
        assert!(
            due(&mut windows).is_empty(),
            "1 has not handled its frame yet, so the queue may not run dry"
        );
        windows.delivered(ONE);
        assert_eq!(due(&mut windows), [ONE, TWO]);
    }

    #[test]
    fn a_window_joining_mid_round_waits_for_the_next() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        assert_eq!(due(&mut windows), [ONE]);
        windows.add(TWO);
        assert!(due(&mut windows).is_empty());
        windows.delivered(ONE);
        assert_eq!(due(&mut windows), [ONE, TWO]);
    }

    #[test]
    fn a_window_that_cannot_be_posted_to_does_not_hold_up_the_round() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        windows.add(ONE);
        windows.add(TWO);
        assert_eq!(due(&mut windows), [ONE, TWO]);
        windows.undeliverable(ONE);
        windows.delivered(TWO);
        assert_eq!(due(&mut windows), [ONE, TWO], "1 kept a subscription");
    }

    #[test]
    fn a_window_stays_while_any_subscription_lives() {
        let mut windows = Subscribers::new();
        windows.add(ONE);
        windows.add(ONE);
        assert_eq!(due(&mut windows), [ONE]);
        windows.remove(ONE);
        assert!(!windows.is_empty(), "one subscription is left");
        windows.delivered(ONE);
        assert_eq!(due(&mut windows), [ONE], "it is listed once per frame");
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
