//! The header title's scroll: measuring it, its timer and frames, and the
//! frame that draws it. The pure motion is `crate::title_scroll`.

use std::time::Duration;

use windows::Win32::Graphics::Gdi::{DrawTextW, IntersectClipRect, ReleaseDC, RestoreDC, SaveDC};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, KillTimer, SetTimer};

use super::header::{TITLE_TEXT_MAX, TitleMotion, set_title_tip};
use super::*;

/// Measures the header's title and restarts its scroll when the text, the
/// box, or the DPI changed. Called after the header is built or updated.
pub(super) unsafe fn sync_title(window: HWND) {
    // SAFETY: the borrow copies handles and text; measuring uses our font and
    // sends no message that re-enters the window procedure.
    unsafe {
        let Some((controls, layout, header, font, key, renaming)) = with_state(window, |state| {
            (
                state.header,
                state.header_layout,
                state.form.nav.header.clone(),
                state.heading_font,
                state.title_motion.as_ref().map(TitleMotion::key),
                state.renaming,
            )
        }) else {
            return;
        };
        let Some(header) = header else {
            with_state(window, |state| state.title_motion = None);
            stop_title(window);
            set_title_tip(window, controls, "");
            return;
        };
        if controls.title.is_none() {
            return;
        }
        let (_, title_rect) = layout.place(header.icon.is_some());
        let width = title_rect.width.max(0);
        let dpi = GetDpiForWindow(window).max(96);
        if key.is_some_and(|key| !key.needs_restart(&header.title, width, dpi)) {
            return;
        }
        let animate = crate::win32::system::system_animations();
        let text_width = measure_title(window, &header.title, font);
        let motion = TitleMotion::new(
            &header.title,
            width,
            dpi,
            dpi as f32 / 96.0,
            text_width,
            animate,
        );
        let tip = if motion.overflow() > 0.0 {
            header.title.clone()
        } else {
            String::new()
        };
        let next = motion.next_move();
        with_state(window, |state| state.title_motion = Some(motion));
        set_title_tip(window, controls, &tip);
        let _ = KillTimer(Some(window), TITLE_TIMER);
        if !renaming && let Some(next) = next {
            set_title_timer(window, next);
        }
    }
}

/// Measures `text` with the header's font, in pixels.
unsafe fn measure_title(window: HWND, text: &str, font: HFONT) -> f32 {
    // SAFETY: the DC is released below; the font outlives the call.
    unsafe {
        let dc = GetDC(Some(window));
        let previous = SelectObject(dc, HGDIOBJ(font.0));
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        crate::win32::system::draw_text(
            dc,
            text,
            &mut bounds,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, previous);
        ReleaseDC(Some(window), dc);
        (bounds.right - bounds.left) as f32
    }
}

/// Stops the title's frames and its pause timer.
pub(super) unsafe fn stop_title(window: HWND) {
    // SAFETY: the borrow only drops the subscription.
    unsafe {
        with_state(window, |state| {
            if let Some(motion) = state.title_motion.as_mut() {
                motion.stop_frames();
            }
        });
        let _ = KillTimer(Some(window), TITLE_TIMER);
    }
}

/// Restarts the title's motion from offset 0, unless it is renaming or the
/// window is minimized.
pub(super) unsafe fn restart_title(window: HWND) {
    // SAFETY: the borrow only resets the timer origin and drops the frames.
    unsafe {
        let next = with_state(window, |state| {
            if state.renaming || IsIconic(window).as_bool() {
                if let Some(motion) = state.title_motion.as_mut() {
                    motion.stop_frames();
                }
                return None;
            }
            let motion = state.title_motion.as_mut()?;
            motion.restart();
            motion.next_move()
        })
        .flatten();
        let _ = KillTimer(Some(window), TITLE_TIMER);
        if let Some(next) = next {
            set_title_timer(window, next);
        }
    }
}

/// Starts or stops the scroll when the window is minimized or restored.
pub(super) unsafe fn on_window_size(window: HWND) {
    // SAFETY: plain window queries; the helpers follow their own rules.
    unsafe {
        if IsIconic(window).as_bool() {
            stop_title(window);
        } else {
            restart_title(window);
        }
    }
}

