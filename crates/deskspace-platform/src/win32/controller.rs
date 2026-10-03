//! A hidden top-level window that owns process-wide concerns: the tray icon,
//! global hotkeys, console close, session end, and system theme changes.
//!
//! It is a hidden top-level window rather than a message-only window
//! (HWND_MESSAGE) because message-only windows do not receive broadcasts such
//! as `TaskbarCreated` and `WM_SETTINGCHANGE`.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#message-only-windows

use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::NIN_SELECT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_HOTKEY,
    WM_QUERYENDSESSION, WM_SETTINGCHANGE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, TrayEvent};
use crate::win32::system::{hiword, loword};
use crate::win32::tray;
use crate::win32::{runtime, shell_items};

const CLASS_NAME: PCWSTR = w!("DeskSpaceController");
pub(crate) const TRAY_CALLBACK: u32 = WM_APP + 1;
/// NIN_KEYSELECT is NIN_SELECT | NINF_KEY; defined here because it is a
/// macro, not a constant, in the SDK headers.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw
const NIN_KEYSELECT: u32 = NIN_SELECT | 0x1;

static REGISTER_CLASS: Once = Once::new();
static CLASS_ATOM: AtomicU32 = AtomicU32::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub(crate) fn create() -> Result<HWND, PlatformError> {
    // SAFETY: plain window class registration and creation on the UI thread.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        REGISTER_CLASS.call_once(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            CLASS_ATOM.store(RegisterClassW(&class) as u32, Ordering::SeqCst);
            TASKBAR_CREATED.store(
                RegisterWindowMessageW(w!("TaskbarCreated")),
                Ordering::SeqCst,
            );
        });
        if CLASS_ATOM.load(Ordering::SeqCst) == 0 {
            return Err(PlatformError::ClassRegistration);
        }
        Ok(CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!("DeskSpace"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )?)
    }
}

/// Whether a WM_SETTINGCHANGE is the "ImmersiveColorSet" broadcast sent when
/// the user switches between light and dark mode.
unsafe fn is_color_set_change(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    // SAFETY: for WM_SETTINGCHANGE a non-null lParam is a NUL-terminated
    // wide string naming the changed area.
    let area = unsafe { PCWSTR(lparam.0 as *const u16).to_string() };
    area.is_ok_and(|area| area == "ImmersiveColorSet")
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`.
    unsafe {
        if let Some(result) = shell_items::forward_menu_message(message, wparam, lparam) {
            return result;
        }
        match message {
            WM_HOTKEY => {
                runtime::dispatch(Event::Hotkey {
                    id: wparam.0 as i32,
                });
                LRESULT(0)
            }
            TRAY_CALLBACK => {
                // With NOTIFYICON_VERSION_4, LOWORD(lParam) is the event and
                // wParam carries the anchor point in screen coordinates.
                match (lparam.0 & 0xFFFF) as u32 {
                    NIN_SELECT | NIN_KEYSELECT => {
                        runtime::dispatch(Event::Tray(TrayEvent::Activated));
                    }
                    WM_CONTEXTMENU => {
                        runtime::dispatch(Event::Tray(TrayEvent::MenuRequested {
                            x: loword(wparam.0 as isize),
                            y: hiword(wparam.0 as isize),
                        }));
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                if is_color_set_change(lparam) {
                    runtime::dispatch(Event::SystemThemeChanged);
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(window);
                LRESULT(0)
            }
            WM_QUERYENDSESSION => LRESULT(1),
            WM_ENDSESSION => {
                // At logoff/shutdown the process may be terminated once this
                // returns, without WM_DESTROY, so save now.
                // https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-endsession
                if wparam.0 != 0 {
                    runtime::dispatch_close_once();
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                runtime::dispatch_close_once();
                runtime::destroy_ui_windows();
                tray::remove(window);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ if message != 0 && message == TASKBAR_CREATED.load(Ordering::SeqCst) => {
                tray::add(window, &runtime::tray_tooltip());
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
