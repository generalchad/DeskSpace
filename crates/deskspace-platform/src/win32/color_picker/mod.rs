//! The color picker popup (ROADMAP F-51), replacing the dated system
//! `ChooseColor` dialog. One page holds the saturation/value square, the hue
//! and opacity sliders, the recent and default swatches, an eyedropper, and
//! one value box that reads and writes hex, RGB, HSL, or HSB. It returns the
//! chosen color from a short modal loop.
//!
//! `state.rs` holds what each input does, `format.rs` the value box's text,
//! and `render::color_picker` the layout; `d2d::ColorPickerPainter` paints
//! it over the Windows 11 acrylic backdrop (opaque where that is missing).
//!
//! `pick` owns the `Picker` box; the window only points at it through
//! `GWLP_USERDATA` for the duration of the loop, which ends when the window is
//! destroyed. The owner (the settings window or the controller) is disabled
//! while it is open, so only the picker can be driven, but the loop pumps
//! every message so Windows does not mark the owner "not responding".

use deskspace_core::geometry::Point;
use deskspace_core::theme::Color;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetDC, GetPixel, InvalidateRect, ReleaseDC, ScreenToClient, ValidateRect,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GW_OWNER,
    GetCaretBlinkTime, GetCursorPos, GetMessageW, GetWindow, HTCAPTION, HTCLIENT, IDC_ARROW,
    IDC_CROSS, IDC_IBEAM, IsWindow, KillTimer, LoadCursorW, MSG, PostQuitMessage, SW_SHOW,
    SWP_NOACTIVATE, SWP_NOZORDER, SetCursor, SetForegroundWindow, SetTimer, SetWindowPos,
    ShowWindow, TranslateMessage, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_DPICHANGED,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_PAINT, WM_RBUTTONDOWN,
    WM_SETCURSOR, WM_TIMER, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::edit::EditKey;
use crate::error::PlatformError;
use crate::render::color_picker::Region;
use crate::render::d2d::ColorPickerPainter;
use crate::win32::dark_mode::UiColors;
use crate::win32::system::{hiword, loword, system_accent_color, work_area_at};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{clipboard, dpi, dwm, window_state};

mod draw;
mod format;
mod state;

pub(crate) use draw::draw_color_box;
use state::{Outcome, PickerKey, PickerState, picker_key};

const CLASS_NAME: PCWSTR = w!("DeskSpaceColorPicker");
const CARET_TIMER: usize = 1;
/// `GetCaretBlinkTime` when the user turned blinking off.
const NO_BLINK: u32 = u32::MAX;
/// `GetPixel`'s failure value.
const CLR_INVALID: u32 = 0xFFFF_FFFF;

/// What the window points at: the picker's state and what only the window
/// needs.
struct Picker {
    state: PickerState,
    /// `None` while `WM_PAINT` has it checked out.
    painter: Option<ColorPickerPainter>,
    tracking_leave: bool,
}

