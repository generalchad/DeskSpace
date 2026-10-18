//! Places the section list, the pane, and the current section's page, and
//! scrolls that page inside the pane.

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HWND, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetDC, GetTextExtentPoint32W, HFONT, HGDIOBJ, InvalidateRect, ReleaseDC, SelectObject,
};
use windows::Win32::UI::Controls::SetScrollInfo;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetScrollInfo, GetWindowTextLengthW, GetWindowTextW, LB_GETCURSEL, SB_BOTTOM,
    SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP,
    SB_VERT, SCROLLBAR_COMMAND, SCROLLINFO, SIF_ALL, SIF_PAGE, SIF_POS, SIF_RANGE, SW_HIDE,
    SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetWindowPos, ShowWindow,
};

use super::console;
use super::layout::{console_layout, control_rects, debug_layout, group_rects, s, section_rows};
use super::with_state;
use crate::ui::DebugControlKind;
use crate::win32::dpi::{scale_for, window_dpi};
use crate::win32::system::{hiword, loword};

/// How far one wheel notch and one scroll-bar arrow move the panel, at 96
/// DPI.
const WHEEL_STEP: i32 = 48;
const LINE_STEP: i32 = 20;
/// One wheel notch (WHEEL_DELTA, WinUser.h).
const WHEEL_DELTA: i32 = 120;

/// Places the section list, the pane, and the current page for the client
/// size.
pub(super) unsafe fn relayout(hwnd: HWND) {
    // SAFETY: `hwnd` is the live debug window; positions are set outside the
    // state borrow.
    unsafe {
        let Some((rail, pane)) = with_state(hwnd, |state| (state.rail, state.pane)) else {
            return;
        };
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let scale = scale_for(window_dpi(hwnd));
        let layout = debug_layout(client.right, client.bottom, scale);
        for (part, rect) in [(rail, layout.rail), (pane, layout.panel)] {
            if let Some(part) = part {
                place(part, rect);
            }
        }
        if let Some(controls) = with_state(hwnd, |state| state.console_controls) {
            console::place_all(controls, console_layout(layout.console, scale), scale);
        }
        layout_page(hwnd);
    }
}

