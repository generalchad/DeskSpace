//! The settings window's highlight outline, which fades in and out rather
//! than switching (ROADMAP D26). The window owns the fade's clock here: a
//! scene only says whether the outline is on.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::InvalidateRect;

use crate::render::SpaceScene;
use crate::render::highlight::HighlightFade;
use crate::win32::frame_clock;
use crate::win32::space_animation;

use super::with_state;

/// A space window's highlight: the level it paints the outline at, and the
/// frames that carry a fade while one runs.
pub(super) struct Highlight {
    fade: HighlightFade,
    frames: Option<frame_clock::Subscription>,
}

impl Highlight {
    pub(super) fn new() -> Self {
        Self {
            fade: HighlightFade::at_rest(false),
            frames: None,
        }
    }

    /// The level to paint the outline at now, 0 to 1.
    pub(super) fn level(&self) -> f32 {
        self.fade.level(space_animation::now())
    }

    /// Follows `previous` to `scene`: on a change it fades when the scene asks
    /// for it, and switches otherwise.
    pub(super) fn follow(&mut self, previous: Option<bool>, scene: &SpaceScene, window: HWND) {
        if previous == Some(scene.highlight) {
            return;
        }
        let now = space_animation::now();
        if scene.highlight_fades {
            self.fade.toward(scene.highlight, now);
            if self.frames.is_none() {
                self.frames = Some(frame_clock::Subscription::new(window));
            }
        } else {
            self.frames = None;
            self.fade.jump(scene.highlight);
        }
    }

    /// Advances to now: whether a fade was running, so the window repaints
    /// (the frame that ends one draws its final level), and whether it ended,
    /// so the frames can stop.
    fn step(&mut self) -> bool {
        let running = self.frames.is_some();
        if running && self.fade.is_done(space_animation::now()) {
            self.frames = None;
        }
        running
    }
}

/// Stores `scene` as the window's, starting or ending the highlight's fade
/// when the outline changed.
///
/// # Safety
/// As `with_state`: `window` is a live space window, and this does not
/// re-enter.
pub(crate) unsafe fn store_scene(window: HWND, scene: crate::render::PreparedScene) {
    // SAFETY: the caller's contract; the borrow does not re-enter.
    unsafe {
        with_state(window, |state| {
            let previous = state.scene.as_ref().map(|scene| scene.scene().highlight);
            state.highlight.follow(previous, scene.scene(), window);
            state.scene = Some(scene);
        });
    }
}

/// Steps the highlight on a frame, repainting while its level moves.
///
/// # Safety
/// As `with_state`.
pub(crate) unsafe fn on_frame(window: HWND) {
    // SAFETY: the caller's contract; the borrow ends before the repaint.
    let moving = unsafe { with_state(window, |state| state.highlight.step()) };
    if moving == Some(true) {
        // SAFETY: a live window owned by this thread; invalidating sends no
        // message.
        unsafe {
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}
