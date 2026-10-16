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
//! Window state lives behind a raw `*mut SpaceState` in `GWLP_USERDATA` and
//! is freed exactly once, in `WM_NCDESTROY`.

use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, InvalidateRect, MapWindowPoints, SetWindowRgn, ValidateRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_DBLCLKS, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetClientRect,
    GetParent, GetWindowLongPtrW, GetWindowRect, HTCLIENT, HWND_BOTTOM, HWND_TOPMOST, IDC_ARROW,
    IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE,
    RegisterClassW, SC_MINIMIZE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetCursor, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WINDOW_EX_STYLE, WINDOWPOS, WM_CAPTURECHANGED, WM_DESTROY, WM_DPICHANGED,
    WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WM_SYSCOMMAND,
    WM_WINDOWPOSCHANGING, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, MouseButton, SpaceEvent};
use crate::render::{PaintFrame, Painter, PreparedScene, SpaceScene, SpaceStyle};
use crate::ui::CursorShape;
use crate::win32::system::{hiword, loword};
use crate::win32::{drop_target, dwm, runtime};

const CLASS_NAME: PCWSTR = w!("DeskSpaceSpace");

/// Mouse-button bits of the `wParam` key state on mouse messages. Defined here
/// to avoid pulling in `Win32_System_SystemServices` for three constants.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;
const MK_ANY_BUTTON: usize = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

static REGISTER_CLASS: Once = Once::new();
static CLASS_ATOM: AtomicU32 = AtomicU32::new(0);

pub(crate) struct SpaceWindowOptions {
    pub id: u64,
    pub rect: Rect,
    pub style: SpaceStyle,
    pub host: Option<HWND>,
    pub click_through: bool,
    pub on_top: bool,
    pub visible: bool,
    /// Whether to register an OLE drop target. The desktop layer skips it: it
    /// is display-only, and a drop target that does not move the files would
    /// still tell a shell source the drop was performed.
    pub register_drop: bool,
}

struct SpaceState {
    id: u64,
    hosted: bool,
    /// Development placement above other windows instead of at the bottom.
    on_top: bool,
    /// The style last applied, so re-applying an unchanged one (every space
    /// on every settings edit) skips the DWM calls and region rebuild.
    style: SpaceStyle,
    frame: Frame,
    /// `None` while `WM_PAINT` has it checked out.
    painter: Option<Painter>,
    scene: Option<PreparedScene>,
    tracking_leave: bool,
    cursor: CursorShape,
}

pub(crate) fn create(options: SpaceWindowOptions) -> Result<HWND, PlatformError> {
    let hosted = options.host.is_some();
    let state = Box::into_raw(Box::new(SpaceState {
        id: options.id,
        hosted,
        on_top: options.on_top,
        style: options.style,
        frame: Frame::for_style(options.style, hosted),
        painter: Some(Painter::new()),
        scene: None,
        tracking_leave: false,
        cursor: CursorShape::Arrow,
    }));

    // An OLE drop target (registered below) makes the space accept file drags
    // and choose their effect; WS_EX_ACCEPTFILES and WM_DROPFILES are not used.
    let mut ex_style: WINDOW_EX_STYLE = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
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
        update_region(hwnd);
        if !hosted {
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
        if options.register_drop
            && let Err(error) = drop_target::register(hwnd, options.id, hosted)
        {
            tracing::warn!(%error, "space will not accept dropped files");
        }
        Ok(hwnd)
    }
}

pub(crate) fn set_rect(hwnd: HWND, rect: Rect) {
    if current_rect(hwnd) == Some(rect) {
        return;
    }
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

/// Sends a window to the bottom of the z-order without moving or resizing it.
/// The desktop layer uses this after a top-level space is created so the
/// full-screen layer stays behind the spaces (both pin to the bottom).
pub(crate) fn lower(hwnd: HWND) {
    // SAFETY: `hwnd` is a live window. SWP_NOZORDER is not set, so the z-order
    // change applies; the window procedure then pins it to the bottom.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// The window's rect in the coordinates `set_rect` takes: the parent's client
/// coordinates for a hosted child (Progman/WorkerW spans the desktop), screen
/// coordinates otherwise.
fn current_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is a live space window; neither call sends messages.
    unsafe {
        GetWindowRect(hwnd, &mut rect).ok()?;
        if let Ok(parent) = GetParent(hwnd) {
            let mut corners = [
                POINT {
                    x: rect.left,
                    y: rect.top,
                },
                POINT {
                    x: rect.right,
                    y: rect.bottom,
                },
            ];
            MapWindowPoints(None, Some(parent), &mut corners);
            rect = RECT {
                left: corners[0].x,
                top: corners[0].y,
                right: corners[1].x,
                bottom: corners[1].y,
            };
        }
    }
    Some(Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ))
}

