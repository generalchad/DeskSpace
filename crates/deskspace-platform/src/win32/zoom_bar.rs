//! The zoom pop-up window (ROADMAP F-57): a small bar at the top center of
//! the monitor under the pointer that shows the icon size and offers zoom
//! out, zoom in, and Reset. The app decides when it shows and hides; this
//! module places it, paints it (`render::d2d::ZoomBarPainter`), and reports
//! button clicks as `Event::Zoom`.
//!
//! Like a space it is never activated, so showing it never takes the focus
//! from the app or the desktop the user is working in, and it takes the
//! space's material through `dwm::apply_style`.

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ValidateRect};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowRect, HWND_TOPMOST, IsWindowVisible, MA_NOACTIVATE,
    SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WM_DPICHANGED,
    WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCCREATE,
    WM_NCDESTROY, WM_PAINT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, ZoomAction};
use crate::render::SpaceStyle;
use crate::render::d2d::ZoomBarPainter;
use crate::render::zoom_bar::{ZOOM_BAR_MARGIN, ZoomBar, ZoomBarLayout, zoom_bar_origin};
use crate::win32::system::{cursor_pos, hiword, loword, work_area_at};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{dpi, dwm, runtime, window_state};

const CLASS_NAME: PCWSTR = w!("DeskSpaceZoomBar");

struct ZoomBarState {
    /// `None` until the first `show`.
    bar: Option<ZoomBar>,
    /// `bar.label()` as UTF-16, so painting does not format.
    label: Vec<u16>,
    /// The DPI factor of the monitor the bar was last shown on.
    scale: f32,
    hovered: Option<ZoomAction>,
    /// The button the left press went down on; a click fires only when the
    /// release is on the same one.
    pressed: Option<ZoomAction>,
    tracking_leave: bool,
    /// The style last applied, so showing the bar again with the same one
    /// skips the DWM calls.
    applied_style: Option<SpaceStyle>,
    /// `None` while `WM_PAINT` has it checked out.
    painter: Option<ZoomBarPainter>,
}

/// Creates the pop-up window, hidden.
pub(crate) fn create() -> Result<HWND, PlatformError> {
    window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)).arrow_cursor())?;
    let state = Box::into_raw(Box::new(ZoomBarState {
        bar: None,
        label: Vec::new(),
        scale: 1.0,
        hovered: None,
        pressed: None,
        tracking_leave: false,
        applied_style: None,
        painter: Some(ZoomBarPainter::default()),
    }));
    // SAFETY: plain Win32 window creation. Ownership of `state` passes to the
    // window in WM_NCCREATE and it is freed in WM_NCDESTROY; if creation fails
    // after WM_NCCREATE, WM_NCDESTROY has already freed it.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            CLASS_NAME,
            w!("DeskSpace zoom"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(window_class::instance()?),
            Some(state as *const core::ffi::c_void),
        )?
    };
    Ok(hwnd)
}