/// Opens the picker `dark`-themed over `owner`, showing `recent` (most recent
/// first), and returns the chosen color, or `None` if it was cancelled or the
/// window could not be created. Blocks in a modal loop with the owner
/// disabled, so the owner waits.
pub(crate) unsafe fn pick(
    owner: HWND,
    initial: Color,
    dark: bool,
    recent: &[Color],
) -> Option<Color> {
    if let Err(error) = ensure_class() {
        tracing::warn!(%error, "could not register the color picker class");
        return None;
    }
    let accent = system_accent_color()
        .map(|accent| accent.with_alpha(255))
        .unwrap_or(UiColors::new(dark).accent);
    let mut picker = Box::new(Picker {
        state: PickerState::new(initial, recent, dark, accent),
        painter: Some(ColorPickerPainter::default()),
        tracking_leave: false,
    });
    let scale = dpi::scale_for(dpi::window_dpi(owner));
    picker.state.scale = scale;
    let size = scaled(picker.state.layout.size, scale);
    let (x, y) = picker_position(size);
    let pointer = &mut *picker as *mut Picker;

    // SAFETY: Win32 window creation on the UI thread. `picker` is owned here
    // and dropped after the loop; the window points at it only while it is
    // alive.
    unsafe {
        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!("Color Picker"),
            WS_POPUP,
            x,
            y,
            size.0,
            size.1,
            Some(owner),
            None,
            Some(window_class::instance().ok()?),
            Some(pointer as *const core::ffi::c_void),
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                tracing::warn!(%error, "could not create the color picker window");
                return None;
            }
        };
        let acrylic = dwm::apply_popup_backdrop(hwnd, dark);
        with_picker(hwnd, |picker| picker.state.acrylic = acrylic);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = EnableWindow(owner, false);
        let blink = GetCaretBlinkTime();
        if blink != 0 && blink != NO_BLINK {
            SetTimer(Some(hwnd), CARET_TIMER, blink, None);
        }

        let mut message = MSG::default();
        while IsWindow(Some(hwnd)).as_bool() {
            // Pump every message, not just the picker's: the owner is disabled,
            // so it cannot be driven, but a thread that stops retrieving its
            // own windows' messages is marked "not responding" by Windows.
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 {
                // Another window asked to quit; hand it back to the outer loop.
                PostQuitMessage(message.wParam.0 as i32);
                break;
            }
            if status.0 == -1 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = EnableWindow(owner, true);
    }
    picker.state.result
}

fn scaled(size: (i32, i32), scale: f32) -> (i32, i32) {
    let scale = |value: i32| (value as f32 * scale).round() as i32;
    (scale(size.0), scale(size.1))
}

/// Places the picker at the cursor, kept inside that monitor's work area.
fn picker_position(size: (i32, i32)) -> (i32, i32) {
    let mut cursor = POINT::default();
    // SAFETY: `cursor` is a valid out-pointer.
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let work = work_area_at(Point::new(cursor.x, cursor.y));
    (
        cursor
            .x
            .clamp(work.x, (work.x + work.width - size.0).max(work.x)),
        cursor
            .y
            .clamp(work.y, (work.y + work.height - size.1).max(work.y)),
    )
}

fn ensure_class() -> Result<(), PlatformError> {
    window_class::register(
        WindowClass::new(CLASS_NAME, Some(wndproc))
            .style(CS_DBLCLKS)
            .arrow_cursor(),
    )
}

/// Runs `f` with a short-lived borrow of the window's picker. `f` must not
/// call anything that can re-enter `wndproc`.
unsafe fn with_picker<R>(window: HWND, f: impl FnOnce(&mut Picker) -> R) -> Option<R> {
    // SAFETY: picker windows hold the `Picker` `pick` owns for as long as
    // the window exists; the caller rules out re-entry.
    unsafe { window_state::with_state(window, f) }
}

/// The client point from a mouse message's `lparam`, in physical pixels.
fn mouse_point(lparam: LPARAM) -> (f32, f32) {
    (loword(lparam.0) as f32, hiword(lparam.0) as f32)
}

/// The cursor's position in `window`'s client area.
unsafe fn cursor_in(window: HWND) -> (f32, f32) {
    let mut point = POINT::default();
    // SAFETY: `point` is a valid out-pointer and `window` is live.
    unsafe {
        let _ = GetCursorPos(&mut point);
        let _ = ScreenToClient(window, &mut point);
    }
    (point.x as f32, point.y as f32)
}

/// The color of the screen pixel under the cursor.
fn color_under_cursor() -> Option<Color> {
    let mut point = POINT::default();
    // SAFETY: the screen DC is released before returning; `point` is a valid
    // out-pointer.
    let value = unsafe {
        GetCursorPos(&mut point).ok()?;
        let screen = GetDC(None);
        let value: COLORREF = GetPixel(screen, point.x, point.y);
        ReleaseDC(None, screen);
        value.0
    };
    // COLORREF is 0x00BBGGRR.
    (value != CLR_INVALID).then(|| {
        Color::rgb(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        )
    })
}