pub(crate) fn set_scene(hwnd: HWND, scene: SpaceScene) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let unchanged = unsafe {
        with_state(hwnd, |state| {
            state
                .scene
                .as_ref()
                .is_some_and(|current| *current.scene() == scene)
        })
    };
    if unchanged == Some(true) {
        return;
    }
    let prepared = PreparedScene::new(scene);
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    unsafe {
        with_state(hwnd, |state| {
            if let Some(painter) = state.painter.as_mut() {
                painter.retain_icons(prepared.scene());
            }
            state.scene = Some(prepared);
        });
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

/// Sets the pointer shown over the space. The app calls this while handling
/// a mouse event on the space, so the cursor is over it (or captured by it)
/// and is updated at once. WM_SETCURSOR is not sent while the mouse is
/// captured, so waiting for it would leave the old shape during a drag.
pub(crate) fn set_cursor(hwnd: HWND, shape: CursorShape) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let changed = unsafe {
        with_state(hwnd, |state| {
            std::mem::replace(&mut state.cursor, shape) != shape
        })
    };
    if changed == Some(true) {
        apply_cursor(shape);
    }
}

fn apply_cursor(shape: CursorShape) {
    let id = match shape {
        CursorShape::Arrow => IDC_ARROW,
        CursorShape::SizeWestEast => IDC_SIZEWE,
        CursorShape::SizeNorthSouth => IDC_SIZENS,
        CursorShape::SizeNorthWestSouthEast => IDC_SIZENWSE,
        CursorShape::SizeNorthEastSouthWest => IDC_SIZENESW,
    };
    // SAFETY: stock cursors are shared system resources and are never freed.
    unsafe {
        if let Ok(cursor) = LoadCursorW(None, id) {
            SetCursor(Some(cursor));
        }
    }
}

