//! One window per space.
//!
//! Top-level spaces are `WS_POPUP` windows kept at the bottom of the z-order,
//! styled by DWM (`dwm::apply_style`) with the Windows 11 backdrop and rounded
//! corners, and painted with per-pixel alpha by `render::d2d::Painter`.
//!
//! Desktop-hosted spaces are `WS_CHILD` windows of the desktop's Progman or
//! WorkerW. Child windows get neither DWM backdrops nor composited alpha, so
//! they are painted opaque and rounded with a window region.
//!
//! Window state lives behind a raw `*mut spaceState` in `GWLP_USERDATA` and
//! is freed exactly once, in `WM_NCDESTROY`.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, InvalidateRect, MapWindowPoints, SetWindowRgn, ValidateRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, DragQueryPoint, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_DBLCLKS, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetClientRect,
    GetParent, GetWindowLongPtrW, HWND_BOTTOM, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA, LoadCursorW,
    MA_NOACTIVATE, RegisterClassW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    WINDOW_EX_STYLE, WINDOWPOS, WM_CAPTURECHANGED, WM_DROPFILES, WM_ERASEBKGND, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE,
    WM_WINDOWPOSCHANGING, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_ACCEPTFILES, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, spaceEvent, MouseButton};
use crate::render::{spaceScene, spaceStyle, Painter, PreparedScene};
use crate::win32::system::{hiword, loword};
use crate::win32::{dwm, runtime};

const CLASS_NAME: PCWSTR = w!("DeskSpacespace");

/// Mouse-button bits of the `wParam` key state on mouse messages. Defined here
/// to avoid pulling in `Win32_System_SystemServices` for three constants.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;
const MK_ANY_BUTTON: usize = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

static REGISTER_CLASS: Once = Once::new();
static CLASS_ATOM: AtomicU32 = AtomicU32::new(0);

pub(crate) struct spaceWindowOptions {
    pub id: u64,
    pub rect: Rect,
    pub style: spaceStyle,
    pub host: Option<HWND>,
    pub click_through: bool,
    pub on_top: bool,
    pub visible: bool,
}

struct spaceState {
    id: u64,
    hosted: bool,
    /// Development placement above other windows instead of at the bottom.
    on_top: bool,
    corner_radius: i32,
    /// `None` while `WM_PAINT` has it checked out.
    painter: Option<Painter>,
    scene: Option<PreparedScene>,
    tracking_leave: bool,
}

