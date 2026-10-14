//! The "Custom shortcut" dialog (ROADMAP F-53): the old-fashioned way to make
//! a shortcut, as Explorer's Create Shortcut wizard does. The user types or
//! browses to a target (a file, folder, program, or web address), optional
//! arguments, and a name, which follows the target until it is edited.
//!
//! A typed path is resolved the way the Run dialog reads it
//! (`shortcuts::resolve_path`), so `%WINDIR%\notepad.exe` and `notepad` work,
//! and a target that does not exist is reported in the dialog instead of
//! producing a broken shortcut.

use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{FillRect, HDC};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CREATESTRUCTW, DefWindowProcW, EN_CHANGE, ES_AUTOHSCROLL,
    GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, RegisterClassW, SetWindowLongPtrW,
    SetWindowTextW, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_NCCREATE, WM_NCDESTROY, WNDCLASSW,
    WS_EX_CLIENTEDGE, WS_TABSTOP,
};
use windows::core::{PCWSTR, w};

use crate::ui::{CustomShortcut, ShortcutTarget};
use crate::win32::dialog::{self, DialogChrome, ID_CANCEL, ID_OK, window_text};
use crate::win32::shortcuts::{display_name, resolve_path};
use crate::win32::system::wide;

const CLASS_NAME: PCWSTR = w!("DeskSpaceShortcutDialog");
const ID_TARGET: usize = 10;
const ID_BROWSE: usize = 11;
const ID_ARGUMENTS: usize = 12;
const ID_NAME: usize = 13;
/// The client size, at 96 DPI.
const CLIENT_WIDTH: i32 = 480;
const CLIENT_HEIGHT: i32 = 262;
/// SS_NOPREFIX | SS_CENTERIMAGE for labels (WinUser.h).
/// https://learn.microsoft.com/en-us/windows/win32/controls/static-control-styles
const LABEL_STYLE: u32 = 0x0080 | 0x0200;

static REGISTER_CLASS: Once = Once::new();
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

struct DialogState {
    owner: HWND,
    chrome: DialogChrome,
    target: Option<HWND>,
    arguments: Option<HWND>,
    name: Option<HWND>,
    /// The line that explains why Create did nothing.
    message: Option<HWND>,
    /// The user typed a name, so it no longer follows the target.
    name_edited: bool,
    /// Set while the dialog itself writes the name, so the resulting
    /// `EN_CHANGE` is not taken as typing.
    setting_name: bool,
    result: Option<CustomShortcut>,
}

/// Shows the dialog titled `title` over `owner` and returns the shortcut the
/// user described, or `None` when they cancelled. Blocks in a modal loop.
pub(crate) unsafe fn custom_shortcut(
    owner: HWND,
    title: &str,
    dark: bool,
) -> Option<CustomShortcut> {
    // SAFETY: Win32 window creation on the UI thread. `state` is owned here
    // and dropped after the loop; the window points at it only while alive.
    unsafe {
        if !ensure_class() {
            return None;
        }
        let chrome = DialogChrome::new(dialog::dpi_of(owner), dark);
        let client = (chrome.s(CLIENT_WIDTH), chrome.s(CLIENT_HEIGHT));
        let dpi = chrome.dpi;
        let mut state = Box::new(DialogState {
            owner,
            chrome,
            target: None,
            arguments: None,
            name: None,
            message: None,
            name_edited: false,
            setting_name: false,
            result: None,
        });
        let pointer = &mut *state as *mut DialogState;
        let Some(window) =
            dialog::create_frame(CLASS_NAME, title, owner, client, dpi, pointer.cast())
        else {
            tracing::warn!("could not create the custom shortcut dialog");
            return None;
        };
        build(window, &mut state, client);
        dialog::show(window, dark);
        if let Some(target) = state.target {
            let _ = SetFocus(Some(target));
        }
        dialog::run_modal(owner, window);
        state.result.take()
    }
}