/// The pause timer fired: start a traverse if one is due, or wait for the
/// next one.
pub(super) unsafe fn on_title_timer(window: HWND) {
    // SAFETY: the borrow only reads the motion and may create a subscription.
    unsafe {
        let (moving, next) = with_state(window, |state| {
            if state.renaming {
                return (false, None);
            }
            let Some(motion) = state.title_motion.as_mut() else {
                return (false, None);
            };
            if motion.is_moving() {
                motion.ensure_frames(window);
                (true, None)
            } else {
                (false, motion.next_move())
            }
        })
        .unwrap_or((false, None));
        if moving {
            invalidate_title(window);
        } else if let Some(next) = next {
            set_title_timer(window, next);
        }
    }
}

/// A frame arrived: move the title while a traverse runs, then stop the
/// frames and wait for the next traverse.
pub(super) unsafe fn on_title_frame(window: HWND) {
    // SAFETY: the borrow only reads the motion and may create a subscription.
    unsafe {
        let (present, moving, next) = with_state(window, |state| {
            let Some(motion) = state.title_motion.as_mut() else {
                return (false, false, None);
            };
            let moving = motion.is_moving();
            if moving {
                motion.ensure_frames(window);
            } else {
                motion.stop_frames();
            }
            (true, moving, if moving { None } else { motion.next_move() })
        })
        .unwrap_or((false, false, None));
        if !present {
            return;
        }
        invalidate_title(window);
        if !moving && let Some(next) = next {
            set_title_timer(window, next);
        }
    }
}

fn invalidate_title(window: HWND) {
    // SAFETY: a live child of `window`; invalidating sends no message.
    unsafe {
        if let Some(Some(title)) = with_state(window, |state| state.header.title) {
            let _ = InvalidateRect(Some(title), None, false);
        }
    }
}

fn set_title_timer(window: HWND, after: Duration) {
    let millis = after.as_millis().clamp(1, u32::MAX as u128) as u32;
    // SAFETY: a timer on our live window; the id is beside its other ids.
    unsafe {
        let _ = SetTimer(Some(window), TITLE_TIMER, millis, None);
    }
}

/// A long name scrolls in the header: the icon slot and title are
/// painted here, and the title's offset comes from its `TitleScroll`.
pub(super) unsafe fn on_draw_title(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is the title's live DC during WM_DRAWITEM; the state
    // borrow copies its text and ends before any drawing.
    unsafe {
        let Some((colors, body, font, mut text, len, offset, ellipsis)) =
            with_state(window, |state| {
                let mut text = [0u16; TITLE_TEXT_MAX];
                let mut len = 0;
                let mut offset = 0;
                let mut ellipsis = false;
                if let Some(motion) = state.title_motion.as_ref() {
                    let title = motion.text();
                    len = title.len().min(TITLE_TEXT_MAX);
                    text[..len].copy_from_slice(&title[..len]);
                    offset = motion.offset().round() as i32;
                    ellipsis = motion.ellipsizes();
                }
                (
                    state.colors,
                    state.body_brush,
                    state.heading_font,
                    text,
                    len,
                    offset,
                    ellipsis,
                )
            })
        else {
            return;
        };
        FillRect(item.hDC, &item.rcItem, body);
        if len == 0 {
            return;
        }
        let saved = SaveDC(item.hDC);
        let _ = IntersectClipRect(
            item.hDC,
            item.rcItem.left,
            item.rcItem.top,
            item.rcItem.right,
            item.rcItem.bottom,
        );
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(item.hDC, colorref(colors.text));
        let mut rect = item.rcItem;
        rect.left -= offset;
        let format = DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX;
        let format = if ellipsis {
            format | DT_END_ELLIPSIS
        } else {
            format
        };
        DrawTextW(item.hDC, &mut text[..len], &mut rect, format);
        SelectObject(item.hDC, previous);
        if saved != 0 {
            let _ = RestoreDC(item.hDC, saved);
        }
    }
}