pub(crate) fn create(options: spaceWindowOptions) -> Result<HWND, PlatformError> {
    let hosted = options.host.is_some();
    let state = Box::into_raw(Box::new(spaceState {
        id: options.id,
        hosted,
        on_top: options.on_top,
        corner_radius: options.style.corners.radius(),
        painter: Some(Painter::new()),
        scene: None,
        tracking_leave: false,
    }));

    // WS_EX_ACCEPTFILES makes the shell deliver WM_DROPFILES for files dragged
    // from Explorer or the desktop; without it the drag shows the 'no' cursor.
    // https://learn.microsoft.com/en-us/windows/win32/shell/wm-dropfiles
    let mut ex_style: WINDOW_EX_STYLE = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_ACCEPTFILES;
    if options.on_top {
        ex_style |= WS_EX_TOPMOST;
    }
    if options.click_through {
        // WS_EX_TRANSPARENT only makes a window click-through when it is also
        // layered; on its own it merely changes paint order.
        // https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles
        ex_style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
    }
    let style = if hosted {
        WS_CHILD | WS_CLIPSIBLINGS
    } else {
        WS_POPUP
    };
    let rect = options.rect;

    // SAFETY: plain Win32 window creation. Ownership of `state` passes to the
    // window in WM_NCCREATE and it is freed in WM_NCDESTROY. If creation fails
    // after WM_NCCREATE, WM_NCDESTROY has already freed it, so it is
    // deliberately not freed here.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        ensure_class(instance)?;
        let hwnd = CreateWindowExW(
            ex_style,
            CLASS_NAME,
            w!("DeskSpace space"),
            style,
            rect.x,
            rect.y,
            rect.width.max(1),
            rect.height.max(1),
            options.host,
            None,
            Some(instance),
            Some(state as *const core::ffi::c_void),
        )?;

        if options.click_through {
            let _ = SetLayeredWindowAttributes(hwnd, Default::default(), 255, LWA_ALPHA);
        }
        if hosted {
            update_region(hwnd, options.style.corners.radius());
        } else {
            dwm::apply_style(hwnd, options.style);
            let _ = SetWindowPos(
                hwnd,
                Some(if options.on_top {
                    HWND_TOPMOST
                } else {
                    HWND_BOTTOM
                }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        if options.visible {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(hwnd)
    }
}

pub(crate) fn set_rect(hwnd: HWND, rect: Rect) {
    // SAFETY: `hwnd` is a live space window. Messages sent synchronously by
    // SetWindowPos (WM_WINDOWPOSCHANGING, WM_SIZE) do not touch the runtime.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width.max(1),
            rect.height.max(1),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub(crate) fn set_scene(hwnd: HWND, scene: spaceScene) {
    let radius = scene.corner_radius;
    let prepared = PreparedScene::new(scene);
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let reshape = unsafe {
        with_state(hwnd, |state| {
            state.scene = Some(prepared);
            let changed = state.corner_radius != radius;
            state.corner_radius = radius;
            state.hosted && changed
        })
    };
    // SAFETY: `hwnd` is a live space window.
    unsafe {
        if reshape == Some(true) {
            update_region(hwnd, radius);
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

pub(crate) fn set_style(hwnd: HWND, style: spaceStyle) {
    // SAFETY: `hwnd` is a live space window.
    let hosted = unsafe { with_state(hwnd, |state| state.hosted) }.unwrap_or(true);
    if !hosted {
        dwm::apply_style(hwnd, style);
    }
    // SAFETY: `hwnd` is a live space window.
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn ensure_class(instance: HINSTANCE) -> Result<(), PlatformError> {
    REGISTER_CLASS.call_once(|| {
        let class = WNDCLASSW {
            // Without CS_DBLCLKS Windows never sends WM_LBUTTONDBLCLK, and a
            // double-click arrives as two WM_LBUTTONDOWNs.
            // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttondblclk
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            // SAFETY: loading a shared stock cursor.
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its strings are static.
        let atom = unsafe { RegisterClassW(&class) };
        CLASS_ATOM.store(atom as u32, Ordering::SeqCst);
    });
    if CLASS_ATOM.load(Ordering::SeqCst) == 0 {
        Err(PlatformError::ClassRegistration)
    } else {
        Ok(())
    }
}

/// Clips a desktop-hosted space to a rounded rectangle. DWM rounds top-level
/// windows itself; child windows need a region (which is not anti-aliased).
unsafe fn update_region(hwnd: HWND, radius: i32) {
    let mut client = Default::default();
    // SAFETY: `hwnd` is live; the system owns the region after SetWindowRgn.
    unsafe {
        if GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
        // CreateRoundRectRgn excludes the right and bottom edges, hence +1.
        let region = CreateRoundRectRgn(
            0,
            0,
            client.right + 1,
            client.bottom + 1,
            radius * 2,
            radius * 2,
        );
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
}

unsafe fn state_of(window: HWND) -> *mut spaceState {
    // SAFETY: reading the user-data slot of a window of our class.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut spaceState }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut spaceState) -> R) -> Option<R> {
    // SAFETY: a non-null pointer is the live state (WM_NCDESTROY clears the
    // slot before freeing), and `f` cannot re-enter to create a second borrow.
    unsafe { state_of(window).as_mut() }.map(f)
}

/// Converts a client point to the space's desktop coordinates: screen
/// coordinates for top-level spaces, the host's client coordinates for hosted
/// ones.
unsafe fn to_desktop(window: HWND, lparam: LPARAM, hosted: bool) -> (i32, i32) {
    let mut point = [POINT {
        x: loword(lparam.0),
        y: hiword(lparam.0),
    }];
    // SAFETY: `window` is live; `point` is a valid one-element buffer.
    unsafe {
        let target = if hosted { GetParent(window).ok() } else { None };
        MapWindowPoints(Some(window), target, &mut point);
    }
    (point[0].x, point[0].y)
}

fn button_for(message: u32) -> MouseButton {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

/// Reads the paths and client drop point out of a WM_DROPFILES handle.
unsafe fn dropped_files(drop: HDROP) -> (Vec<PathBuf>, POINT) {
    // SAFETY: `drop` is the live HDROP from WM_DROPFILES; buffers are sized
    // from DragQueryFileW's reported length plus the NUL.
    unsafe {
        let count = DragQueryFileW(drop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = DragQueryFileW(drop, index, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            let copied = DragQueryFileW(drop, index, Some(&mut buffer)) as usize;
            paths.push(PathBuf::from(OsString::from_wide(&buffer[..copied])));
        }
        let mut point = POINT::default();
        let _ = DragQueryPoint(drop, &mut point);
        (paths, point)
    }
}

unsafe fn paint(window: HWND) {
    // SAFETY (all calls): `window` is live and the state borrows are short.
    unsafe {
        let checked_out = with_state(window, |state| {
            (state.painter.take(), state.scene.take(), state.hosted)
        });
        let Some((Some(mut painter), scene, hosted)) = checked_out else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        if let Err(error) = painter.paint(window, scene.as_ref(), hosted) {
            tracing::error!(%error, "space paint failed");
        }
        with_state(window, |state| {
            state.painter = Some(painter);
            if state.scene.is_none() {
                state.scene = scene;
            }
        });
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
        let space_event = |event: spaceEvent| {
            if let Some(id) = with_state(window, |state| state.id) {
                runtime::dispatch(Event::space { id, event });
            }
        };
        let hosted = || with_state(window, |state| state.hosted).unwrap_or(false);

        match message {
            WM_NCCREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as *mut spaceState;
                    if !state.is_null() {
                        SetWindowLongPtrW(window, GWLP_USERDATA, state as isize);
                    }
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => {
                if let Some((true, radius)) =
                    with_state(window, |state| (state.hosted, state.corner_radius))
                {
                    update_region(window, radius);
                }
                let _ = InvalidateRect(Some(window), None, false);
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGING => {
                // Keep top-level spaces under every other window so they behave
                // like part of the desktop.
                let position = lparam.0 as *mut WINDOWPOS;
                let pinned = with_state(window, |state| !state.hosted && !state.on_top);
                if pinned == Some(true)
                    && !position.is_null()
                    && !(*position).flags.contains(SWP_NOZORDER)
                {
                    (*position).hwndInsertAfter = HWND_BOTTOM;
                }
                LRESULT(0)
            }
            // Clicking a space must not activate it (and pull it above other
            // apps); WS_EX_NOACTIVATE alone does not cover every click path.
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_MOUSEMOVE => {
                let start_tracking = with_state(window, |state| {
                    !std::mem::replace(&mut state.tracking_leave, true)
                });
                if start_tracking == Some(true) {
                    // Ask for WM_MOUSELEAVE so hover highlights clear when the
                    // cursor leaves the space.
                    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackmouseevent
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: window,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut track);
                }
                let (x, y) = to_desktop(window, lparam, hosted());
                space_event(spaceEvent::MouseMove { x, y });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with_state(window, |state| state.tracking_leave = false);
                space_event(spaceEvent::MouseLeave);
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDBLCLK => {
                // Capture so the matching button-up arrives even if the cursor
                // leaves the window mid-drag.
                // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input#mouse-capture
                SetCapture(window);
                let (x, y) = to_desktop(window, lparam, hosted());
                space_event(spaceEvent::MouseDown {
                    x,
                    y,
                    button: button_for(message),
                });
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                let (x, y) = to_desktop(window, lparam, hosted());
                space_event(spaceEvent::MouseUp {
                    x,
                    y,
                    button: button_for(message),
                });
                if wparam.0 & MK_ANY_BUTTON == 0 {
                    let _ = ReleaseCapture();
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let (x, y) = to_desktop(window, lparam, hosted());
                space_event(spaceEvent::DoubleClick { x, y });
                LRESULT(0)
            }
            WM_DROPFILES => {
                let drop = HDROP(wparam.0 as *mut core::ffi::c_void);
                let (paths, point) = dropped_files(drop);
                DragFinish(drop);
                let mut point = [point];
                let target = if hosted() {
                    GetParent(window).ok()
                } else {
                    None
                };
                MapWindowPoints(Some(window), target, &mut point);
                if !paths.is_empty() {
                    space_event(spaceEvent::FilesDropped {
                        x: point[0].x,
                        y: point[0].y,
                        paths,
                    });
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                // lParam is the window gaining capture. Our own ReleaseCapture
                // after the last button-up also lands here, after MouseUp has
                // been delivered, so a CaptureLost then is harmless.
                if lparam.0 != window.0 as isize {
                    space_event(spaceEvent::CaptureLost);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let state = state_of(window);
                if !state.is_null() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    drop(Box::from_raw(state));
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
