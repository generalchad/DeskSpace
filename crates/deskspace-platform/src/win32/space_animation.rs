//! Plays a `Transition` on a space window (ROADMAP D12).
//!
//! The window keeps its running animation in its state and a timer of its
//! own; each tick takes the frame for now, moves the window to the frame's
//! rect (hiding it while that is empty), and repaints through the frame.
//! Nothing calls into the app per frame. A request that finds an animation
//! heading the other way reverses it from where it is.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{InvalidateRect, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, KillTimer, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetTimer,
    SetWindowPos, ShowWindow,
};

use crate::render::{Geometry, Play, Transition, TransitionFrame, Way};
use crate::win32::space_window::{current_rect, with_state};
use crate::win32::system::work_area_at;

/// The id of a space window's own animation timer. It is set on the space
/// window, not the controller, so it never reaches the app's `Timer` ids.
pub(crate) const TIMER_ID: usize = 1;
/// About 60 frames a second; `SetTimer` rounds it to the system tick.
const FRAME_INTERVAL_MS: u32 = 15;

/// A space window's running animation.
pub(crate) struct Animation {
    transition: Transition,
    play: Play,
    /// The frame painted now.
    frame: TransitionFrame,
}

impl Animation {
    pub(crate) fn frame(&self) -> TransitionFrame {
        self.frame
    }
}

/// The time on the one clock every transition runs on.
fn now() -> Duration {
    // Process state: the clock's start, so `Transition` can take plain
    // durations; set on first use and never cleared.
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed()
}

/// Shows or hides `window` with `play`. An animation already running the
/// other way turns around from where it is; one already heading there
/// carries on.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn set_visible(window: HWND, visible: bool, play: Play) {
    let way = if visible { Way::In } else { Way::Out };
    let now = now();
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let running = unsafe {
        with_state(window, |state| {
            let animation = state.animation.as_mut()?;
            if animation.transition.way() != way {
                animation.transition.reverse(now);
            }
            Some(())
        })
    };
    if running.flatten().is_some() {
        return;
    }
    // SAFETY: a plain visibility query on a live window.
    let shown = unsafe { IsWindowVisible(window) }.as_bool();
    if shown == visible {
        return;
    }
    let Some(rect) = current_rect(window) else {
        return;
    };
    if play.duration.is_zero() || play.effect == deskspace_core::animation::Effect::None {
        // SAFETY: a live window owned by this thread.
        unsafe {
            let _ = ShowWindow(window, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        return;
    }
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), play.edge);
    let transition = Transition::new(play, geometry, now);
    let frame = transition.frame(now);
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            state.animation = Some(Animation {
                transition,
                play,
                frame,
            });
        });
        let _ = SetTimer(Some(window), TIMER_ID, FRAME_INTERVAL_MS, None);
        apply(window, frame);
    }
}

/// Advances `window`'s animation to now, and ends it once done.
///
/// # Safety
/// `window` is a live space window, in its `WM_TIMER`.
pub(crate) unsafe fn on_timer(window: HWND) {
    let now = now();
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let step = unsafe {
        with_state(window, |state| {
            let animation = state.animation.as_mut()?;
            animation.frame = animation.transition.frame(now);
            let done = animation.transition.is_done(now);
            Some((
                animation.frame,
                done,
                animation.transition.way(),
                animation.transition.geometry().rect,
            ))
        })
    };
    let Some(Some((frame, done, way, rect))) = step else {
        // SAFETY: killing this window's own timer.
        unsafe {
            let _ = KillTimer(Some(window), TIMER_ID);
        }
        return;
    };
    // SAFETY: the caller's contract.
    unsafe {
        if !done {
            apply(window, frame);
            return;
        }
        let _ = KillTimer(Some(window), TIMER_ID);
        with_state(window, |state| state.animation = None);
        // The window ends at its own rect, whichever way it went: shown, or
        // hidden at the place it will show again.
        move_to(window, rect);
        if way == Way::Out {
            let _ = ShowWindow(window, SW_HIDE);
        } else if !IsWindowVisible(window).as_bool() {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        let _ = InvalidateRect(Some(window), None, false);
    }
}

/// While `window` animates, makes `rect` where it ends up, keeping the
/// progress, and returns true; false when nothing is running.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn retarget(window: HWND, rect: Rect) -> bool {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            let Some(animation) = state.animation.as_mut() else {
                return false;
            };
            let geometry =
                Geometry::for_visibility(rect, work_area_at(rect.center()), animation.play.edge);
            animation.transition.set_geometry(geometry);
            true
        })
    }
    .unwrap_or(false)
}

/// Whether `window` is on its way out, so it takes no drops.
pub(crate) fn is_leaving(window: HWND) -> bool {
    // SAFETY: space windows and the desktop layer share this state; the
    // borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            state
                .animation
                .as_ref()
                .is_some_and(|animation| animation.transition.way() == Way::Out)
        })
    }
    .unwrap_or(false)
}

/// Puts `window` where `frame` says, hidden while the frame's rect is empty,
/// and repaints it through the frame.
unsafe fn apply(window: HWND, frame: TransitionFrame) {
    // SAFETY: a live window owned by this thread; SetWindowPos sends only
    // WM_WINDOWPOSCHANGING and WM_SIZE, which do not reach the app.
    unsafe {
        if frame.rect.is_empty() {
            let _ = ShowWindow(window, SW_HIDE);
            return;
        }
        move_to(window, frame.rect);
        if !IsWindowVisible(window).as_bool() {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        // Paint now rather than at the next idle moment, so the window never
        // shows the previous frame at the new size.
        let _ = RedrawWindow(Some(window), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
    }
}

unsafe fn move_to(window: HWND, rect: Rect) {
    // SAFETY: as in `apply`.
    unsafe {
        let _ = SetWindowPos(
            window,
            None,
            rect.x,
            rect.y,
            rect.width.max(1),
            rect.height.max(1),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