unsafe fn build(window: HWND, state: &mut DialogState, client: (i32, i32)) {
    let chrome = &state.chrome;
    let s = |value| chrome.s(value);
    let margin = s(16);
    let width = client.0 - 2 * margin;
    let field_height = s(24);
    let browse_width = s(96);
    let tab = WS_TABSTOP.0;
    let edit = |text: &str, bounds, id| {
        // SAFETY: a child edit of our live window.
        unsafe {
            dialog::control(
                chrome,
                window,
                w!("Edit"),
                text,
                ES_AUTOHSCROLL as u32 | tab,
                WS_EX_CLIENTEDGE,
                bounds,
                id,
            )
        }
    };
    let label = |text: &str, y| {
        // SAFETY: a child static of our live window.
        unsafe {
            dialog::control(
                chrome,
                window,
                w!("Static"),
                text,
                LABEL_STYLE,
                WINDOW_EX_STYLE(0),
                (margin, y, width, s(20)),
                0,
            )
        }
    };
    // SAFETY: every control is a child of our live window.
    unsafe {
        let mut y = margin;
        label("Type the location of the item, or a web address:", y);
        y += s(22);
        state.target = edit(
            "",
            (margin, y, width - browse_width - s(8), field_height),
            ID_TARGET,
        );
        dialog::control(
            chrome,
            window,
            w!("Button"),
            "Browse\u{2026}",
            BS_PUSHBUTTON as u32 | tab,
            WINDOW_EX_STYLE(0),
            (margin + width - browse_width, y - s(3), browse_width, s(30)),
            ID_BROWSE,
        );
        y += field_height + s(12);
        label("Arguments (optional):", y);
        y += s(22);
        state.arguments = edit("", (margin, y, width, field_height), ID_ARGUMENTS);
        y += field_height + s(12);
        label("Name:", y);
        y += s(22);
        state.name = edit("", (margin, y, width, field_height), ID_NAME);
        let button = (s(96), s(30));
        let buttons_y = client.1 - margin - button.1;
        state.message = dialog::control(
            chrome,
            window,
            w!("Static"),
            "",
            LABEL_STYLE,
            WINDOW_EX_STYLE(0),
            (margin, buttons_y, width - 2 * button.0 - s(16), button.1),
            0,
        );
        dialog::control(
            chrome,
            window,
            w!("Button"),
            "Create",
            BS_DEFPUSHBUTTON as u32 | tab,
            WINDOW_EX_STYLE(0),
            (
                client.0 - margin - 2 * button.0 - s(8),
                buttons_y,
                button.0,
                button.1,
            ),
            ID_OK,
        );
        dialog::control(
            chrome,
            window,
            w!("Button"),
            "Cancel",
            BS_PUSHBUTTON as u32 | tab,
            WINDOW_EX_STYLE(0),
            (client.0 - margin - button.0, buttons_y, button.0, button.1),
            ID_CANCEL,
        );
    }
}

/// Keeps the name following the target until the user edits it, and greys
/// the arguments for a web address, which takes none.
unsafe fn on_target_changed(window: HWND) {
    // SAFETY: the handles are copied out under a short borrow; the controls
    // are written after it.
    unsafe {
        let Some((Some(target), arguments, name, edited)) = with_state(window, |state| {
            (state.target, state.arguments, state.name, state.name_edited)
        }) else {
            return;
        };
        let parsed = ShortcutTarget::parse(&window_text(target));
        if let Some(arguments) = arguments {
            let _ = EnableWindow(arguments, !matches!(parsed, Some(ShortcutTarget::Url(_))));
        }
        if edited {
            return;
        }
        if let Some(name) = name {
            let text = parsed
                .map(|target| target.suggested_name())
                .unwrap_or_default();
            with_state(window, |state| state.setting_name = true);
            let text = wide(&text);
            let _ = SetWindowTextW(name, PCWSTR(text.as_ptr()));
            with_state(window, |state| state.setting_name = false);
        }
        show_message(window, "");
    }
}

/// A name typed by the user stops following the target; clearing it lets it
/// follow again.
unsafe fn on_name_changed(window: HWND) {
    // SAFETY: the edit's text is read outside the borrow.
    unsafe {
        let Some((Some(name), false)) =
            with_state(window, |state| (state.name, state.setting_name))
        else {
            return;
        };
        let edited = !window_text(name).trim().is_empty();
        with_state(window, |state| state.name_edited = edited);
    }
}

/// Browse: picks a file with the standard Open dialog and makes it the
/// target. Folders can be typed.
unsafe fn on_browse(window: HWND) {
    // SAFETY: the Open dialog is modal over our window; the target edit is
    // written after it returns.
    unsafe {
        let Some(path) = browse_for_file(window) else {
            return;
        };
        if let Some(Some(target)) = with_state(window, |state| state.target) {
            let text = wide(&path.to_string_lossy());
            let _ = SetWindowTextW(target, PCWSTR(text.as_ptr()));
            let _ = SetFocus(Some(target));
        }
    }
}