/// Shows `bar` at the top center of the monitor under the pointer, above
/// other windows, without activating it.
pub(crate) fn show(hwnd: HWND, bar: ZoomBar) {
    let cursor = cursor_pos();
    let scale = dpi::scale_for(dpi::monitor_dpi_at(cursor));
    let layout = ZoomBarLayout::new();
    let scaled = |value: i32| (value as f32 * scale).round() as i32;
    let (width, height) = (scaled(layout.width), scaled(layout.height));
    let origin = zoom_bar_origin(work_area_at(cursor), width, scaled(ZOOM_BAR_MARGIN));
    // SAFETY: `hwnd` is the live pop-up; the borrow does not re-enter.
    let restyle = unsafe {
        with_state(hwnd, |state| {
            state.label = bar.label().encode_utf16().collect();
            state.scale = scale;
            state.bar = Some(bar);
            state.applied_style.replace(bar.style) != Some(bar.style)
        })
    };
    if restyle == Some(true) {
        dwm::apply_style(hwnd, bar.style);
    }
    // SAFETY: `hwnd` is live. The messages SetWindowPos sends go to
    // DefWindowProcW, which does not touch the runtime.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            origin.x,
            origin.y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

pub(crate) fn hide(hwnd: HWND) {
    // SAFETY: `hwnd` is the live pop-up; the borrow does not re-enter.
    unsafe {
        with_state(hwnd, |state| {
            state.hovered = None;
            state.pressed = None;
        });
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

/// Whether the pop-up is showing with the pointer over it.
pub(crate) fn is_hovered(hwnd: HWND) -> bool {
    let mut rect = RECT::default();
    // SAFETY: plain queries of a live window.
    let visible =
        unsafe { IsWindowVisible(hwnd).as_bool() && GetWindowRect(hwnd, &mut rect).is_ok() };
    let cursor = cursor_pos();
    visible
        && cursor.x >= rect.left
        && cursor.x < rect.right
        && cursor.y >= rect.top
        && cursor.y < rect.bottom
}

/// Runs `f` with a short-lived borrow of the pop-up's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut ZoomBarState) -> R) -> Option<R> {
    // SAFETY: the caller's contract; this class always stores a ZoomBarState.
    unsafe { window_state::with_state::<ZoomBarState, R>(window, f) }
}

/// The enabled button under a client-area point, in the layout's 96-DPI
/// pixels. Same contract as `with_state`.
unsafe fn button_at(window: HWND, lparam: LPARAM) -> Option<ZoomAction> {
    // SAFETY: the caller's contract.
    unsafe {
        with_state(window, |state| {
            let bar = state.bar?;
            let point = Point::new(
                (loword(lparam.0) as f32 / state.scale) as i32,
                (hiword(lparam.0) as f32 / state.scale) as i32,
            );
            ZoomBarLayout::new()
                .hit(point)
                .filter(|action| bar.enabled(*action))
        })
        .flatten()
    }
}

unsafe fn paint(window: HWND) {
    // SAFETY: `window` is live and each state borrow is short; the painter and
    // label are checked out so the paint itself holds no borrow.
    unsafe {
        let checked_out = with_state(window, |state| {
            let bar = state.bar?;
            let painter = state.painter.take()?;
            Some((
                painter,
                bar,
                std::mem::take(&mut state.label),
                state.hovered,
                state.scale,
            ))
        })
        .flatten();
        let Some((mut painter, bar, label, hovered, scale)) = checked_out else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        if let Err(error) = painter.paint(window, &bar, &label, hovered, scale) {
            tracing::error!(%error, "zoom pop-up paint failed");
        }
        with_state(window, |state| {
            state.painter = Some(painter);
            if state.label.is_empty() {
                state.label = label;
            }
        });
    }
}

unsafe fn on_mouse_move(window: HWND, lparam: LPARAM) {
    // SAFETY: `window` is live; the state borrows are short.
    unsafe {
        let start_tracking = with_state(window, |state| {
            !std::mem::replace(&mut state.tracking_leave, true)
        });
        if start_tracking == Some(true) {
            // Ask for WM_MOUSELEAVE so the hover fill clears.
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackmouseevent
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
        }
        let hovered = button_at(window, lparam);
        set_hovered(window, hovered);
    }
}

/// Repaints when the hovered button changes. Same contract as `with_state`.
unsafe fn set_hovered(window: HWND, hovered: Option<ZoomAction>) {
    // SAFETY: the caller's contract.
    unsafe {
        let changed = with_state(window, |state| {
            std::mem::replace(&mut state.hovered, hovered) != hovered
        });
        if changed == Some(true) {
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; state
    // access follows the rules on `with_state`.
    unsafe {
        match message {
            WM_NCCREATE => {
                window_state::attach_create_param::<ZoomBarState>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            // Clicking the bar must not activate it and take the focus.
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            // The bar is sized for its monitor's DPI on every show, so the
            // system's suggested rectangle is not applied.
            WM_DPICHANGED => LRESULT(0),
            WM_MOUSEMOVE => {
                on_mouse_move(window, lparam);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with_state(window, |state| {
                    state.tracking_leave = false;
                    state.pressed = None;
                });
                set_hovered(window, None);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                SetCapture(window);
                let pressed = button_at(window, lparam);
                with_state(window, |state| state.pressed = pressed);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                let released = button_at(window, lparam);
                let pressed = with_state(window, |state| state.pressed.take()).flatten();
                if let Some(action) = pressed.filter(|action| Some(*action) == released) {
                    // Last: the app re-shows the bar from inside the dispatch.
                    runtime::dispatch(Event::Zoom(action));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                window_state::free::<ZoomBarState>(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
