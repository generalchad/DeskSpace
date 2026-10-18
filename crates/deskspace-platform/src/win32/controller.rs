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
    SPI_SETCLIENTAREAANIMATION, WM_CLIPBOARDUPDATE, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY,
    WM_ENDSESSION, WM_HOTKEY, WM_QUERYENDSESSION, WM_SETTINGCHANGE, WM_TIMER, WS_EX_TOOLWINDOW,
    WS_POPUP,
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

/// Whether a WM_SETTINGCHANGE reports Windows' "Animation effects" switch,
/// which `SystemParametersInfo` broadcasts with the action in `wParam`.
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange
fn is_animation_change(wparam: WPARAM) -> bool {
    wparam.0 == SPI_SETCLIENTAREAANIMATION.0 as usize
}

/// The event a posted or plain notification message stands for, decoded from
/// its parameters alone. `None` when the message is not one of these, or its
/// payload does not decode.
fn event_for(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Event> {
    let event = match message {
        WM_HOTKEY => Event::Hotkey {
            id: wparam.0 as i32,
        },
        // SetTimer with no callback posts WM_TIMER; wParam is the timer id.
        // https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-timer
        WM_TIMER => Event::Timer { id: wparam.0 },
        WM_CLIPBOARDUPDATE => Event::ClipboardChanged,
        messages::TRAY_CALLBACK => tray_event(wparam, lparam)?,
        messages::SHOW_SPACES => Event::ShowSpaces,
        messages::FOLDER_CHANGE => Event::FolderChanged,
        messages::WAKE => Event::Wake { id: wparam.0 },
        messages::EDIT_KEY => Event::EditKey(keyboard::decode(wparam.0, lparam.0)?),
        messages::DELETE_KEY => Event::DeleteSelection,
        messages::CLIPBOARD_KEY => Event::Clipboard(keyboard::clipboard_action(wparam.0)?),
        messages::RENAME_KEY => Event::RenameSelection,
        messages::PROPERTIES_KEY => Event::ShowProperties,
        messages::SELECTION_KEY => Event::SelectionKey(keyboard::selection_key(wparam.0)?),
        messages::SELECT_ALL_KEY => Event::SelectAll,
        messages::TYPE_KEY => Event::Typed(char::from_u32(wparam.0 as u32)?),
        messages::NAV_KEY => Event::Navigate(keyboard::nav_key(wparam.0)?),
        _ => return None,
    };
    Some(event)
}

/// With NOTIFYICON_VERSION_4, LOWORD(lParam) is the tray event and wParam
/// carries the anchor point in screen coordinates.
fn tray_event(wparam: WPARAM, lparam: LPARAM) -> Option<Event> {
    match (lparam.0 & 0xFFFF) as u32 {
        NIN_SELECT | NIN_KEYSELECT => Some(Event::Tray(TrayEvent::Activated)),
        WM_CONTEXTMENU => Some(Event::Tray(TrayEvent::MenuRequested {
            x: loword(wparam.0 as isize),
            y: hiword(wparam.0 as isize),
        })),
        _ => None,
    }
}

/// A press posted by the mouse hook, raised only when it landed outside
/// every DeskSpace window.
fn on_outside_probe(wparam: WPARAM, lparam: LPARAM) {
    let point = POINT {
        x: wparam.0 as isize as i32,
        y: lparam.0 as i32,
    };
    if outside_click::is_outside(point) {
        runtime::dispatch(Event::PointerDownOutside {
            on_desktop: outside_click::is_desktop(point),
        });
    }
}

fn on_destroy(window: HWND) {
    // SAFETY: `window` is the controller, still live during WM_DESTROY.
    unsafe {
        let _ = RemoveClipboardFormatListener(window);
        outside_click::stop();
        keyboard::stop();
        runtime::dispatch_close_once();
        runtime::destroy_ui_windows();
        tray::remove(window);
        PostQuitMessage(0);
    }
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
        if let Some(event) = event_for(message, wparam, lparam) {
            runtime::dispatch(event);
            return LRESULT(0);
        }
        match message {
            messages::OUTSIDE_PROBE => {
                on_outside_probe(wparam, lparam);
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                if is_color_set_change(lparam) {
                    runtime::dispatch(Event::SystemThemeChanged);
                } else if is_animation_change(wparam) {
                    runtime::dispatch(Event::SystemAnimationsChanged);
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
                on_destroy(window);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::SelectionKey;

    fn event(message: u32, wparam: usize, lparam: isize) -> Option<Event> {
        event_for(message, WPARAM(wparam), LPARAM(lparam))
    }

    #[test]
    fn posted_messages_carry_their_payload() {
        assert_eq!(event(messages::WAKE, 7, 0), Some(Event::Wake { id: 7 }));
        assert_eq!(event(WM_TIMER, 3, 0), Some(Event::Timer { id: 3 }));
        assert_eq!(event(WM_HOTKEY, 2, 0), Some(Event::Hotkey { id: 2 }));
        assert_eq!(
            event(messages::TYPE_KEY, 'k' as usize, 0),
            Some(Event::Typed('k'))
        );
        assert_eq!(
            event(messages::SELECTION_KEY, keyboard::SEL_OPEN, 0),
            Some(Event::SelectionKey(SelectionKey::Open))
        );
        assert_eq!(event(messages::SHOW_SPACES, 0, 0), Some(Event::ShowSpaces));
    }

    #[test]
    fn undecodable_payloads_and_other_messages_raise_nothing() {
        assert_eq!(event(messages::TYPE_KEY, 0xD800, 0), None);
        assert_eq!(event(messages::SELECTION_KEY, usize::MAX, 0), None);
        assert_eq!(event(messages::OUTSIDE_PROBE, 0, 0), None);
        assert_eq!(event(WM_CLOSE, 0, 0), None);
    }

    #[test]
    fn only_the_animation_switch_counts_as_an_animation_change() {
        assert!(is_animation_change(WPARAM(0x1043)));
        assert!(!is_animation_change(WPARAM(0)));
    }

    #[test]
    fn tray_callback_decodes_activation_and_menu_anchor() {
        assert_eq!(
            event(messages::TRAY_CALLBACK, 0, NIN_SELECT as isize),
            Some(Event::Tray(TrayEvent::Activated))
        );
        assert_eq!(
            event(messages::TRAY_CALLBACK, 0, NIN_KEYSELECT as isize),
            Some(Event::Tray(TrayEvent::Activated))
        );
        // A negative x (a monitor left of the primary) survives the packing.
        let anchor = ((40u16 as usize) << 16) | (-12i16 as u16 as usize);
        assert_eq!(
            event(messages::TRAY_CALLBACK, anchor, WM_CONTEXTMENU as isize),
            Some(Event::Tray(TrayEvent::MenuRequested { x: -12, y: 40 }))
        );
        assert_eq!(event(messages::TRAY_CALLBACK, 0, 0x0200), None);
    }
}