pub(crate) fn set_style(hwnd: HWND, style: SpaceStyle) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let Some(Some(hosted)) = (unsafe {
        with_state(hwnd, |state| {
            if state.style == style {
                return None;
            }
            state.style = style;
            state.frame = Frame::for_style(style, state.hosted);
            Some(state.hosted)
        })
    }) else {
        return;
    };
    if !hosted {
        dwm::apply_style(hwnd, style);
    }
    // SAFETY: `hwnd` is a live space window.
    unsafe {
        update_region(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

/// Who draws a space's corners and border, and how its window is clipped.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    paint: PaintFrame,
    /// Clip the window to a rounded region of this radius. Needed for opaque
    /// desktop-hosted windows, whose pixels outside the rounded corners
    /// would otherwise show. Regions are not anti-aliased, so the painter
    /// still draws smooth corners inside them. Blurred top-level spaces use
    /// DWM's corners instead, because the blur material ignores regions
    /// (`Theme::effective_corner_radius`).
    region_radius: Option<i32>,
}

impl Frame {
    fn for_style(style: SpaceStyle, hosted: bool) -> Self {
        // A chrome-less window (the desktop layer) has no rounded corners or
        // border; it fills its whole rectangle.
        let radius = if style.chrome {
            style.corner_radius.max(0)
        } else {
            0
        };
        if !hosted && style.dwm_draws_frame() {
            return Self {
                paint: PaintFrame {
                    opaque: false,
                    radius: 0,
                    border: false,
                },
                region_radius: None,
            };
        }
        Self {
            paint: PaintFrame {
                opaque: hosted,
                radius,
                border: style.chrome && style.border,
            },
            region_radius: (radius > 0 && hosted).then_some(radius),
        }
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

/// Applies the frame's rounded window region, or removes it.
unsafe fn update_region(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let Some(radius) = (unsafe { with_state(hwnd, |state| state.frame.region_radius) }) else {
        return;
    };
    let mut client = Default::default();
    // SAFETY: `hwnd` is live; the system owns the region after SetWindowRgn.
    unsafe {
        let Some(radius) = radius else {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        };
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

unsafe fn state_of(window: HWND) -> *mut SpaceState {
    // SAFETY: reading the user-data slot of a window of our class.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut SpaceState }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut SpaceState) -> R) -> Option<R> {
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

unsafe fn paint(window: HWND) {
    // SAFETY: for every call, `window` is live and the state borrows are
    // short.
    unsafe {
        let checked_out = with_state(window, |state| {
            (state.painter.take(), state.scene.take(), state.frame.paint)
        });
        let Some((Some(mut painter), scene, frame)) = checked_out else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        if let Err(error) = painter.paint(window, scene.as_ref(), frame) {
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
        let space_event = |event: SpaceEvent| {
            if let Some(id) = with_state(window, |state| state.id) {
                runtime::dispatch(Event::Space { id, event });
            }
        };
        let hosted = || with_state(window, |state| state.hosted).unwrap_or(false);

        match message {
            WM_NCCREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as *mut SpaceState;
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
                update_region(window);
                let _ = InvalidateRect(Some(window), None, false);
                LRESULT(0)
            }
            WM_SYSCOMMAND if (wparam.0 as u32 & 0xFFF0) == SC_MINIMIZE => {
                // A space is part of the desktop and must stay on it: Show
                // Desktop (`MinimizeAll`) minimizes every normal window, and a
                // space has to survive with its blur. `WS_EX_TOOLWINDOW` makes
                // the shell skip tool windows, and this ignores the command if
                // one is sent directly, so survival never depends on that
                // undocumented exclusion. Minimizing would also tear down the
                // accent blur, which only survives while the window does.
                // https://learn.microsoft.com/en-us/windows/win32/menurc/wm-syscommand
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // The suggested rectangle in lParam is deliberately ignored:
                // DeskSpace owns a space's rectangle in the model and reflows
                // it for the new DPI, so applying the system's default would
                // fight the model. LOWORD(wParam) is the new horizontal DPI.
                // https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
                let dpi = loword(wparam.0 as isize).max(0) as u32;
                space_event(SpaceEvent::DpiChanged { dpi });
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
                space_event(SpaceEvent::MouseMove { x, y });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with_state(window, |state| {
                    state.tracking_leave = false;
                    state.cursor = CursorShape::Arrow;
                });
                space_event(SpaceEvent::MouseLeave);
                LRESULT(0)
            }
            WM_SETCURSOR if loword(lparam.0) == HTCLIENT as i32 => {
                let shape = with_state(window, |state| state.cursor).unwrap_or_default();
                apply_cursor(shape);
                LRESULT(1)
            }
            WM_MOUSEWHEEL => {
                // Unlike other mouse messages, the wheel position is in screen
                // coordinates, and the delta is the signed high word of wParam
                // (GET_WHEEL_DELTA_WPARAM).
                // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
                let delta = hiword(wparam.0 as isize);
                let mut point = [POINT {
                    x: loword(lparam.0),
                    y: hiword(lparam.0),
                }];
                if hosted() {
                    MapWindowPoints(None, GetParent(window).ok(), &mut point);
                }
                // SAFETY: GetKeyState reads this thread's input queue state,
                // which matches the message being processed.
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                space_event(SpaceEvent::Wheel {
                    x: point[0].x,
                    y: point[0].y,
                    delta,
                    ctrl,
                });
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDBLCLK => {
                // Capture so the matching button-up arrives even if the cursor
                // leaves the window mid-drag.
                // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input#mouse-capture
                SetCapture(window);
                let (x, y) = to_desktop(window, lparam, hosted());
                // SAFETY: GetKeyState reads this thread's input queue state,
                // which matches the message being processed.
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                space_event(SpaceEvent::MouseDown {
                    x,
                    y,
                    button: button_for(message),
                    ctrl,
                    shift,
                });
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                let (x, y) = to_desktop(window, lparam, hosted());
                space_event(SpaceEvent::MouseUp {
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
                space_event(SpaceEvent::DoubleClick { x, y });
                LRESULT(0)
            }
            WM_DESTROY => {
                // Revoke while the window still exists so OLE releases its
                // reference to the drop target before the HWND is gone.
                drop_target::revoke(window);
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                // lParam is the window gaining capture. Our own ReleaseCapture
                // after the last button-up also lands here, after MouseUp has
                // been delivered, so a CaptureLost then is harmless.
                if lparam.0 != window.0 as isize {
                    space_event(SpaceEvent::CaptureLost);
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
