//! Plays a `Transition` on a space window (ROADMAP D12).
//!
//! The window keeps its running animation in its state, and gets a frame
//! after every composition while it runs (`frame_clock`); each frame takes
//! the transition's frame for now, moves the window to the frame's
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
    DestroyWindow, IsWindowVisible, SW_HIDE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
};

use crate::render::{Geometry, Mask, Play, PreparedScene, Transition, TransitionFrame, Way};
use crate::win32::debug_info::transparency_enabled;
use crate::win32::flame_overlay::FlameOverlay;
use crate::win32::frame_clock;
use crate::win32::runtime;
use crate::win32::space_window::{
    Backing, SpaceState, current_rect, show, show_backing, with_state,
};
use crate::win32::system::work_area_at;

/// What an animation is for, which decides how it ends and what a new rect
/// means while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    /// Hiding or showing: an Out ends hidden.
    Visibility,
    /// Folding to the title bar or unfolding: the window stays either way.
    Fold,
    /// A deleted space's window going away: it destroys itself at the end.
    Dying,
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
    /// Burn's flames, on their own window above the space.
    flames: Option<FlameOverlay>,
    /// What plays once a hide ends, bringing the window back: the debug
    /// window's preview (`preview`).
    then: Option<Play>,
    /// Windows draws the material flat ("Transparency effects" off), so it
    /// steps aside while a frame leaves the window see-through: a flat
    /// material stayed as a dark box under a fading space (seen 2026-10-18).
    /// A real blur stays: a composition space's follows the frame (T13), and
    /// the accent blur steps at the end.
    flat_material: bool,
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
    // A real hide or show (the tray, the hotkey, exit) cancels a preview's
    // way back, so a hidden space stays hidden.
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            if let Some(animation) = state.animation.as_mut() {
                animation.then = None;
            }
        });
    }
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
            if visible {
                show(window);
            } else {
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
        return;
    }
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), play.edge);
    // SAFETY: the caller's contract.
    unsafe { start(window, Play { way, ..play }, geometry, Purpose::Visibility) };
}

/// Plays `out` on a shown `window` and then `back`, leaving it shown where
/// it was: the debug window's preview of an effect. Pressed again mid-way,
/// it turns around like any hide or show. It never touches the model.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn preview(window: HWND, out: Play, back: Play) {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let heading = unsafe {
        with_state(window, |state| {
            state
                .animation
                .as_ref()
                .filter(|animation| animation.purpose == Purpose::Visibility)
                .map(|animation| animation.transition.way())
        })
    }
    .flatten();
    match heading {
        // Leaving: come straight back instead, and stay.
        Some(Way::Out) => {
            // SAFETY: the caller's contract.
            unsafe {
                with_state(window, |state| {
                    if let Some(animation) = state.animation.as_mut() {
                        animation.then = None;
                    }
                });
                turn_around(window, Purpose::Visibility, Way::In);
            }
        }
        // Coming back: leave again, and come back once more after.
        Some(Way::In) => {
            // SAFETY: the caller's contract.
            unsafe {
                turn_around(window, Purpose::Visibility, Way::Out);
                with_state(window, |state| {
                    if let Some(animation) = state.animation.as_mut() {
                        animation.then = Some(back);
                    }
                });
            }
        }
        None => {
            // SAFETY: a plain visibility query on a live window.
            if is_instant(out) || !unsafe { IsWindowVisible(window) }.as_bool() {
                return;
            }
            // SAFETY: the caller's contract.
            unsafe {
                set_visible(window, false, out);
                with_state(window, |state| {
                    if let Some(animation) = state.animation.as_mut() {
                        animation.then = Some(back);
                    }
                });
            }
        }
    }
}

/// Folds `window` to `rect` (its title bar) or unfolds it to `rect` with
/// `play`, whose way says which. A fold already running the other way turns
/// around.
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
    unsafe { start(window, play, geometry, Purpose::Fold) };
}

/// Plays `play` on a deleted space's `window`, then destroys it. A space
/// still arriving turns around from where it is; a fold ends first. A
/// hidden window, or an instant play, is destroyed at once.
///
/// # Safety
/// `window` is a live space window the runtime no longer lists as a space;
/// call it outside any state borrow.
pub(crate) unsafe fn dismiss(window: HWND, play: Play) {
    let now = now();
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let arriving = unsafe {
        with_state(window, |state| {
            let animation = state
                .animation
                .as_mut()
                .filter(|animation| animation.purpose != Purpose::Fold)?;
            if animation.transition.way() == Way::In {
                animation.transition.reverse(now);
            }
            animation.purpose = Purpose::Dying;
            Some(())
        })
    };
    if arriving.flatten().is_some() {
        return;
    }
    // SAFETY: the caller's contract.
    unsafe { finish(window) };
    // SAFETY: a plain visibility query on a live window.
    let shown = unsafe { IsWindowVisible(window) }.as_bool();
    let rect = current_rect(window).filter(|_| shown && !is_instant(play));
    let Some(rect) = rect else {
        // SAFETY: the caller's contract.
        unsafe { destroy(window) };
        return;
    };
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), play.edge);
    let play = Play {
        way: Way::Out,
        ..play
    };
    // SAFETY: the caller's contract.
    unsafe { start(window, play, geometry, Purpose::Dying) };
}