/// Shows the current section's page, sized and scrolled to fit the pane,
/// with its controls in their rows, and hides the other pages.
pub(super) unsafe fn layout_page(hwnd: HWND) {
    let scale = scale_for(window_dpi(hwnd));
    // SAFETY: what the layout needs is copied out under a short borrow; the
    // windows are moved after it.
    unsafe {
        let Some((Some(pane), pages, current, scroll, font)) = with_state(hwnd, |state| {
            (
                state.pane,
                state
                    .sections
                    .iter()
                    .map(|section| section.page)
                    .collect::<Vec<_>>(),
                state.current,
                state.scroll,
                state.chrome.font,
            )
        }) else {
            return;
        };
        let Some((rows, controls, groups)) = with_state(hwnd, |state| {
            let rows: Vec<(DebugControlKind, Option<String>)> = state
                .panel
                .sections
                .get(current)
                .map(|section| {
                    section
                        .controls
                        .iter()
                        .map(|c| (c.kind.clone(), c.group.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let built = state.sections.get(current);
            let controls: Vec<(Option<HWND>, Option<HWND>)> = built
                .map(|section| section.controls.iter().map(|c| (c.label, c.main)).collect())
                .unwrap_or_default();
            let groups: Vec<Option<HWND>> = built
                .map(|section| section.group_titles.clone())
                .unwrap_or_default();
            (rows, controls, groups)
        }) else {
            return;
        };
        let layout = section_rows(
            rows.iter().map(|(kind, group)| (kind, group.as_deref())),
            scale,
        );
        let content = layout.height;
        let mut client = RECT::default();
        let _ = GetClientRect(pane, &mut client);
        let viewport = client.bottom;
        let scroll = scroll.clamp(0, (content - viewport).max(0));
        with_state(hwnd, |state| state.scroll = scroll);
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: content - 1,
            nPage: viewport.max(0) as u32,
            nPos: scroll,
            nTrackPos: 0,
        };
        SetScrollInfo(pane, SB_VERT, &info, true);
        // Showing or hiding the scroll bar changes the pane's client width.
        let _ = GetClientRect(pane, &mut client);
        let width = client.right;
        for (index, page) in pages.iter().enumerate() {
            if index != current {
                let _ = ShowWindow(*page, SW_HIDE);
            }
        }
        let Some(page) = pages.get(current).copied() else {
            return;
        };
        place(page, Rect::new(0, -scroll, width, content.max(viewport)));
        let mut frames = Vec::with_capacity(groups.len());
        for (span, title) in layout.groups.iter().zip(groups) {
            let title_width = title.map(|title| caption_width(title, font)).unwrap_or(0);
            let (frame, title_rect) = group_rects(*span, width, title_width, scale);
            frames.push(frame);
            if let Some(title) = title {
                place(title, title_rect);
            }
        }
        // The page draws the boxes as it erases, so new ones need an erase.
        let moved = with_state(hwnd, |state| {
            state.sections.get_mut(current).is_some_and(|section| {
                std::mem::replace(&mut section.group_frames, frames) != section.group_frames
            })
        });
        if moved == Some(true) {
            let _ = InvalidateRect(Some(page), None, true);
        }
        let placed = rows
            .iter()
            .zip(controls)
            .zip(layout.tops)
            .zip(layout.insets);
        for ((((kind, _), (label, main)), top), inset) in placed {
            let caption = main.map(|main| caption_width(main, font)).unwrap_or(0);
            let (label_rect, main_rect) = control_rects(kind, top, inset, width, caption, scale);
            if let (Some(label), Some(rect)) = (label, label_rect) {
                place(label, rect);
            }
            if let Some(main) = main {
                place(main, main_rect);
            }
        }
        let _ = ShowWindow(page, SW_SHOW);
    }
}

/// The width of a control's caption in `font`.
unsafe fn caption_width(control: HWND, font: HFONT) -> i32 {
    // SAFETY: the DC is released before returning, with its old font back.
    unsafe {
        let length = GetWindowTextLengthW(control).max(0) as usize;
        if length == 0 {
            return 0;
        }
        let mut text = vec![0u16; length + 1];
        let copied = GetWindowTextW(control, &mut text).max(0) as usize;
        let dc = GetDC(Some(control));
        let old = SelectObject(dc, HGDIOBJ(font.0));
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &text[..copied], &mut size);
        SelectObject(dc, old);
        ReleaseDC(Some(control), dc);
        size.cx
    }
}

pub(super) fn place(window: HWND, rect: Rect) {
    // SAFETY: `window` is a live child of the debug window.
    unsafe {
        let _ = SetWindowPos(
            window,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Scrolls the current page to `to` (clamped).
unsafe fn scroll_to(hwnd: HWND, to: i32) {
    // SAFETY: the borrow is short; the layout runs after it.
    unsafe {
        let changed = with_state(hwnd, |state| {
            let changed = state.scroll != to;
            state.scroll = to;
            changed
        });
        if changed == Some(true) {
            layout_page(hwnd);
        }
    }
}

/// The pane's scroll bar, forwarded from the pane.
pub(super) unsafe fn on_vscroll(hwnd: HWND, wparam: WPARAM) {
    let scale = scale_for(window_dpi(hwnd));
    // SAFETY: `hwnd` is the live debug window; the pane is its child.
    unsafe {
        let Some((Some(pane), scroll)) = with_state(hwnd, |state| (state.pane, state.scroll))
        else {
            return;
        };
        let mut info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_ALL,
            ..Default::default()
        };
        let _ = GetScrollInfo(pane, SB_VERT, &mut info);
        let page = info.nPage as i32;
        let to = match SCROLLBAR_COMMAND(loword(wparam.0 as isize)) {
            SB_LINEUP => scroll - s(LINE_STEP, scale),
            SB_LINEDOWN => scroll + s(LINE_STEP, scale),
            SB_PAGEUP => scroll - page,
            SB_PAGEDOWN => scroll + page,
            SB_THUMBTRACK | SB_THUMBPOSITION => info.nTrackPos,
            SB_TOP => 0,
            SB_BOTTOM => info.nMax,
            _ => return,
        };
        scroll_to(hwnd, to);
    }
}

pub(super) unsafe fn on_wheel(hwnd: HWND, wparam: WPARAM) {
    let delta = hiword(wparam.0 as isize);
    let step = s(WHEEL_STEP, scale_for(window_dpi(hwnd)));
    // SAFETY: `hwnd` is the live debug window.
    unsafe {
        if let Some(scroll) = with_state(hwnd, |state| state.scroll) {
            scroll_to(hwnd, scroll - delta * step / WHEEL_DELTA);
        }
    }
}

/// The section list moved: show that section from its top.
pub(super) unsafe fn on_section_changed(hwnd: HWND, rail: HWND) {
    // SAFETY: `rail` is our live list box; the borrow is short.
    unsafe {
        let index = SendMessageW(rail, LB_GETCURSEL, None, None).0;
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        with_state(hwnd, |state| {
            state.current = index;
            state.scroll = 0;
        });
        layout_page(hwnd);
    }
}
