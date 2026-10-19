//! Plays a `Transition` on a space window (ROADMAP D12).
//!
//! The window keeps its running animation in its state, and gets a frame
//! after every composition while it runs (`frame_clock`); each frame takes
//! the transition's frame for now, moves the window to the frame's
//! rect (hiding it while that is empty), and repaints through the frame.
//! Nothing calls into the app per frame. A request that finds an animation
//! heading the other way reverses it from where it is.
//!
//! `Playing` makes every decision and touches no window, so its tests cover
//! them; the functions below it only carry them out.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use deskspace_core::animation::Effect;
use deskspace_core::geometry::{Point, Rect};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{InvalidateRect, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, IsWindowVisible, SW_HIDE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
};

use crate::render::{Geometry, Mask, Play, PreparedScene, Transition, TransitionFrame, Way};
use crate::win32::flame_overlay::FlameOverlay;
use crate::win32::frame_clock;
use crate::win32::runtime;
use crate::win32::space_window::{Backing, current_rect, show, show_backing, with_state};
use crate::win32::system::transparency_enabled;
use crate::win32::system::work_area_at;

/// What an animation is for, which decides how it ends and what a new rect
/// means while it runs, with what only that purpose carries.
#[derive(Debug, PartialEq)]
enum Purpose<S> {
    /// Hiding or showing: an Out ends hidden. `then` plays once a hide
    /// ends, bringing the window back: the debug window's preview.
    Visibility { then: Option<Play> },
    /// Folding to the title bar or unfolding: the window stays either way.
    /// `deferred` is a scene that arrived while folding: the folded scene
    /// has no items, so the window keeps drawing the unfolded one until the
    /// fold ends.
    Fold { deferred: Option<S> },
    /// A deleted space's window going away: it destroys itself at the end.
    Dying,
}

/// A running animation's decisions: its transition, what it is for, and
/// the frame painted now. Generic over the scene a fold holds back, so the
/// tests need no real one.
#[derive(Debug)]
struct Playing<S> {
    transition: Transition,
    purpose: Purpose<S>,
    frame: TransitionFrame,
}

/// One frame of a running animation (`Playing::step`).
#[derive(Debug, Clone, Copy)]
struct Step {
    frame: TransitionFrame,
    done: bool,
    /// Still in its delay (startup's stagger), so the window stays as it
    /// is, hidden, and not even its blur shows early.
    waiting: bool,
}

/// What a new rect does to a running animation (`Playing::retarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Retarget {
    /// The animation took it: the window ends up there.
    Taken,
    /// A fold heading elsewhere ends where it is, so the window moves at
    /// once.
    EndFold,
}

/// What the window does once an animation ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Shown,
    Hidden,
    Destroyed,
}

/// How an animation ended (`Playing::end`).
#[derive(Debug, PartialEq)]
struct Ending<S> {
    /// Where the window ends up.
    rect: Rect,
    outcome: Outcome,
    /// A scene held back during a fold, to show now.
    scene: Option<S>,
    /// What plays next: a preview's way back.
    then: Option<Play>,
}

impl<S> Playing<S> {
    fn new(play: Play, geometry: Geometry, purpose: Purpose<S>, now: Duration) -> Self {
        let transition = Transition::new(play, geometry, now);
        let frame = transition.frame(now);
        Self {
            transition,
            purpose,
            frame,
        }
    }

    fn way(&self) -> Way {
        self.transition.way()
    }

    fn is_visibility(&self) -> bool {
        matches!(self.purpose, Purpose::Visibility { .. })
    }

    fn is_fold(&self) -> bool {
        matches!(self.purpose, Purpose::Fold { .. })
    }

    fn is_dying(&self) -> bool {
        matches!(self.purpose, Purpose::Dying)
    }

    /// Keeps `scene` for the end of a fold, or hands it back to be shown now.
    fn defer(&mut self, scene: S) -> Option<S> {
        let folding = self.way() == Way::Out;
        match &mut self.purpose {
            Purpose::Fold { deferred } if folding => {
                *deferred = Some(scene);
                None
            }
            _ => Some(scene),
        }
    }

    /// Heads `way` from exactly where it is, unless it already does. Returns
    /// a scene held back during a fold, which an unfold shows at once.
    fn head(&mut self, way: Way, now: Duration) -> Option<S> {
        if self.way() == way {
            return None;
        }
        self.transition.reverse(now);
        match &mut self.purpose {
            Purpose::Fold { deferred } => deferred.take(),
            _ => None,
        }
    }