/// The standard Open dialog over `owner`, for one existing file.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileopendialog
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions
unsafe fn browse_for_file(owner: HWND) -> Option<PathBuf> {
    // SAFETY: OLE is initialized on this thread; the dialog is modal over
    // `owner` and released when it goes out of scope.
    unsafe {
        let picker: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = picker.GetOptions().ok()?;
        picker
            .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)
            .ok()?;
        let _ = picker.SetTitle(w!("Choose the item the shortcut opens"));
        // Cancel comes back as an error (HRESULT_FROM_WIN32(ERROR_CANCELLED)).
        picker.Show(Some(owner)).ok()?;
        let item = picker.GetResult().ok()?;
        display_name(&item, SIGDN_FILESYSPATH).map(PathBuf::from)
    }
}

/// Create: checks the target and hands back the shortcut, or explains in the
/// dialog why it cannot.
unsafe fn on_create(window: HWND) {
    // SAFETY: the handles are copied out under a short borrow; the edits are
    // read after it, and the borrow that stores the result makes no calls.
    unsafe {
        let Some((Some(target), arguments, name, owner)) = with_state(window, |state| {
            (state.target, state.arguments, state.name, state.owner)
        }) else {
            return;
        };
        let typed = window_text(target);
        let shortcut = match ShortcutTarget::parse(&typed) {
            None => Err("Type the location of an item, or a web address.".to_string()),
            Some(ShortcutTarget::Url(url)) => Ok(ShortcutTarget::Url(url)),
            Some(ShortcutTarget::Path(path)) => resolve_path(&path)
                .map(ShortcutTarget::Path)
                .ok_or_else(|| format!("DeskSpace can't find \u{201C}{}\u{201D}.", typed.trim())),
        };
        let target = match shortcut {
            Ok(target) => target,
            Err(message) => {
                show_message(window, &message);
                return;
            }
        };
        let typed_name = name.map(|name| window_text(name)).unwrap_or_default();
        let name = match typed_name.trim() {
            "" => target.suggested_name(),
            typed => typed.to_string(),
        };
        let arguments = arguments.map(|edit| window_text(edit)).unwrap_or_default();
        with_state(window, |state| {
            state.result = Some(CustomShortcut {
                target,
                arguments: arguments.trim().to_string(),
                name,
            });
        });
        dialog::close(owner, window);
    }
}

unsafe fn show_message(window: HWND, text: &str) {
    // SAFETY: the static is a live child; the borrow ends before it is set.
    unsafe {
        if let Some(Some(message)) = with_state(window, |state| state.message) {
            let text = wide(text);
            let _ = SetWindowTextW(message, PCWSTR(text.as_ptr()));
        }
    }
}

unsafe fn on_cancel(window: HWND) {
    // SAFETY: the borrow ends before the dialog is closed.
    unsafe {
        if let Some(owner) = with_state(window, |state| {
            state.result = None;
            state.owner
        }) {
            dialog::close(owner, window);
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
                let create = lparam.0 as *const CREATESTRUCTW;
                if let Some(create) = create.as_ref() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                match id {
                    ID_OK => on_create(window),
                    ID_CANCEL => on_cancel(window),
                    ID_BROWSE => on_browse(window),
                    ID_TARGET if code == EN_CHANGE => on_target_changed(window),
                    ID_NAME if code == EN_CHANGE => on_name_changed(window),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                with_state(window, |state| state.chrome.control_color(message, wparam))
                    .flatten()
                    .unwrap_or_else(|| DefWindowProcW(window, message, wparam, lparam))
            }
            WM_ERASEBKGND => {
                let Some(brush) = with_state(window, |state| state.chrome.body_brush) else {
                    return DefWindowProcW(window, message, wparam, lparam);
                };
                let mut client = RECT::default();
                let _ = GetClientRect(window, &mut client);
                FillRect(HDC(wparam.0 as *mut core::ffi::c_void), &client, brush);
                LRESULT(1)
            }
            WM_CLOSE => {
                on_cancel(window);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                // The state belongs to `custom_shortcut`, which drops it after
                // the modal loop; only the pointer is cleared here.
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn ensure_class() -> bool {
    REGISTER_CLASS.call_once(|| {
        let Some(instance) = dialog::instance() else {
            return;
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its name is static.
        CLASS_REGISTERED.store(unsafe { RegisterClassW(&class) } != 0, Ordering::SeqCst);
    });
    CLASS_REGISTERED.load(Ordering::SeqCst)
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut DialogState) -> R) -> Option<R> {
    // SAFETY: the pointer is the live state `custom_shortcut` owns for as
    // long as the window exists, and `f` cannot re-enter for a second borrow.
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut DialogState;
    unsafe { pointer.as_mut() }.map(f)
}
