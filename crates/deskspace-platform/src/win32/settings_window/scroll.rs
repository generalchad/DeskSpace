//! Scrolling a settings page that is taller than the window: its scroll bar,
//! the wheel, and PageUp, PageDown, Home, and End.
//!
//! The page's controls keep their layout coordinates and are moved with the
//! content (`SW_SCROLLCHILDREN`), so nothing else in the window has to know a
//! page is scrolled; only `cards::erase_page`, which draws the cards, reads
//! the offset.

use windows::Win32::Foundation::{HWND, WPARAM};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::UI::Controls::SetScrollInfo;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_END, VK_HOME, VK_NEXT, VK_PRIOR};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetScrollInfo, IsChild, MSG, SB_BOTTOM, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN,
    SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP, SB_VERT, SCROLLBAR_COMMAND, SCROLLINFO,
    SIF_PAGE, SIF_POS, SIF_RANGE, SIF_TRACKPOS, SW_ERASE, SW_INVALIDATE, SW_SCROLLCHILDREN,
    ScrollWindowEx, WM_KEYDOWN,
};

use crate::page_scroll::{PageScroll, ScrollInput};
use crate::win32::system::{hiword, loword};

use super::build::scaled;
use super::with_state;

/// One scroll-bar arrow's step, at 96 DPI: about one row of fields.
const SCROLL_LINE: i32 = 40;

/// Shows `scroll` on `page`'s scroll bar, which Windows hides while the page
/// fits.
pub(super) unsafe fn set_bar(page: HWND, scroll: PageScroll) {
    let info = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: scroll.content() - 1,
        nPage: scroll.view() as u32,
        nPos: scroll.offset(),
        nTrackPos: 0,
    };
    // SAFETY: `page` is a live page container of ours.
    unsafe {
        SetScrollInfo(page, SB_VERT, &info, true);
    }
}

/// Moves `page` of the settings window `settings` for `input`.
unsafe fn scroll_page(settings: HWND, page: HWND, input: ScrollInput) {
    // SAFETY: the borrow only updates the offset; the page is scrolled after
    // it, since scrolling sends messages.
    unsafe {
        let Some((moved, scroll)) = with_state(settings, |state| {
            let index = state.pages.iter().position(|each| *each == page)?;
            let line = scaled(SCROLL_LINE, state.dpi);
            let scroll = state.scrolls.get_mut(index)?;
            Some((scroll.apply(input, line), *scroll))
        })
        .flatten() else {
            return;
        };
        if moved == 0 {
            return;
        }
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-scrollwindowex
        let _ = ScrollWindowEx(
            page,
            0,
            -moved,
            None,
            None,
            None,
            None,
            SW_SCROLLCHILDREN | SW_INVALIDATE | SW_ERASE,
        );
        set_bar(page, scroll);
        // Paint the uncovered strip now, so a held arrow or a fast wheel does
        // not leave it blank until the queue empties.
        let _ = UpdateWindow(page);
    }
}

/// Puts the page at `index` back at its top, as a page newly shown starts.
pub(super) unsafe fn to_top(settings: HWND, index: usize) {
    // SAFETY: the handle is copied out under a short borrow.
    unsafe {
        if let Some(page) = with_state(settings, |state| state.pages.get(index).copied()).flatten()
        {
            scroll_page(settings, page, ScrollInput::Top);
        }
    }
}

/// `WM_VSCROLL` from `page`'s own scroll bar.
pub(super) unsafe fn on_vscroll(settings: HWND, page: HWND, wparam: WPARAM) {
    let input = match SCROLLBAR_COMMAND(loword(wparam.0 as isize)) {
        SB_LINEUP => ScrollInput::LineUp,
        SB_LINEDOWN => ScrollInput::LineDown,
        SB_PAGEUP => ScrollInput::PageUp,
        SB_PAGEDOWN => ScrollInput::PageDown,
        SB_TOP => ScrollInput::Top,
        SB_BOTTOM => ScrollInput::Bottom,
        SB_THUMBTRACK | SB_THUMBPOSITION => {
            // The message carries only 16 bits of position; the scroll bar
            // has the full one.
            let mut info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_TRACKPOS,
                ..Default::default()
            };
            // SAFETY: `info` is a valid out-parameter for the live page.
            let _ = unsafe { GetScrollInfo(page, SB_VERT, &mut info) };
            ScrollInput::To(info.nTrackPos)
        }
        _ => return,
    };
    // SAFETY: the caller passes the live settings window and its page.
    unsafe { scroll_page(settings, page, input) }
}

/// A wheel notch over `page`, or passed up from one of its controls.
pub(super) unsafe fn on_wheel(settings: HWND, page: HWND, wparam: WPARAM) {
    let delta = hiword(wparam.0 as isize);
    // SAFETY: the caller passes the live settings window and its page.
    unsafe { scroll_page(settings, page, ScrollInput::Wheel(delta)) }
}

/// PageUp, PageDown, Home, or End pressed while a control on a settings page
/// has the focus: scrolls that page and returns true, so the message loop
/// drops the key. A drop-down, a text box, and a slider keep these keys,
/// since each moves its own value or caret with them.
pub(crate) fn on_page_key(settings: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN {
        return false;
    }
    let input = match message.wParam.0 as u16 {
        key if key == VK_PRIOR.0 => ScrollInput::PageUp,
        key if key == VK_NEXT.0 => ScrollInput::PageDown,
        key if key == VK_HOME.0 => ScrollInput::Top,
        key if key == VK_END.0 => ScrollInput::Bottom,
        _ => return false,
    };
    let focus = message.hwnd;
    // SAFETY: `settings` is the live settings window; the handles are copied
    // out under a short borrow, and the class read is into a sized buffer.
    unsafe {
        let Some(pages) = with_state(settings, |state| state.pages.clone()) else {
            return false;
        };
        let Some(page) = pages
            .into_iter()
            .find(|page| IsChild(*page, focus).as_bool())
        else {
            return false;
        };
        let mut class = [0u16; 32];
        let length = GetClassNameW(focus, &mut class) as usize;
        let keeps_keys = matches!(
            String::from_utf16_lossy(&class[..length]).as_str(),
            "ComboBox" | "Edit" | "msctls_trackbar32"
        );
        if keeps_keys {
            return false;
        }
        scroll_page(settings, page, input);
        true
    }
}
