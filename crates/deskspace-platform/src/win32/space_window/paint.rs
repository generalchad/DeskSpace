//! Painting a space window: checking out its painter and scene, drawing the
//! current frame, and showing the blur through the frame's mask afterwards.

use std::time::Instant;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::win32::dpi::window_dpi;
use crate::win32::space_animation::Animation;

use super::with_state;

pub(super) unsafe fn paint(window: HWND) {
    // SAFETY: for every call, `window` is live and the state borrows are
    // short.
    unsafe {
        let checked_out = with_state(window, |state| {
            let mut frame = state.frame.paint(window_dpi(window));
            frame.highlight = state.highlight.level();
            (
                state.painter.take(),
                state.scene.take(),
                frame,
                state.animation.as_ref().map(Animation::frame),
            )
        });
        let Some((Some(mut painter), scene, frame, transition)) = checked_out else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        let started = Instant::now();
        let painted = painter.paint(window, scene.as_ref(), frame, transition.as_ref());
        let elapsed = started.elapsed();
        let mut client = RECT::default();
        if GetClientRect(window, &mut client).is_ok() {
            let width = client.right - client.left;
            let height = client.bottom - client.top;
            if elapsed > crate::SCENE_STEP_BUDGET {
                tracing::debug!(
                    width,
                    height,
                    elapsed_us = elapsed.as_micros(),
                    "scene: painted"
                );
            } else {
                tracing::trace!(
                    width,
                    height,
                    elapsed_us = elapsed.as_micros(),
                    "scene: painted"
                );
            }
        }
        if let Err(error) = &painted {
            tracing::error!(%error, "space paint failed");
        }
        // The painter drew the blur's mask with a frame that leaves the
        // window see-through; only then does the blur show through it.
        let masked = painted.is_ok()
            && transition
                .as_ref()
                .is_some_and(|transition| !transition.covers_window());
        with_state(window, |state| {
            state.painter = Some(painter);
            if state.scene.is_none() {
                state.scene = scene;
            }
            if let Some(visuals) = &mut state.visuals
                && let Err(error) = visuals.mask_blur(masked)
            {
                tracing::warn!(%error, "space composition backdrop not masked");
            }
        });
    }
}
