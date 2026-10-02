use std::collections::VecDeque;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use deskspace_core::theme::Color;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetClientRect, GetMessageW, GetWindowLongPtrW, IsWindow, LWA_ALPHA,
    LWA_COLORKEY, MSG, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    WM_CAPTURECHANGED, WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_ENDSESSION, WM_ERASEBKGND,
    WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE,
    WM_TIMER, WNDCLASSW, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{MouseButton, WindowEvent};
use crate::render::{Painter, Scene};
use crate::win32::{console, desktop_shell, dpi};

const CLASS_NAME: PCWSTR = w!("DeskSpaceOverlay");
const HOTKEY_BASE_ID: i32 = 0x100;
const TIMER_ID: usize = 1;

/// Mouse-button bits of the `wParam` key state on mouse messages. Defined here
/// to avoid pulling in `Win32_System_SystemServices` for three constants.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;
const MK_ANY_BUTTON: usize = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

static REGISTER_CLASS: Once = Once::new();
static CLASS_ATOM: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub placement: desktop_shell::Placement,
    pub click_through: bool,
    pub color_key: Color,
    pub visible: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "DeskSpace".to_string(),
            x: 80,
            y: 80,
            width: 1280,
            height: 800,
            placement: desktop_shell::Placement::TopLevel,
            click_through: false,
            color_key: Color::rgb(255, 0, 255),
            visible: true,
        }
    }
}

pub trait EventHandler {
    fn on_event(&mut self, event: WindowEvent, ctx: &mut WindowContext<'_>);
}

pub struct WindowContext<'a> {
    hwnd: HWND,
    painter: &'a mut Painter,
    width: i32,
    height: i32,
    color_key: Color,
}

impl<'a> WindowContext<'a> {
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn color_key(&self) -> Color {
        self.color_key
    }

    pub fn paint(&mut self, scene: &Scene) -> Result<(), PlatformError> {
        self.painter.draw(self.hwnd, scene)
    }