fn set_cursor(name: PCWSTR) {
    // SAFETY: loading a shared system cursor.
    unsafe {
        if let Ok(cursor) = LoadCursorW(None, name) {
            SetCursor(Some(cursor));
        }
    }
}

fn key_down(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    // SAFETY: reads the calling thread's key state.
    unsafe { GetKeyState(key.0 as i32) < 0 }
}

unsafe fn invalidate(window: HWND) {
    // SAFETY: `window` is live.
    unsafe {
        let _ = InvalidateRect(Some(window), None, false);
    }
}

/// Asks for `WM_MOUSELEAVE`, so hover clears when the pointer leaves.
unsafe fn track_leave(window: HWND) {
    // SAFETY: a short borrow; TrackMouseEvent sends no message.
    unsafe {
        if with_picker(window, |picker| {
            std::mem::replace(&mut picker.tracking_leave, true)
        }) == Some(false)
        {
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
        }
    }
}

/// Ends the eyedropper, keeping the sampled color or going back to the one
/// before, and lets the mouse go.
unsafe fn stop_sampling(window: HWND, keep: bool) {
    // SAFETY: the state borrow ends before ReleaseCapture, which sends
    // WM_CAPTURECHANGED; by then sampling is over, so that message is a no-op.
    unsafe {
        with_picker(window, |picker| picker.state.end_sampling(keep));
        let _ = ReleaseCapture();
        invalidate(window);
    }
}

/// Carries out what an input asked for. `Done` destroys the window, so call
/// this last in a message branch.
unsafe fn act(window: HWND, outcome: Outcome) {
    // SAFETY: `window` is live; no state borrow is held across these calls.
    unsafe {
        match outcome {
            Outcome::Nothing => {}
            Outcome::Redraw => invalidate(window),
            Outcome::Done => {
                let _ = DestroyWindow(window);
            }
            Outcome::Sample => {
                with_picker(window, |picker| picker.state.start_sampling());
                SetCapture(window);
                set_cursor(IDC_CROSS);
                invalidate(window);
            }
            Outcome::Copy(text) => {
                if let Err(error) = clipboard::set_text(window, &text) {
                    tracing::warn!(%error, "could not copy the color value");
                }
                invalidate(window);
            }
        }
    }
}

