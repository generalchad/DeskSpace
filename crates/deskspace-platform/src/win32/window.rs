use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use deskspace_core::theme::Color;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetClientRect,
    GetMessageW, GetWindowLongPtrW, LWA_ALPHA, LWA_COLORKEY, MSG, PostQuitMessage, RegisterClassW,
    SW_HIDE, SW_SHOW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    TranslateMessage, WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WM_TIMER,
    WNDCLASSW, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    WS_POPUP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{MouseButton, WindowEvent};
use crate::render::{Painter, Scene};
use crate::win32::{desktop_shell, dpi};

const CLASS_NAME: PCWSTR = w!("DeskSpaceOverlay");
const HOTKEY_BASE_ID: i32 = 0x100;
const TIMER_ID: usize = 1;

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
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    pub fn show(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

struct WindowState {
    hwnd: HWND,
    handler: Box<dyn EventHandler>,
    painter: Painter,
    color_key: Color,
    width: i32,
    height: i32,
    dpi: u32,
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
        unsafe {
            let module = GetModuleHandleW(None)?;
            let instance = HINSTANCE(module.0);
            ensure_class(instance)?;

            let host = desktop_shell::find_host(config.placement);

            let state = Box::new(WindowState {
                hwnd: HWND::default(),
                handler,
                painter: Painter::new(),
                color_key: config.color_key,
                width: config.width,
                height: config.height,
                dpi: 96,
            });
            let state = Box::into_raw(state);

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

            let hwnd = match CreateWindowExW(
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
            ) {
                Ok(hwnd) => hwnd,
                Err(error) => {
                    drop(Box::from_raw(state));
                    return Err(PlatformError::Win32(error));
                }
            };

            (*state).hwnd = hwnd;

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
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    pub fn toggle_visible(&mut self) {
        self.set_visible(!self.visible);
    }

    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    pub fn run(self) -> Result<(), PlatformError> {
        let mut message = MSG::default();
        unsafe {
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }
}

impl Drop for DesktopWindow {
    fn drop(&mut self) {
        unsafe {
            if windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(self.hwnd)).as_bool() {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
            }
        }
    }
}

fn ensure_class(instance: HINSTANCE) -> Result<(), PlatformError> {
    REGISTER_CLASS.call_once(|| unsafe {
        let class = WNDCLASSW {
            style: Default::default(),
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
        let atom = RegisterClassW(&class);
        CLASS_ATOM.store(atom as u32, Ordering::SeqCst);
    });
    if CLASS_ATOM.load(Ordering::SeqCst) == 0 {
        Err(PlatformError::ClassRegistration)
    } else {
        Ok(())
    }
}

unsafe fn state_of(window: HWND) -> *mut WindowState {
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut WindowState }
}

unsafe fn dispatch(state: &mut WindowState, event: WindowEvent) {
    let mut ctx = WindowContext {
        hwnd: state.hwnd,
        painter: &mut state.painter,
        width: state.width,
        height: state.height,
        color_key: state.color_key,
    };
    state.handler.on_event(event, &mut ctx);
}

unsafe fn client_size(window: HWND) -> (i32, i32) {
    let mut rect = RECT::default();
    if unsafe { GetClientRect(window, &mut rect) }.is_ok() {
        (rect.right - rect.left, rect.bottom - rect.top)
    } else {
        (0, 0)
    }
}

fn loword(value: isize) -> i32 {
    (value & 0xFFFF) as i16 as i32
}

fn hiword(value: isize) -> i32 {
    ((value >> 16) & 0xFFFF) as i16 as i32
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
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
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    let dpi = dpi::window_dpi(window);
                    state.dpi = dpi;
                    let (width, height) = client_size(window);
                    if width > 0 && height > 0 {
                        state.width = width;
                        state.height = height;
                    }
                    dispatch(
                        state,
                        WindowEvent::Created {
                            width: state.width,
                            height: state.height,
                            dpi,
                        },
                    );
                }
                LRESULT(0)
            }
            WM_SIZE => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    let width = loword(lparam.0);
                    let height = hiword(lparam.0);
                    state.width = width;
                    state.height = height;
                    dispatch(state, WindowEvent::Resized { width, height });
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    let dpi = (wparam.0 & 0xFFFF) as u32;
                    state.dpi = dpi;
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
                    dispatch(state, WindowEvent::DpiChanged { dpi });
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    state.painter.reset_paint();
                    dispatch(state, WindowEvent::Paint);
                    if !state.painter.was_painted() {
                        let _ = windows::Win32::Graphics::Gdi::ValidateRect(Some(window), None);
                    }
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    dispatch(
                        state,
                        WindowEvent::MouseMove {
                            x: loword(lparam.0),
                            y: hiword(lparam.0),
                        },
                    );
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    let button = match message {
                        WM_LBUTTONDOWN => MouseButton::Left,
                        WM_RBUTTONDOWN => MouseButton::Right,
                        _ => MouseButton::Middle,
                    };
                    dispatch(
                        state,
                        WindowEvent::MouseDown {
                            x: loword(lparam.0),
                            y: hiword(lparam.0),
                            button,
                        },
                    );
                }
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    let button = match message {
                        WM_LBUTTONUP => MouseButton::Left,
                        WM_RBUTTONUP => MouseButton::Right,
                        _ => MouseButton::Middle,
                    };
                    dispatch(
                        state,
                        WindowEvent::MouseUp {
                            x: loword(lparam.0),
                            y: hiword(lparam.0),
                            button,
                        },
                    );
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    dispatch(
                        state,
                        WindowEvent::DoubleClick {
                            x: loword(lparam.0),
                            y: hiword(lparam.0),
                        },
                    );
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    dispatch(
                        state,
                        WindowEvent::Hotkey {
                            id: wparam.0 as i32,
                        },
                    );
                }
                LRESULT(0)
            }
            WM_TIMER => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    dispatch(state, WindowEvent::Timer { id: wparam.0 });
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let state = state_of(window);
                if let Some(state) = state.as_mut() {
                    dispatch(state, WindowEvent::CloseRequested);
                }
                PostQuitMessage(0);
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

fn colorref(color: Color) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

pub fn hotkey_id(index: i32) -> i32 {
    HOTKEY_BASE_ID + index
}

pub fn timer_id() -> usize {
    TIMER_ID
}
