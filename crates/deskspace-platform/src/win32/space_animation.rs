//! Plays a `Transition` on a space window (ROADMAP D12).
//!
//! The window keeps its running animation in its state and a timer of its
//! own; each tick takes the frame for now, moves the window to the frame's
//! rect (hiding it while that is empty), and repaints through the frame.
//! Nothing calls into the app per frame. A request that finds an animation
//! heading the other way reverses it from where it is.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use deskspace_core::animation::Effect;
use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{InvalidateRect, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, KillTimer, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetTimer,
    SetWindowPos, ShowWindow,
};

use crate::render::{Geometry, Play, PreparedScene, Transition, TransitionFrame, Way};
use crate::win32::space_window::{SpaceState, current_rect, with_state};
use crate::win32::system::work_area_at;

/// The id of a space window's own animation timer. It is set on the space
/// window, not the controller, so it never reaches the app's `Timer` ids.
pub(crate) const TIMER_ID: usize = 1;
/// About 60 frames a second; `SetTimer` rounds it to the system tick.
const FRAME_INTERVAL_MS: u32 = 15;

/// What an animation is for, which decides how it ends and what a new rect
/// means while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    /// Hiding or showing: an Out ends hidden.
    Visibility,
    /// Folding to the title bar or unfolding: the window stays either way.
    Fold,
}

/// A space window's running animation.
pub(crate) struct Animation {
    transition: Transition,
    play: Play,
    purpose: Purpose,
    /// The frame painted now.
    frame: TransitionFrame,
    /// A scene that arrived while folding. The folded scene has no items, so
    /// the window keeps drawing the unfolded one until the fold ends.
    deferred: Option<PreparedScene>,
}

impl Animation {
    pub(crate) fn frame(&self) -> TransitionFrame {
        self.frame
    }

    /// Keeps `scene` for the end of a fold, or hands it back to be shown now.
    pub(crate) fn defer(&mut self, scene: PreparedScene) -> Option<PreparedScene> {
        if self.purpose == Purpose::Fold && self.transition.way() == Way::Out {
            self.deferred = Some(scene);
            return None;
        }
        Some(scene)
    }

    /// The rect the window has once the animation ends, whichever way it is
    /// going now.
    fn end_rect(&self) -> Rect {
        let geometry = self.transition.geometry();
        match (self.purpose, self.transition.way()) {
            (Purpose::Fold, Way::Out) => geometry.rolled,
            _ => geometry.rect,
        }
    }
}

/// The time on the one clock every transition runs on.
fn now() -> Duration {
    // Process state: the clock's start, so `Transition` can take plain
    // durations; set on first use and never cleared.
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed()
}

fn is_instant(play: Play) -> bool {
    play.effect == Effect::None || play.duration.is_zero()
}

/// Shows or hides `window` with `play`. An animation already running the
/// other way turns around from where it is; one already heading there
/// carries on.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn set_visible(window: HWND, visible: bool, play: Play) {
    let way = if visible { Way::In } else { Way::Out };
    // SAFETY: the caller's contract.
    if unsafe { turn_around(window, Purpose::Visibility, way) } {
        return;
    }
    // A fold still running ends first, so the window hides at its folded or
    // unfolded rect and shows there again.
    // SAFETY: the caller's contract.
    unsafe { finish(window) };
    // SAFETY: a plain visibility query on a live window.
    let shown = unsafe { IsWindowVisible(window) }.as_bool();
    if shown == visible {
        return;
    }
    let Some(rect) = current_rect(window) else {
        return;
    };
    if is_instant(play) {
        // SAFETY: a live window owned by this thread.
        unsafe {
            let _ = ShowWindow(window, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        return;
    }
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), play.edge);
    // SAFETY: the caller's contract.
    unsafe { start(window, Play { way, ..play }, geometry, Purpose::Visibility) };
}

/// Folds `window` to `rect` (its title bar) or unfolds it to `rect` with
/// `play`, a Roll whose way says which. A fold already running the other way
/// turns around.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn fold(window: HWND, rect: Rect, play: Play) {
    // SAFETY: the caller's contract.
    if unsafe { turn_around(window, Purpose::Fold, play.way) } {
        return;
    }
    // A space folded while it hides or shows just ends up at the new rect.
    // SAFETY: the caller's contract.
    if unsafe { retarget(window, rect) } {
        return;
    }
    let Some(current) = current_rect(window) else {
        return;
    };
    if current == rect {
        return;
    }
    if is_instant(play) {
        // SAFETY: the caller's contract.
        unsafe {
            move_to(window, rect);
            let _ = InvalidateRect(Some(window), None, false);
        }
        return;
    }
    let work_area = work_area_at(rect.center());
    let geometry = match play.way {
        Way::Out => Geometry::for_fold(current, rect, work_area),
        Way::In => Geometry::for_fold(rect, current, work_area),
    };
    // SAFETY: the caller's contract.
    unsafe {
        start(
            window,
            Play {
                effect: Effect::Roll,
                ..play
            },
            geometry,
            Purpose::Fold,
        )
    };
}