/// Destroys a dying window and drops it from the runtime's list.
unsafe fn destroy(window: HWND) {
    runtime::forget_dying(window);
    // SAFETY: the caller's contract: a window of this thread no state borrow
    // holds. It frees its own state in WM_NCDESTROY.
    unsafe {
        let _ = DestroyWindow(window);
    }
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
    let flames = (play.effect == Effect::Burn)
        .then(|| FlameOverlay::create(window, geometry.rect, play.detail))
        .flatten();
    let flat_material = transparency_enabled() == Some(false);
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            state.animation = Some(Animation {
                transition,
                play,
                purpose,
                frame,
                deferred: None,
                flames,
                then: None,
                flat_material,
            });
        });
        frame_clock::subscribe(window);
        if !waiting {
            apply(window, frame, flat_material);
            draw_flames(window, frame, now);
        }
    }
}

/// Draws Burn's flames for `frame`, if this animation has them. The overlay
/// is taken out of the state while it draws, so no borrow is held across
/// the Win32 calls.
unsafe fn draw_flames(window: HWND, frame: TransitionFrame, now: Duration) {
    let Mask::Burn { front, .. } = frame.mask else {
        return;
    };
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let flames = unsafe {
        with_state(window, |state| {
            state
                .animation
                .as_mut()
                .and_then(|animation| animation.flames.take())
        })
    };
    let Some(Some(mut flames)) = flames else {
        return;
    };
    flames.draw(front, now.as_secs_f32());
    // SAFETY: as above.
    unsafe {
        with_state(window, |state| {
            if let Some(animation) = state.animation.as_mut() {
                animation.flames = Some(flames);
            }
        });
    }
}

/// Advances `window`'s animation to now, and ends it once done.
///
/// # Safety
/// `window` is a live space window, handling `messages::FRAME`.
pub(crate) unsafe fn on_frame(window: HWND) {
    frame_clock::delivered(window);
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
                animation.flat_material,
            ))
        })
    };
    let Some(Some((frame, done, waiting, flat_material))) = step else {
        frame_clock::unsubscribe(window);
        return;
    };
    // SAFETY: the caller's contract.
    unsafe {
        if done {
            finish(window);
        } else if !waiting {
            apply(window, frame, flat_material);
            draw_flames(window, frame, now);
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
            let gone = animation.transition.way() == Way::Out;
            let rect = animation.end_rect();
            if let Some(scene) = animation.deferred.take() {
                state.scene = Some(scene);
            }
            let then = animation.then.filter(|_| gone);
            Some((rect, animation.purpose, gone, animation.flames.take(), then))
        })
    };
    frame_clock::unsubscribe(window);
    // SAFETY: the caller's contract.
    unsafe {
        let Some(Some((rect, purpose, gone, flames, then))) = ended else {
            return;
        };
        // The flames' window goes outside any state borrow.
        drop(flames);
        if purpose == Purpose::Dying {
            destroy(window);
            return;
        }
        let hide = purpose == Purpose::Visibility && gone;
        move_to(window, rect);
        if hide {
            let _ = ShowWindow(window, SW_HIDE);
        } else if !IsWindowVisible(window).as_bool() {
            show(window);
        }
        // After the hide, so nothing stepped aside shows around a gone space.
        show_backing(window, Backing::WHOLE);
        let _ = InvalidateRect(Some(window), None, false);
        if let Some(back) = then {
            set_visible(window, true, back);
            return;
        }
    }
    // An exit waits for the last space to leave (ROADMAP D13).
    runtime::space_left();
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
                Purpose::Visibility | Purpose::Dying => {
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

/// Whether `window` is a deleted space's, playing its way out, so its
/// input no longer reaches the app.
pub(crate) fn is_dying(window: HWND) -> bool {
    // SAFETY: the borrow does not re-enter.
    unsafe {
        with_state(window, |state: &mut SpaceState| {
            state
                .animation
                .as_ref()
                .is_some_and(|animation| animation.purpose == Purpose::Dying)
        })
    }
    .unwrap_or(false)
}

/// Whether `window` is on its way out (hiding, folding away its items, or
/// deleted), so it takes no drops.
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
/// and repaints it through the frame. While the frame leaves the window
/// see-through, a `flat_material` steps aside.
unsafe fn apply(window: HWND, frame: TransitionFrame, flat_material: bool) {
    // SAFETY: a live window owned by this thread; SetWindowPos sends only
    // WM_WINDOWPOSCHANGING and WM_SIZE, which do not reach the app.
    unsafe {
        if frame.rect.is_empty() {
            let _ = ShowWindow(window, SW_HIDE);
            return;
        }
        move_to(window, frame.rect);
        show_backing(
            window,
            Backing {
                material: frame.covers_window() || !flat_material,
            },
        );
        if !IsWindowVisible(window).as_bool() {
            show(window);
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