    pub fn request_repaint(&self) {
        // SAFETY: `hwnd` is this context's live window.
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    pub fn show(&self, visible: bool) {
        // SAFETY: `hwnd` is this context's live window. Messages sent
        // synchronously by ShowWindow are queued by `dispatch`.
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

struct WindowState {
    hwnd: HWND,
    color_key: Color,
    width: i32,
    height: i32,
    dpi: u32,
    /// `None` while `dispatch` has the handler checked out.
    handler: Option<Box<dyn EventHandler>>,
    painter: Option<Painter>,
    /// Events raised re-entrantly while the handler is running.
    pending: VecDeque<WindowEvent>,
    /// A `WM_PAINT` arrived while the handler was running and was postponed.
    repaint_deferred: bool,
    /// `WM_NCDESTROY` arrived while the handler was running; the outermost
    /// `dispatch` frees the state once the handler is back.
    free_after_dispatch: bool,
}

impl WindowState {
    fn is_dispatching(&self) -> bool {
        self.handler.is_none()
    }
}

pub struct DesktopWindow {
    hwnd: HWND,
    visible: bool,
}

impl DesktopWindow {
    pub fn create(
        config: WindowConfig,
        handler: Box<dyn EventHandler>,
    ) -> Result<Self, PlatformError> {
        // SAFETY: plain Win32 window creation. Ownership of `state` passes to
        // the window in WM_NCCREATE and it is freed in WM_NCDESTROY.
        unsafe {
            let module = GetModuleHandleW(None)?;
            let instance = HINSTANCE(module.0);
            ensure_class(instance)?;

            let host = desktop_shell::find_host(config.placement);

            let state = Box::into_raw(Box::new(WindowState {
                hwnd: HWND::default(),
                color_key: config.color_key,
                width: config.width,
                height: config.height,
                dpi: 96,
                handler: Some(handler),
                painter: Some(Painter::new()),
                pending: VecDeque::new(),
                repaint_deferred: false,
                free_after_dispatch: false,
            }));

            let mut ex_style = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let (style, layered) = if host.is_some() {
                (WS_CHILD | WS_VISIBLE, false)
            } else {
                ex_style |= WS_EX_LAYERED;
                if config.click_through {
                    ex_style |= WS_EX_TRANSPARENT;
                }
                (WS_POPUP, true)
            };

            let title: Vec<u16> = config
                .title
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // On failure the state is deliberately leaked: if WM_NCCREATE ran,
            // WM_NCDESTROY has already freed it, and freeing again here would be
            // a double free. Creation failure is fatal to the app anyway.
            let hwnd = CreateWindowExW(
                ex_style,
                CLASS_NAME,
                PCWSTR(title.as_ptr()),
                style,
                config.x,
                config.y,
                config.width,
                config.height,
                host,
                None,
                Some(instance),
                Some(state as *const core::ffi::c_void),
            )?;

            if layered {
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    colorref(config.color_key),
                    255,
                    LWA_COLORKEY | LWA_ALPHA,
                );
            }

            let _ = ShowWindow(hwnd, if config.visible { SW_SHOW } else { SW_HIDE });
            let _ = UpdateWindow(hwnd);

            Ok(Self {
                hwnd,
                visible: config.visible,
            })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        // SAFETY: `hwnd` is owned by this DesktopWindow.
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    pub fn toggle_visible(&mut self) {
        self.set_visible(!self.visible);
    }

    pub fn invalidate(&self) {
        // SAFETY: `hwnd` is owned by this DesktopWindow.
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    /// Pumps messages until the window is destroyed. Console Ctrl+C, Ctrl+Break
    /// and console close are turned into `WM_CLOSE` while this runs, so the
    /// handler sees `CloseRequested` and can save.
    pub fn run(self) -> Result<(), PlatformError> {
        console::install(self.hwnd);
        let mut message = MSG::default();
        let result = loop {
            // SAFETY: `message` is a valid out-pointer for the whole loop.
            let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
            // GetMessageW returns -1 on error, which `as_bool` would treat as
            // "keep going" and spin forever.
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagew
            match status.0 {
                0 => break Ok(()),
                -1 => break Err(PlatformError::Win32(windows::core::Error::from_thread())),
                _ => {
                    // SAFETY: `message` was just filled in by GetMessageW.
                    unsafe {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        };
        console::message_loop_finished();
        result
    }
}

impl Drop for DesktopWindow {
    fn drop(&mut self) {
        // SAFETY: IsWindow guards against destroying an already-destroyed HWND.
        unsafe {
            if IsWindow(Some(self.hwnd)).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
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
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: Default::default(),
            hCursor: Default::default(),
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: CLASS_NAME,
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

/// Returns the window's `WindowState`, stored as a raw pointer in
/// `GWLP_USERDATA` by WM_NCCREATE. Null before WM_NCCREATE and after
/// WM_NCDESTROY.
unsafe fn state_of(window: HWND) -> *mut WindowState {
    // SAFETY: reading the user-data slot of a window of our class.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut WindowState }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut WindowState) -> R) -> Option<R> {
    // SAFETY: a non-null pointer is the live state (WM_NCDESTROY clears the
    // slot before freeing). Nothing else holds a reference to it: `dispatch`
    // only keeps the raw pointer while the handler runs, and `f` cannot
    // re-enter.
    unsafe { state_of(window).as_mut() }.map(f)
}

/// Delivers `event` to the handler.
///
/// Win32 calls made by the handler (ShowWindow, SetWindowPos, DestroyWindow,
/// modal menu loops) re-enter `wndproc` synchronously. Holding a `&mut
/// WindowState` across the handler call would let the nested call create a
/// second, aliasing `&mut`. So the handler and painter are moved out of the
/// state for the duration of the call; nested events are queued in `pending`
/// and delivered by this outermost call once the handler returns.
unsafe fn dispatch(window: HWND, event: WindowEvent) {
    // SAFETY (all derefs of `state` below): `state` is live, because
    // WM_NCDESTROY defers freeing while `is_dispatching`. Each borrow ends
    // before any call that could re-enter `wndproc`.
    let state = unsafe { state_of(window) };
    if state.is_null() {
        return;
    }
    let checked_out = unsafe {
        let state = &mut *state;
        state.pending.push_back(event);
        match (state.handler.take(), state.painter.take()) {
            (Some(handler), Some(painter)) => Some((handler, painter)),
            (handler, painter) => {
                state.handler = handler;
                state.painter = painter;
                None
            }
        }
    };
    let Some((mut handler, mut painter)) = checked_out else {
        return;
    };

    loop {
        let next = unsafe {
            let state = &mut *state;
            state.pending.pop_front().map(|event| {
                (
                    event,
                    state.hwnd,
                    state.width,
                    state.height,
                    state.color_key,
                )
            })
        };
        let Some((event, hwnd, width, height, color_key)) = next else {
            break;
        };
        let is_paint = matches!(event, WindowEvent::Paint);
        if is_paint {
            painter.reset_paint();
        }
        let mut ctx = WindowContext {
            hwnd,
            painter: &mut painter,
            width,
            height,
            color_key,
        };
        handler.on_event(event, &mut ctx);
        if is_paint && !painter.was_painted() {
            // SAFETY: an unpainted WM_PAINT must still validate, or Windows
            // keeps resending it.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
        }
    }

    let (free, hwnd, repaint) = unsafe {
        let state = &mut *state;
        state.handler = Some(handler);
        state.painter = Some(painter);
        let repaint = std::mem::take(&mut state.repaint_deferred);
        (state.free_after_dispatch, state.hwnd, repaint)
    };
    if free {
        // SAFETY: WM_NCDESTROY cleared GWLP_USERDATA and left the free to us;
        // nothing else can reach the pointer any more.
        drop(unsafe { Box::from_raw(state) });
    } else if repaint {
        // SAFETY: the window is alive (not yet destroyed).
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

unsafe fn client_size(window: HWND) -> (i32, i32) {
    let mut rect = RECT::default();
    if unsafe { GetClientRect(window, &mut rect) }.is_ok() {
        (rect.right - rect.left, rect.bottom - rect.top)
    } else {
        (0, 0)
    }
}

/// Equivalent of GET_X_LPARAM / GET_Y_LPARAM: the `as i16` sign-extends, so
/// coordinates left of or above the window (possible while the mouse is
/// captured, and on multi-monitor setups) stay negative. LOWORD/HIWORD would
/// turn them into large positive values.
/// https://learn.microsoft.com/en-us/windows/win32/api/windowsx/nf-windowsx-get_x_lparam
fn loword(value: isize) -> i32 {
    (value & 0xFFFF) as i16 as i32
}

fn hiword(value: isize) -> i32 {
    ((value >> 16) & 0xFFFF) as i16 as i32
}

fn button_for(message: u32) -> MouseButton {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; state
    // access follows the rules on `with_state` and `dispatch`.
    unsafe {
        match message {
            WM_NCCREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as *mut WindowState;
                    if !state.is_null() {
                        (*state).hwnd = window;
                        SetWindowLongPtrW(window, GWLP_USERDATA, state as isize);
                    }
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_CREATE => {
                let (width, height) = client_size(window);
                let dpi = dpi::window_dpi(window);
                let created = with_state(window, |state| {
                    state.dpi = dpi;
                    if width > 0 && height > 0 {
                        state.width = width;
                        state.height = height;
                    }
                    WindowEvent::Created {
                        width: state.width,
                        height: state.height,
                        dpi,
                    }
                });
                if let Some(event) = created {
                    dispatch(window, event);
                }
                LRESULT(0)
            }
            WM_SIZE => {
                let width = loword(lparam.0);
                let height = hiword(lparam.0);
                with_state(window, |state| {
                    state.width = width;
                    state.height = height;
                });
                dispatch(window, WindowEvent::Resized { width, height });
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xFFFF) as u32;
                with_state(window, |state| state.dpi = dpi);
                let suggested = lparam.0 as *const RECT;
                if !suggested.is_null() {
                    let rect = *suggested;
                    let _ = SetWindowPos(
                        window,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        Default::default(),
                    );
                }
                dispatch(window, WindowEvent::DpiChanged { dpi });
                LRESULT(0)
            }
            WM_PAINT => {
                // A WM_PAINT nested inside the handler (e.g. from a modal loop)
                // cannot be painted now, because BeginPaint belongs to this
                // message. Validate it so it is not resent in a tight loop, and
                // repaint once the handler returns.
                let busy = with_state(window, |state| {
                    let busy = state.is_dispatching();
                    if busy {
                        state.repaint_deferred = true;
                    }
                    busy
                });
                match busy {
                    Some(false) => dispatch(window, WindowEvent::Paint),
                    _ => {
                        let _ = ValidateRect(Some(window), None);
                    }
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                dispatch(
                    window,
                    WindowEvent::MouseMove {
                        x: loword(lparam.0),
                        y: hiword(lparam.0),
                    },
                );
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                // Capture so the matching button-up arrives even if the cursor
                // leaves the window mid-drag.
                // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input#mouse-capture
                SetCapture(window);
                dispatch(
                    window,
                    WindowEvent::MouseDown {
                        x: loword(lparam.0),
                        y: hiword(lparam.0),
                        button: button_for(message),
                    },
                );
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                dispatch(
                    window,
                    WindowEvent::MouseUp {
                        x: loword(lparam.0),
                        y: hiword(lparam.0),
                        button: button_for(message),
                    },
                );
                if wparam.0 & MK_ANY_BUTTON == 0 {
                    let _ = ReleaseCapture();
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                // lParam is the window gaining capture. Our own ReleaseCapture
                // after the last button-up also lands here, after MouseUp has
                // been delivered, so a CaptureLost then is harmless.
                if lparam.0 != window.0 as isize {
                    dispatch(window, WindowEvent::CaptureLost);
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                dispatch(
                    window,
                    WindowEvent::DoubleClick {
                        x: loword(lparam.0),
                        y: hiword(lparam.0),
                    },
                );
                LRESULT(0)
            }
            WM_HOTKEY => {
                dispatch(
                    window,
                    WindowEvent::Hotkey {
                        id: wparam.0 as i32,
                    },
                );
                LRESULT(0)
            }
            WM_TIMER => {
                dispatch(window, WindowEvent::Timer { id: wparam.0 });
                LRESULT(0)
            }
            WM_ENDSESSION => {
                // At logoff/shutdown the process may be terminated once this
                // returns, without WM_DESTROY, so save now. Only top-level
                // windows receive this message.
                // https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-endsession
                if wparam.0 != 0 {
                    dispatch(window, WindowEvent::CloseRequested);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                dispatch(window, WindowEvent::CloseRequested);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let state = state_of(window);
                if !state.is_null() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    if (*state).is_dispatching() {
                        (*state).free_after_dispatch = true;
                    } else {
                        drop(Box::from_raw(state));
                    }
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn colorref(color: Color) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

pub fn hotkey_id(index: i32) -> i32 {
    HOTKEY_BASE_ID + index
}

pub fn timer_id() -> usize {
    TIMER_ID
}