/// Turns a running animation for `purpose` around if it is not already
/// heading `way`. Returns whether one was running.
unsafe fn turn_around(window: HWND, purpose: Purpose, way: Way) -> bool {
    let now = now();
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let running = unsafe {
        with_state(window, |state| {
            let animation = state
                .animation
                .as_mut()
                .filter(|animation| animation.purpose == purpose)?;
            if animation.transition.way() != way {
                animation.transition.reverse(now);
                // Unfolding again shows the newest scene at once.
                if let Some(scene) = animation.deferred.take() {
                    state.scene = Some(scene);
                }
            }
            Some(())
        })
    };
    running.flatten().is_some()
}

unsafe fn start(window: HWND, play: Play, geometry: Geometry, purpose: Purpose) {
    let now = now();
    let transition = Transition::new(play, geometry, now);
    let frame = transition.frame(now);
    // A window waiting its turn (startup's stagger) stays as it is, hidden,
    // so not even its blur shows early.
    let waiting = transition.is_waiting(now);
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            state.animation = Some(Animation {
                transition,
                play,
                purpose,
                frame,
                deferred: None,
            });
        });
        let _ = SetTimer(Some(window), TIMER_ID, FRAME_INTERVAL_MS, None);
        if !waiting {
            apply(window, frame);
        }
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
            Some((
                animation.frame,
                animation.transition.is_done(now),
                animation.transition.is_waiting(now),
            ))
        })
    };
    let Some(Some((frame, done, waiting))) = step else {
        // SAFETY: killing this window's own timer.
        unsafe {
            let _ = KillTimer(Some(window), TIMER_ID);
        }
        return;
    };
    // SAFETY: the caller's contract.
    unsafe {
        if done {
            finish(window);
        } else if !waiting {
            apply(window, frame);
        }
    }
}

/// Ends `window`'s animation where it was going: shown, folded, or unfolded
/// at its rect, or hidden at the rect it will show at again.
unsafe fn finish(window: HWND) {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let ended = unsafe {
        with_state(window, |state| {
            let mut animation = state.animation.take()?;
            let hide =
                animation.purpose == Purpose::Visibility && animation.transition.way() == Way::Out;
            let rect = animation.end_rect();
            if let Some(scene) = animation.deferred.take() {
                state.scene = Some(scene);
            }
            Some((rect, hide))
        })
    };
    // SAFETY: the caller's contract.
    unsafe {
        let _ = KillTimer(Some(window), TIMER_ID);
        let Some(Some((rect, hide))) = ended else {
            return;
        };
        move_to(window, rect);
        if hide {
            let _ = ShowWindow(window, SW_HIDE);
        } else if !IsWindowVisible(window).as_bool() {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        let _ = InvalidateRect(Some(window), None, false);
    }
}

/// What `space_window::set_rect` should do with `rect` while `window`
/// animates. Returns true when the animation took it: a hiding or showing
/// window ends up at `rect` instead, keeping its progress, and a fold
/// heading to `rect` carries on. Any other rect ends a fold where it is, so
/// the window moves at once.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn retarget(window: HWND, rect: Rect) -> bool {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let (taken, fold_ended) = unsafe {
        with_state(window, |state| {
            let Some(animation) = state.animation.as_mut() else {
                return (false, false);
            };
            match animation.purpose {
                Purpose::Visibility => {
                    let geometry = Geometry::for_visibility(
                        rect,
                        work_area_at(rect.center()),
                        animation.play.edge,
                    );
                    animation.transition.set_geometry(geometry);
                    (true, false)
                }
                Purpose::Fold if animation.end_rect() == rect => (true, false),
                Purpose::Fold => (false, true),
            }
        })
    }
    .unwrap_or((false, false));
    if fold_ended {
        // SAFETY: the caller's contract.
        unsafe { finish(window) };
    }
    taken
}

/// Whether `window` is on its way out (hiding, or folding away its items),
/// so it takes no drops.
pub(crate) fn is_leaving(window: HWND) -> bool {
    // SAFETY: space windows and the desktop layer share this state; the
    // borrow does not re-enter.
    unsafe {
        with_state(window, |state: &mut SpaceState| {
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