    /// Sets what plays once a hide ends, or with `None` cancels it. Only a
    /// hide or show has one.
    fn set_then(&mut self, play: Option<Play>) {
        if let Purpose::Visibility { then } = &mut self.purpose {
            *then = play;
        }
    }

    /// Becomes a deleted space's way out, turning around from where it is
    /// if it was arriving. A fold cannot, and returns false: it ends first.
    fn dismiss(&mut self, now: Duration) -> bool {
        if self.is_fold() {
            return false;
        }
        self.head(Way::Out, now);
        self.purpose = Purpose::Dying;
        true
    }

    /// The rect the window has once the animation ends, whichever way it is
    /// going now.
    fn end_rect(&self) -> Rect {
        let geometry = self.transition.geometry();
        if self.is_fold() && self.way() == Way::Out {
            geometry.rolled
        } else {
            geometry.rect
        }
    }

    /// What `rect`, the window's new rect, does to the animation. A hiding,
    /// showing, or dying window ends up there instead, keeping its progress
    /// (`work_area_at` gives the work area of the monitor at a point); a
    /// fold heading there carries on, and any other rect ends a fold.
    fn retarget(&mut self, rect: Rect, work_area_at: impl FnOnce(Point) -> Rect) -> Retarget {
        if self.is_fold() {
            return if self.end_rect() == rect {
                Retarget::Taken
            } else {
                Retarget::EndFold
            };
        }
        let play = self.transition.play();
        let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), &play);
        self.transition.set_geometry(geometry);
        Retarget::Taken
    }

    /// Moves to `now`.
    fn step(&mut self, now: Duration) -> Step {
        self.frame = self.transition.frame(now);
        Step {
            frame: self.frame,
            done: self.transition.is_done(now),
            waiting: self.transition.is_waiting(now),
        }
    }

    /// Ends the animation where it was going: shown, folded, or unfolded at
    /// its rect, hidden at the rect it will show at again, or destroyed.
    fn end(self) -> Ending<S> {
        let rect = self.end_rect();
        let gone = self.way() == Way::Out;
        let (outcome, scene, then) = match self.purpose {
            Purpose::Visibility { then } if gone => (Outcome::Hidden, None, then),
            Purpose::Visibility { .. } => (Outcome::Shown, None, None),
            Purpose::Fold { deferred } => (Outcome::Shown, deferred, None),
            Purpose::Dying => (Outcome::Destroyed, None, None),
        };
        Ending {
            rect,
            outcome,
            scene,
            then,
        }
    }
}

/// A space window's running animation.
pub(crate) struct Animation {
    playing: Playing<PreparedScene>,
    /// Burn's flames, on their own window above the space.
    flames: Option<FlameOverlay>,
    /// Windows draws the material flat ("Transparency effects" off), so it
    /// steps aside while a frame leaves the window see-through: a flat
    /// material stayed as a dark box under a fading space (seen 2026-10-18).
    /// A real blur stays: a composition space's follows the frame (T13), and
    /// the accent blur steps at the end.
    flat_material: bool,
    /// The window's frames, which stop when the animation is dropped.
    frames: frame_clock::Subscription,
}

impl Animation {
    pub(crate) fn frame(&self) -> TransitionFrame {
        self.playing.frame
    }

    /// Keeps `scene` for the end of a fold, or hands it back to be shown now.
    pub(crate) fn defer(&mut self, scene: PreparedScene) -> Option<PreparedScene> {
        self.playing.defer(scene)
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

/// Runs `f` on `window`'s running animation, if it has one.
///
/// # Safety
/// As `with_state`: `window` is a live space window, and `f` does not
/// re-enter.
unsafe fn with_animation<R>(window: HWND, f: impl FnOnce(&mut Animation) -> R) -> Option<R> {
    // SAFETY: the caller's contract.
    unsafe { with_state(window, |state| state.animation.as_mut().map(f)) }.flatten()
}

/// Shows `scene` from the next paint on.
///
/// # Safety
/// As `with_state`.
unsafe fn show_scene(window: HWND, scene: PreparedScene) {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe { with_state(window, |state| state.scene = Some(scene)) };
}

/// Shows or hides `window` with `play`. An animation already running the
/// other way turns around from where it is; one already heading there
/// carries on.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn set_visible(window: HWND, visible: bool, play: Play) {
    let way = if visible { Way::In } else { Way::Out };
    // SAFETY: the caller's contract; the borrows do not re-enter.
    unsafe {
        // A real hide or show (the tray, the hotkey, exit) cancels a
        // preview's way back, so a hidden space stays hidden.
        with_animation(window, |animation| animation.playing.set_then(None));
        if turn_around(window, Playing::is_visibility, way) {
            return;
        }
        // A fold still running ends first, so the window hides at its folded
        // or unfolded rect and shows there again.
        finish(window);
    }
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
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), &play);
    let purpose = Purpose::Visibility { then: None };
    // SAFETY: the caller's contract.
    unsafe { start(window, Play { way, ..play }, geometry, purpose) };
}