unsafe fn paint(window: HWND) {
    // SAFETY: the painter is taken out of the state, so painting holds no
    // borrow of it.
    unsafe {
        let taken = with_picker(window, |picker| {
            let state = &picker.state;
            picker
                .painter
                .take()
                .map(|painter| (painter, state.view(), state.layout, state.scale))
        })
        .flatten();
        let Some((mut painter, view, layout, scale)) = taken else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        if let Err(error) = painter.paint(window, &view, &layout, scale) {
            tracing::debug!(%error, "color picker paint failed");
        }
        with_picker(window, |picker| picker.painter = Some(painter));
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; every state
    // access is a short `with_picker` borrow that does not call back into here.
    unsafe {
        let sampling = || with_picker(window, |picker| picker.state.is_sampling()).unwrap_or(false);
        match message {
            WM_NCCREATE => {
                window_state::attach_create_param::<Picker>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            // The title strip drags the window.
            WM_NCHITTEST => {
                let (x, y) = cursor_in(window);
                let region = with_picker(window, |picker| picker.state.region_at(x, y)).flatten();
                if region == Some(Region::Title) && !sampling() {
                    LRESULT(HTCAPTION as isize)
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                }
            }
            // A double-click on the title would maximize the pop-up.
            WM_NCLBUTTONDBLCLK => LRESULT(0),
            WM_SETCURSOR if loword(lparam.0) as u32 == HTCLIENT => {
                let (x, y) = cursor_in(window);
                let over_field = with_picker(window, |picker| {
                    picker.state.region_at(x, y) == Some(Region::Field)
                }) == Some(true);
                set_cursor(if sampling() {
                    IDC_CROSS
                } else if over_field {
                    IDC_IBEAM
                } else {
                    IDC_ARROW
                });
                LRESULT(1)
            }
            // The whole client is painted in `WM_PAINT`.
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if sampling() {
                    set_cursor(IDC_CROSS);
                    if let Some(color) = color_under_cursor() {
                        with_picker(window, |picker| picker.state.sample(color));
                        invalidate(window);
                    }
                    return LRESULT(0);
                }
                track_leave(window);
                let (x, y) = mouse_point(lparam);
                let outcome = with_picker(window, |picker| picker.state.moved(x, y));
                act(window, outcome.unwrap_or(Outcome::Nothing));
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                let outcome = with_picker(window, |picker| {
                    picker.tracking_leave = false;
                    picker.state.left()
                });
                act(window, outcome.unwrap_or(Outcome::Nothing));
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                // The eyedropper picks on release, so the press goes nowhere
                // else while it holds the mouse.
                if sampling() {
                    return LRESULT(0);
                }
                SetCapture(window);
                let (x, y) = mouse_point(lparam);
                let outcome = with_picker(window, |picker| {
                    if message == WM_LBUTTONDBLCLK {
                        picker.state.double_clicked(x, y)
                    } else {
                        picker.state.pressed(x, y)
                    }
                });
                if outcome == Some(Outcome::Done) {
                    let _ = ReleaseCapture();
                }
                act(window, outcome.unwrap_or(Outcome::Nothing));
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if sampling() {
                    stop_sampling(window, true);
                    return LRESULT(0);
                }
                let _ = ReleaseCapture();
                let (x, y) = mouse_point(lparam);
                let outcome = with_picker(window, |picker| picker.state.released(x, y));
                act(window, outcome.unwrap_or(Outcome::Nothing));
                LRESULT(0)
            }
            WM_RBUTTONDOWN if sampling() => {
                stop_sampling(window, false);
                LRESULT(0)
            }
            // Losing the mouse to another window (Alt+Tab) cancels the
            // eyedropper.
            WM_CAPTURECHANGED => {
                if sampling() && HWND(lparam.0 as *mut core::ffi::c_void) != window {
                    with_picker(window, |picker| picker.state.end_sampling(false));
                    invalidate(window);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let code = wparam.0 as u16;
                if sampling() {
                    if code == state::vk::ESCAPE {
                        stop_sampling(window, false);
                    }
                    return LRESULT(0);
                }
                let Some(key) = picker_key(code, key_down(VK_CONTROL), key_down(VK_SHIFT)) else {
                    return LRESULT(0);
                };
                let outcome = if key == PickerKey::Paste {
                    let text = clipboard::text();
                    with_picker(window, |picker| {
                        if let Some(text) = text {
                            picker.state.paste(&text);
                        }
                        Outcome::Redraw
                    })
                } else {
                    with_picker(window, |picker| picker.state.key(key))
                };
                act(window, outcome.unwrap_or(Outcome::Nothing));
                LRESULT(0)
            }
            WM_CHAR => {
                let typed = char::from_u32(wparam.0 as u32).filter(|c| !c.is_control());
                if let Some(c) = typed
                    && !sampling()
                {
                    let outcome = with_picker(window, |picker| {
                        picker.state.key(PickerKey::Edit(EditKey::Char(c)))
                    });
                    act(window, outcome.unwrap_or(Outcome::Nothing));
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == CARET_TIMER => {
                if with_picker(window, |picker| picker.state.blink()) == Some(true) {
                    invalidate(window);
                }
                LRESULT(0)
            }
            // Moved to a monitor with another scale: take the size Windows
            // suggests and draw at the new scale.
            // https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged
            WM_DPICHANGED => {
                let scale = dpi::scale_for(hiword(wparam.0 as isize) as u32);
                with_picker(window, |picker| picker.state.scale = scale);
                let suggested = *(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    window,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                invalidate(window);
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(window);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = KillTimer(Some(window), CARET_TIMER);
                // Re-enabled before the pop-up goes, Windows hands the
                // activation back to the owner instead of another app.
                if let Ok(owner) = GetWindow(window, GW_OWNER) {
                    let _ = EnableWindow(owner, true);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                window_state::detach(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
