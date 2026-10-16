//! A hidden top-level window that owns process-wide concerns: the tray icon,
//! global hotkeys, console close, session end, and system theme changes.
//!
//! It is a hidden top-level window rather than a message-only window
//! (HWND_MESSAGE) because message-only windows do not receive broadcasts such
//! as `TaskbarCreated` and `WM_SETTINGCHANGE`.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#message-only-windows

use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::UI::Shell::NIN_SELECT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostQuitMessage, RegisterWindowMessageW,
    WM_CLIPBOARDUPDATE, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_HOTKEY,
    WM_QUERYENDSESSION, WM_SETTINGCHANGE, WM_TIMER, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, TrayEvent};
use crate::win32::system::{hiword, loword};
use crate::win32::tray;
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{desktop_icons, keyboard, messages, outside_click, runtime, shell_items};

pub(crate) const CLASS_NAME: PCWSTR = w!("DeskSpaceController");
/// NIN_KEYSELECT is NIN_SELECT | NINF_KEY; defined here because it is a
/// macro, not a constant, in the SDK headers.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw
const NIN_KEYSELECT: u32 = NIN_SELECT | 0x1;

static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub(crate) fn create() -> Result<HWND, PlatformError> {
    // SAFETY: plain window class registration and creation on the UI thread.
    unsafe {
        let instance = window_class::instance()?;
        window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)))?;
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::SeqCst,
        );
        let window = CreateWindowExW(
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
        )?;
        // Watch the clipboard so a cut ghost clears when another app (or the
        // desktop) replaces what DeskSpace put there.
        let _ = AddClipboardFormatListener(window);
        Ok(window)
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
            messages::SHOW_SPACES => {
                runtime::dispatch(Event::ShowSpaces);
                LRESULT(0)
            }
            messages::FOLDER_CHANGE => {
                runtime::dispatch(Event::FolderChanged);
                LRESULT(0)
            }
            messages::WAKE => {
                runtime::dispatch(Event::Wake { id: wparam.0 });
                LRESULT(0)
            }
            messages::TRAY_CALLBACK => {
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
            // SetTimer with no callback posts WM_TIMER; wParam is the timer id.
            // https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-timer
            WM_TIMER => {
                runtime::dispatch(Event::Timer { id: wparam.0 });
                LRESULT(0)
            }
            messages::EDIT_KEY => {
                if let Some(key) = keyboard::decode(wparam.0, lparam.0) {
                    runtime::dispatch(Event::EditKey(key));
                }
                LRESULT(0)
            }
            messages::DELETE_KEY => {
                runtime::dispatch(Event::DeleteSelection);
                LRESULT(0)
            }
            messages::CLIPBOARD_KEY => {
                if let Some(action) = keyboard::clipboard_action(wparam.0) {
                    runtime::dispatch(Event::Clipboard(action));
                }
                LRESULT(0)
            }
            messages::RENAME_KEY => {
                runtime::dispatch(Event::RenameSelection);
                LRESULT(0)
            }
            messages::PROPERTIES_KEY => {
                runtime::dispatch(Event::ShowProperties);
                LRESULT(0)
            }
            messages::SELECTION_KEY => {
                if let Some(key) = keyboard::selection_key(wparam.0) {
                    runtime::dispatch(Event::SelectionKey(key));
                }
                LRESULT(0)
            }
            messages::SELECT_ALL_KEY => {
                runtime::dispatch(Event::SelectAll);
                LRESULT(0)
            }
            messages::TYPE_KEY => {
                if let Some(character) = char::from_u32(wparam.0 as u32) {
                    runtime::dispatch(Event::Typed(character));
                }
                LRESULT(0)
            }
            messages::NAV_KEY => {
                if let Some(key) = keyboard::nav_key(wparam.0) {
                    runtime::dispatch(Event::Navigate(key));
                }
                LRESULT(0)
            }
            WM_CLIPBOARDUPDATE => {
                runtime::dispatch(Event::ClipboardChanged);
                LRESULT(0)
            }
            messages::OUTSIDE_PROBE => {
                let point = POINT {
                    x: wparam.0 as isize as i32,
                    y: lparam.0 as i32,
                };
                if outside_click::is_outside(point) {
                    runtime::dispatch(Event::PointerDownOutside {
                        on_desktop: outside_click::is_desktop(point),
                    });
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
                let _ = RemoveClipboardFormatListener(window);
                outside_click::stop();
                keyboard::stop();
                runtime::dispatch_close_once();
                runtime::destroy_ui_windows();
                tray::remove(window);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ if message != 0 && message == TASKBAR_CREATED.load(Ordering::SeqCst) => {
                tray::add(window, &runtime::tray_tooltip());
                // Explorer restarted and recreated its desktop listview; hide
                // it again if takeover is on.
                desktop_icons::rehide_if_active();
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