/// Plays `out` on a shown `window` and then `back`, leaving it shown where
/// it was: the debug window's preview of an effect. Pressed again mid-way,
/// it turns around like any hide or show. It never touches the model.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn preview(window: HWND, out: Play, back: Play) {
    let set_then = |play| {
        // SAFETY: the caller's contract; the borrow does not re-enter.
        unsafe { with_animation(window, |animation| animation.playing.set_then(play)) };
    };
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let heading = unsafe {
        with_animation(window, |animation| {
            let playing = &animation.playing;
            playing.is_visibility().then(|| playing.way())
        })
    }
    .flatten();
    match heading {
        // Leaving: come straight back instead, and stay.
        Some(Way::Out) => {
            set_then(None);
            // SAFETY: the caller's contract.
            unsafe { turn_around(window, Playing::is_visibility, Way::In) };
        }
        // Coming back: leave again, and come back once more after.
        Some(Way::In) => {
            // SAFETY: the caller's contract.
            unsafe { turn_around(window, Playing::is_visibility, Way::Out) };
            set_then(Some(back));
        }
        None => {
            // SAFETY: a plain visibility query on a live window.
            if is_instant(out) || !unsafe { IsWindowVisible(window) }.as_bool() {
                return;
            }
            // SAFETY: the caller's contract.
            unsafe { set_visible(window, false, out) };
            set_then(Some(back));
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
    if unsafe { turn_around(window, Playing::is_fold, play.way) } {
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
    let purpose = Purpose::Fold { deferred: None };
    // SAFETY: the caller's contract.
    unsafe { start(window, play, geometry, purpose) };
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
    let dying = unsafe { with_animation(window, |animation| animation.playing.dismiss(now)) };
    if dying == Some(true) {
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
    let geometry = Geometry::for_visibility(rect, work_area_at(rect.center()), &play);
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

/// Turns a running animation `for_this` purpose around if it is not already
/// heading `way`. Returns whether one was running.
unsafe fn turn_around(
    window: HWND,
    for_this: fn(&Playing<PreparedScene>) -> bool,
    way: Way,
) -> bool {
    let now = now();
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let turned = unsafe {
        with_animation(window, |animation| {
            let playing = &mut animation.playing;
            for_this(playing).then(|| playing.head(way, now))
        })
    }
    .flatten();
    let Some(scene) = turned else {
        return false;
    };
    // Unfolding again shows the newest scene at once.
    if let Some(scene) = scene {
        // SAFETY: the caller's contract.
        unsafe { show_scene(window, scene) };
    }
    true
}

unsafe fn start(window: HWND, play: Play, geometry: Geometry, purpose: Purpose<PreparedScene>) {
    let now = now();
    let mut playing = Playing::new(play, geometry, purpose, now);
    let step = playing.step(now);
    let flames = (play.effect == Effect::Burn)
        .then(|| FlameOverlay::create(window, geometry.rect, play.detail))
        .flatten();
    let flat_material = transparency_enabled() == Some(false);
    // Every caller ends a running animation first. One left over is dropped
    // here, outside the borrow and before the new subscription, which its
    // own would otherwise cancel.
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let previous = unsafe { with_state(window, |state| state.animation.take()) }.flatten();
    debug_assert!(
        previous.is_none(),
        "an animation starts after the last ends"
    );
    drop(previous);
    let animation = Animation {
        playing,
        flames,
        flat_material,
        frames: frame_clock::Subscription::new(window),
    };
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| state.animation = Some(animation));
        if !step.waiting {
            apply(window, step.frame, flat_material);
            draw_flames(window, step.frame, now);
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
    let flames = unsafe { with_animation(window, |animation| animation.flames.take()) }.flatten();
    let Some(mut flames) = flames else {
        return;
    };
    flames.draw(front, now.as_secs_f32());
    // SAFETY: as above.
    unsafe { with_animation(window, |animation| animation.flames = Some(flames)) };
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
        with_animation(window, |animation| {
            (animation.playing.step(now), animation.flat_material)
        })
    };
    // A frame posted before the animation ended finds nothing to do.
    let Some((step, flat_material)) = step else {
        return;
    };
    // SAFETY: the caller's contract.
    unsafe {
        if step.done {
            finish(window);
        } else if !step.waiting {
            apply(window, step.frame, flat_material);
            draw_flames(window, step.frame, now);
        }
    }
}

/// Ends `window`'s animation where it was going (`Playing::end`).
unsafe fn finish(window: HWND) {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    let animation = unsafe { with_state(window, |state| state.animation.take()) }.flatten();
    let Some(animation) = animation else {
        return;
    };
    let Animation {
        playing,
        flames,
        frames,
        ..
    } = animation;
    // The frames stop, and the flames' window goes, outside any state
    // borrow and before a preview's way back starts a new animation.
    drop(frames);
    drop(flames);
    let Ending {
        rect,
        outcome,
        scene,
        then,
    } = playing.end();
    // SAFETY: the caller's contract.
    unsafe {
        if let Some(scene) = scene {
            show_scene(window, scene);
        }
        if outcome == Outcome::Destroyed {
            destroy(window);
            return;
        }
        move_to(window, rect);
        if outcome == Outcome::Hidden {
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
/// animates (`Playing::retarget`). Returns true when the animation took it;
/// a fold it ends leaves the window to move at once.
///
/// # Safety
/// `window` is a live space window; call it outside any state borrow.
pub(crate) unsafe fn retarget(window: HWND, rect: Rect) -> bool {
    // SAFETY: the caller's contract; the borrow does not re-enter, and
    // reading a monitor's work area sends no messages.
    let retarget = unsafe {
        with_animation(window, |animation| {
            animation.playing.retarget(rect, work_area_at)
        })
    };
    match retarget {
        Some(Retarget::Taken) => true,
        Some(Retarget::EndFold) => {
            // SAFETY: the caller's contract.
            unsafe { finish(window) };
            false
        }
        None => false,
    }
}

/// Whether `window` is a deleted space's, playing its way out, so its
/// input no longer reaches the app.
pub(crate) fn is_dying(window: HWND) -> bool {
    // SAFETY: the borrow does not re-enter.
    unsafe { with_animation(window, |animation| animation.playing.is_dying()) }.unwrap_or(false)
}

/// Whether `window` is on its way out (hiding, folding away its items, or
/// deleted), so it takes no drops.
pub(crate) fn is_leaving(window: HWND) -> bool {
    // SAFETY: space windows and the desktop layer share this state; the
    // borrow does not re-enter.
    unsafe { with_animation(window, |animation| animation.playing.way() == Way::Out) }
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

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::Edge;

    /// A scene, as far as `Playing` cares.
    type Scene = u32;

    const SPACE: Rect = Rect::new(300, 100, 240, 160);
    const TITLE: Rect = Rect::new(300, 100, 240, 28);
    const WORK_AREA: Rect = Rect::new(0, 0, 1920, 1040);

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    fn play(way: Way) -> Play {
        Play {
            effect: Effect::Fade,
            way,
            duration: ms(200),
            delay: Duration::ZERO,
            detail: 5,
            edge: Edge::Auto,
        }
    }

    fn showing(way: Way) -> Playing<Scene> {
        let play = play(way);
        let geometry = Geometry::for_visibility(SPACE, WORK_AREA, &play);
        Playing::new(play, geometry, Purpose::Visibility { then: None }, ms(0))
    }

    fn folding(way: Way) -> Playing<Scene> {
        let play = Play {
            effect: Effect::Roll,
            ..play(way)
        };
        let geometry = Geometry::for_fold(SPACE, TITLE, WORK_AREA);
        Playing::new(play, geometry, Purpose::Fold { deferred: None }, ms(0))
    }

    #[test]
    fn a_fold_holds_back_a_scene_until_it_turns_or_ends() {
        let mut fold = folding(Way::Out);
        assert_eq!(fold.defer(7), None, "held back while folding");
        assert_eq!(fold.head(Way::In, ms(50)), Some(7), "an unfold shows it");
        assert_eq!(fold.defer(8), Some(8), "unfolding draws it at once");

        let mut fold = folding(Way::Out);
        fold.defer(9);
        let ending = fold.end();
        assert_eq!(ending.scene, Some(9));
        assert_eq!((ending.rect, ending.outcome), (TITLE, Outcome::Shown));
    }

    #[test]
    fn only_a_fold_holds_back_a_scene() {
        assert_eq!(showing(Way::Out).defer(1), Some(1));
        assert_eq!(showing(Way::In).defer(1), Some(1));
    }

    #[test]
    fn heading_the_way_it_goes_changes_nothing() {
        let mut show = showing(Way::In);
        let before = show.step(ms(80)).frame;
        assert_eq!(show.head(Way::In, ms(80)), None);
        assert_eq!(show.way(), Way::In);
        assert_eq!(show.step(ms(80)).frame, before);
        show.head(Way::Out, ms(80));
        assert_eq!(show.way(), Way::Out, "the other way turns it around");
        assert_eq!(show.step(ms(80)).frame, before, "from the very frame");
    }

    #[test]
    fn a_hide_ends_hidden_and_plays_a_previews_way_back() {
        let mut hide = showing(Way::Out);
        hide.set_then(Some(play(Way::In)));
        let ending = hide.end();
        assert_eq!(ending.outcome, Outcome::Hidden);
        assert_eq!(ending.then, Some(play(Way::In)));
        assert_eq!(ending.rect, SPACE, "hidden where it shows again");
    }

    #[test]
    fn a_turned_preview_ends_shown_with_nothing_after() {
        let mut hide = showing(Way::Out);
        hide.set_then(Some(play(Way::In)));
        hide.head(Way::In, ms(50));
        let ending = hide.end();
        assert_eq!(ending.outcome, Outcome::Shown);
        assert_eq!(ending.then, None);
    }

    #[test]
    fn only_a_hide_or_show_has_a_way_back() {
        let mut fold = folding(Way::Out);
        fold.set_then(Some(play(Way::In)));
        assert_eq!(fold.end().then, None);
    }

    #[test]
    fn a_deleted_space_arriving_turns_around_and_is_destroyed() {
        let mut show = showing(Way::In);
        assert!(show.dismiss(ms(50)));
        assert!(show.is_dying());
        assert_eq!(show.way(), Way::Out);
        assert_eq!(show.end().outcome, Outcome::Destroyed);
    }

    #[test]
    fn a_fold_ends_before_its_space_is_dismissed() {
        let mut fold = folding(Way::Out);
        assert!(!fold.dismiss(ms(50)));
        assert!(fold.is_fold());
    }

    #[test]
    fn a_new_rect_moves_a_hide_or_show_and_keeps_its_progress() {
        let mut show = showing(Way::In);
        let moved = SPACE.offset(100, 50);
        let before = show.step(ms(100));
        assert_eq!(show.retarget(moved, |_| WORK_AREA), Retarget::Taken);
        let after = show.step(ms(100));
        assert_eq!(after.frame.shown, before.frame.shown);
        assert_eq!(after.frame.rect, moved);
        assert_eq!(show.end().rect, moved);
    }

    #[test]
    fn a_fold_keeps_its_own_rect_and_ends_for_any_other() {
        let mut fold = folding(Way::Out);
        assert_eq!(fold.retarget(TITLE, |_| WORK_AREA), Retarget::Taken);
        assert_eq!(fold.retarget(SPACE, |_| WORK_AREA), Retarget::EndFold);
        let mut unfold = folding(Way::In);
        assert_eq!(unfold.retarget(SPACE, |_| WORK_AREA), Retarget::Taken);
    }

    #[test]
    fn a_step_reports_the_delay_and_the_end() {
        let delayed = Play {
            delay: ms(100),
            ..play(Way::In)
        };
        let geometry = Geometry::for_visibility(SPACE, WORK_AREA, &delayed);
        let mut show: Playing<Scene> =
            Playing::new(delayed, geometry, Purpose::Visibility { then: None }, ms(0));
        let waiting = show.step(ms(50));
        assert!(waiting.waiting && !waiting.done);
        assert!(show.step(ms(300)).done);
        assert_eq!(
            show.frame.opacity, 1.0,
            "the painted frame is the last step"
        );
    }
}
