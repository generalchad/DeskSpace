//! Low-level keyboard capture for inline title editing and item keys.
//!
//! Spaces are never activated, so they cannot receive `WM_CHAR` or the usual
//! clipboard/Delete accelerators. While the app is renaming a space it installs
//! a `WH_KEYBOARD_LL` hook, translates each key press into an `EditKey`, posts
//! it to the controller, and consumes the event so it does not reach the
//! focused app. Outside a rename the same hook can watch keys for the current
//! selection or space instead of editing; `fn handle` is the source of truth for
//! which key needs which flag, and the AGENTS.md "Gotchas" table repeats it. The
//! hook runs for every key in the session and Windows drops it if this thread is
//! slow to pump messages (`LowLevelHooksTimeout`), so it is installed only
//! while one of those modes is active.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc

use std::cell::Cell;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VIRTUAL_KEY,
    VK_A, VK_APPS, VK_BACK, VK_C, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE,
    VK_F2, VK_F5, VK_F10, VK_HOME, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT,
    VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_UP, VK_V, VK_X,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GetAncestor, GetForegroundWindow, HHOOK, KBDLLHOOKSTRUCT,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

use crate::edit::{EditKey, MoveDir};
use crate::input::{ClipboardAction, NavDir, NavKey, SelectionKey};
use crate::win32::desktop_shell;

/// Posted to the controller with the decoded key: wParam is a kind code and
/// lParam the character for `KIND_CHAR`.
pub(crate) const EDIT_KEY: u32 = WM_APP + 3;

/// Posted to the controller when Delete is pressed while an item is selected.
pub(crate) const DELETE_KEY: u32 = WM_APP + 4;

/// Posted to the controller with a clipboard shortcut; wParam is one of the
/// `ACTION_*` codes.
pub(crate) const CLIPBOARD_KEY: u32 = WM_APP + 5;

/// Posted to the controller when F2 is pressed while an item is selected or a
/// space is current.
pub(crate) const RENAME_KEY: u32 = WM_APP + 6;

/// Posted to the controller when Ctrl+A is pressed while a space is current.
pub(crate) const SELECT_ALL_KEY: u32 = WM_APP + 7;

/// Posted to the controller when Alt+Enter is pressed while an item is
/// selected or a space is current.
pub(crate) const PROPERTIES_KEY: u32 = WM_APP + 9;

/// Posted to the controller for a watched selection key; wParam is one of the
/// `SEL_*` codes.
pub(crate) const SELECTION_KEY: u32 = WM_APP + 11;

/// Posted to the controller with a typed character for type-to-select; wParam
/// is the character.
pub(crate) const TYPE_KEY: u32 = WM_APP + 12;

/// Posted to the controller for an arrow key; wParam packs the direction and
/// the Shift (extend) and Ctrl (focus) modifiers.
pub(crate) const NAV_KEY: u32 = WM_APP + 13;

pub(crate) const SEL_OPEN: usize = 0;
pub(crate) const SEL_CLEAR: usize = 1;
pub(crate) const SEL_PERMANENT_DELETE: usize = 2;
pub(crate) const SEL_REFRESH: usize = 3;
pub(crate) const SEL_MENU: usize = 4;
pub(crate) const SEL_HOME: usize = 5;
pub(crate) const SEL_END: usize = 6;
pub(crate) const SEL_PAGE_UP: usize = 7;
pub(crate) const SEL_PAGE_DOWN: usize = 8;

/// Direction codes packed into `NAV_KEY`'s wParam low bits.
pub(crate) const NAV_LEFT: usize = 0;
pub(crate) const NAV_RIGHT: usize = 1;
pub(crate) const NAV_UP: usize = 2;
pub(crate) const NAV_DOWN: usize = 3;

pub(crate) const ACTION_COPY: usize = 0;
pub(crate) const ACTION_CUT: usize = 1;
pub(crate) const ACTION_PASTE: usize = 2;

const KIND_CHAR: usize = 0;
const KIND_BACKSPACE: usize = 1;
const KIND_DELETE: usize = 2;
const KIND_LEFT: usize = 3;
const KIND_RIGHT: usize = 4;
const KIND_HOME: usize = 5;
const KIND_END: usize = 6;
const KIND_SELECT_LEFT: usize = 7;
const KIND_SELECT_RIGHT: usize = 8;
const KIND_SELECT_HOME: usize = 9;
const KIND_SELECT_END: usize = 10;
const KIND_SELECT_ALL: usize = 11;
const KIND_COMMIT: usize = 12;
const KIND_CANCEL: usize = 13;
const KIND_BACKSPACE_WORD: usize = 14;
const KIND_DELETE_WORD: usize = 15;
const KIND_WORD_LEFT: usize = 16;
const KIND_WORD_RIGHT: usize = 17;
const KIND_SELECT_WORD_LEFT: usize = 18;
const KIND_SELECT_WORD_RIGHT: usize = 19;

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static CONTROLLER: Cell<Option<HWND>> = const { Cell::new(None) };
    /// Capturing every editing keystroke (inline rename).
    static EDITING: Cell<bool> = const { Cell::new(false) };
    /// Watching while an item is selected.
    static WATCH_SELECTION: Cell<bool> = const { Cell::new(false) };
    /// Watching while a space is current (the name is historical).
    static WATCH_PASTE: Cell<bool> = const { Cell::new(false) };
    /// A popup menu is being tracked, so every key belongs to it; the hook
    /// passes everything through until it closes.
    static MENU_OPEN: Cell<bool> = const { Cell::new(false) };
}

/// Marks a popup menu as open, so the hook stops consuming keys (Esc, Enter,
/// arrows) that belong to the menu while it is tracked.
pub(crate) fn set_menu_open(open: bool) {
    MENU_OPEN.set(open);
}

/// Starts or stops capturing keys for `controller` for inline editing.
/// Idempotent.
pub(crate) fn capture(controller: HWND, enabled: bool) {
    EDITING.set(enabled);
    refresh(controller);
}

/// Watches the selection keys while `selection` is true, and the keys that need
/// a current space while `paste` is true. Idempotent.
pub(crate) fn watch_selection(controller: HWND, selection: bool, paste: bool) {
    WATCH_SELECTION.set(selection);
    WATCH_PASTE.set(paste);
    refresh(controller);
}

/// Installs or removes the hook so it is on exactly when a mode needs it.
fn refresh(controller: HWND) {
    let wanted = EDITING.get() || WATCH_SELECTION.get() || WATCH_PASTE.get();
    if wanted == HOOK.get().is_some() {
        if wanted {
            CONTROLLER.set(Some(controller));
        }
        return;
    }
    if !wanted {
        stop();
        return;
    }
    CONTROLLER.set(Some(controller));
    // SAFETY: the hook procedure lives in this module for the life of the
    // process; a low-level hook runs on the installing thread, which pumps
    // messages for the whole run.
    let hook = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hook_proc),
                Some(HINSTANCE(module.0)),
                0,
            )
        })
    };
    match hook {
        Ok(hook) => HOOK.set(Some(hook)),
        Err(error) => tracing::warn!(%error, "could not install the keyboard hook"),
    }
}

/// Removes the hook and clears every watch mode.
pub(crate) fn stop() {
    if let Some(hook) = HOOK.take() {
        // SAFETY: `hook` was returned by SetWindowsHookExW on this thread.
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    CONTROLLER.set(None);
    EDITING.set(false);
    WATCH_SELECTION.set(false);
    WATCH_PASTE.set(false);
}

/// Whether the desktop shell (Progman or the wallpaper WorkerW), or the
/// top-level window it belongs to, has the foreground. A space has the user's
/// attention only while the desktop does, because spaces never activate and
/// clicking one deliberately focuses the desktop.
fn desktop_is_foreground() -> bool {
    // SAFETY: plain window queries.
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() {
            return false;
        }
        desktop_shell::is_desktop_window(foreground)
            || desktop_shell::is_desktop_window(GetAncestor(foreground, GA_ROOT))
    }
}

/// Whether a modifier key is currently held.
fn key_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: plain key-state query.
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

fn is_key(vk: u32, key: VIRTUAL_KEY) -> bool {
    vk == key.0 as u32
}

/// Whether `vk` is a key the system owns (Alt, Windows, or the menu key).
/// These cancel the rename and pass through, so System shortcuts (Alt+Tab,
/// Win+D, the Start menu) keep working. The pressed key is checked directly
/// because a low-level hook runs before `GetAsyncKeyState` reflects it.
fn is_system_key(vk: u32) -> bool {
    is_key(vk, VK_LWIN)
        || is_key(vk, VK_RWIN)
        || is_key(vk, VK_LMENU)
        || is_key(vk, VK_RMENU)
        || is_key(vk, VK_APPS)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let message = wparam.0 as u32;
    if code >= 0 && (message == WM_KEYDOWN || message == WM_SYSKEYDOWN) {
        // SAFETY: for a keyboard hook, lParam points at a KBDLLHOOKSTRUCT.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if handle(event) {
            return LRESULT(1);
        }
    }
    // SAFETY: every hook must pass the event on.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Handles one key press. Returns whether the event was consumed; when it
/// returns `false`, the caller passes the event on unchanged.
fn handle(event: &KBDLLHOOKSTRUCT) -> bool {
    // An open menu owns every key (Esc to close, Enter to activate, arrows to
    // navigate), so pass them all through while one is tracked.
    if MENU_OPEN.get() {
        return false;
    }
    // Selection watch: capture Delete and the clipboard keys that act on a
    // selection or a current space, and pass every other key through untouched.
    // The selection is kept while another app is focused, as Explorer keeps a
    // desktop selection, so these keys belong to a space only while the desktop
    // shell has the foreground; otherwise an app's own Ctrl+C/V is stolen.
    if (WATCH_SELECTION.get() || WATCH_PASTE.get()) && !EDITING.get() {
        if !desktop_is_foreground() {
            return false;
        }
        if key_down(VK_MENU) && is_key(event.vkCode, VK_RETURN) {
            post_message(PROPERTIES_KEY, 0, 0);
            return true;
        }
        if key_down(VK_CONTROL) {
            if is_key(event.vkCode, VK_A) {
                post_message(SELECT_ALL_KEY, 0, 0);
                return true;
            }
            let action = if WATCH_SELECTION.get() && is_key(event.vkCode, VK_C) {
                Some(ACTION_COPY)
            } else if WATCH_SELECTION.get() && is_key(event.vkCode, VK_X) {
                Some(ACTION_CUT)
            } else if WATCH_PASTE.get() && is_key(event.vkCode, VK_V) {
                Some(ACTION_PASTE)
            } else {
                None
            };
            if let Some(action) = action {
                post_message(CLIPBOARD_KEY, action, 0);
                return true;
            }
            // Ctrl+arrow moves the focus only, leaving the selection.
            if WATCH_PASTE.get()
                && let Some(dir) = arrow_dir(event.vkCode)
            {
                post_message(NAV_KEY, dir | (1 << 3), 0);
                return true;
            }
            return false;
        }
        if WATCH_SELECTION.get() && is_key(event.vkCode, VK_RETURN) {
            post_message(SELECTION_KEY, SEL_OPEN, 0);
            return true;
        }
        if (WATCH_SELECTION.get() || WATCH_PASTE.get()) && is_key(event.vkCode, VK_F2) {
            post_message(RENAME_KEY, 0, 0);
            return true;
        }
        if WATCH_SELECTION.get() && key_down(VK_SHIFT) && is_key(event.vkCode, VK_DELETE) {
            post_message(SELECTION_KEY, SEL_PERMANENT_DELETE, 0);
            return true;
        }
        if WATCH_SELECTION.get() && is_key(event.vkCode, VK_DELETE) {
            post_message(DELETE_KEY, 0, 0);
            return true;
        }
        if is_key(event.vkCode, VK_ESCAPE) {
            post_message(SELECTION_KEY, SEL_CLEAR, 0);
            return true;
        }
        if is_key(event.vkCode, VK_F5) {
            post_message(SELECTION_KEY, SEL_REFRESH, 0);
            return true;
        }
        // The Menu key and Shift+F10 open the context menu, as on the desktop.
        if is_key(event.vkCode, VK_APPS) || (key_down(VK_SHIFT) && is_key(event.vkCode, VK_F10)) {
            post_message(SELECTION_KEY, SEL_MENU, 0);
            return true;
        }
        // Scrolling, arrows, and type-to-select need a current space, not just
        // a selection (the paste watch tracks that).
        if WATCH_PASTE.get() {
            if let Some(dir) = arrow_dir(event.vkCode) {
                let extend = if key_down(VK_SHIFT) { 1 << 2 } else { 0 };
                post_message(NAV_KEY, dir | extend, 0);
                return true;
            }
            let scroll = if is_key(event.vkCode, VK_HOME) {
                Some(SEL_HOME)
            } else if is_key(event.vkCode, VK_END) {
                Some(SEL_END)
            } else if is_key(event.vkCode, VK_PRIOR) {
                Some(SEL_PAGE_UP)
            } else if is_key(event.vkCode, VK_NEXT) {
                Some(SEL_PAGE_DOWN)
            } else {
                None
            };
            if let Some(scroll) = scroll {
                post_message(SELECTION_KEY, scroll, 0);
                return true;
            }
            if let Some(character) = typeahead_char(event.vkCode) {
                post_message(TYPE_KEY, character as usize, 0);
                return true;
            }
        }
        return false;
    }

    // Alt and Windows combinations belong to the system (Alt+Tab, Win+D,
    // Start menu): cancel the rename and let them through.
    if key_down(VK_MENU) || key_down(VK_LWIN) || key_down(VK_RWIN) || is_system_key(event.vkCode) {
        post(KIND_CANCEL, 0);
        return false;
    }
    if key_down(VK_CONTROL) {
        if is_key(event.vkCode, VK_A) {
            post(KIND_SELECT_ALL, 0);
            return true;
        }
        let shift = key_down(VK_SHIFT);
        let kind = if event.vkCode == VK_LEFT.0 as u32 {
            Some(if shift {
                KIND_SELECT_WORD_LEFT
            } else {
                KIND_WORD_LEFT
            })
        } else if event.vkCode == VK_RIGHT.0 as u32 {
            Some(if shift {
                KIND_SELECT_WORD_RIGHT
            } else {
                KIND_WORD_RIGHT
            })
        } else if event.vkCode == VK_BACK.0 as u32 {
            Some(KIND_BACKSPACE_WORD)
        } else if event.vkCode == VK_DELETE.0 as u32 {
            Some(KIND_DELETE_WORD)
        } else {
            None
        };
        return match kind {
            Some(kind) => {
                post(kind, 0);
                true
            }
            None => false,
        };
    }

    match classify(event.vkCode, key_down(VK_SHIFT)) {
        Some((kind, ch)) => {
            post(kind, ch);
            true
        }
        None => false,
    }
}

/// The `NAV_*` direction code for an arrow key, or `None` for any other key.
fn arrow_dir(vk: u32) -> Option<usize> {
    Some(match vk {
        v if v == VK_LEFT.0 as u32 => NAV_LEFT,
        v if v == VK_RIGHT.0 as u32 => NAV_RIGHT,
        v if v == VK_UP.0 as u32 => NAV_UP,
        v if v == VK_DOWN.0 as u32 => NAV_DOWN,
        _ => return None,
    })
}

/// Decodes a posted `NAV_KEY` wParam into a `NavKey`.
pub(crate) fn nav_key(action: usize) -> Option<NavKey> {
    let dir = match action & 0x3 {
        NAV_LEFT => NavDir::Left,
        NAV_RIGHT => NavDir::Right,
        NAV_UP => NavDir::Up,
        NAV_DOWN => NavDir::Down,
        _ => return None,
    };
    Some(NavKey {
        dir,
        extend: action & (1 << 2) != 0,
        focus: action & (1 << 3) != 0,
    })
}

/// The character a bare key press contributes to type-to-select, or `None` for
/// keys that are not text (modifiers, function keys, navigation, control
/// characters). Shift is fine (an uppercase letter); Ctrl and Alt are handled
/// before this is reached.
fn typeahead_char(vk: u32) -> Option<char> {
    // SAFETY: reading the keyboard state and layout; no pointers held.
    let character = unsafe { char_for(vk) }?;
    (!character.is_control()).then_some(character)
}

fn post(kind: usize, ch: usize) {
    post_message(EDIT_KEY, kind, ch as isize);
}

fn post_message(message: u32, wparam: usize, lparam: isize) {
    if let Some(controller) = CONTROLLER.get() {
        // SAFETY: posting to our own controller window.
        unsafe {
            let _ = PostMessageW(Some(controller), message, WPARAM(wparam), LPARAM(lparam));
        }
    }
}

/// Maps a virtual key to an `EditKey` kind. Returns `None` for keys the editor
/// does not consume (function keys, IME keys, modifiers). `shift` extends the
/// selection for the movement keys.
fn classify(vk: u32, shift: bool) -> Option<(usize, usize)> {
    let key = match vk {
        v if v == VK_BACK.0 as u32 => (KIND_BACKSPACE, 0),
        v if v == VK_DELETE.0 as u32 => (KIND_DELETE, 0),
        v if v == VK_LEFT.0 as u32 => (if shift { KIND_SELECT_LEFT } else { KIND_LEFT }, 0),
        v if v == VK_RIGHT.0 as u32 => (if shift { KIND_SELECT_RIGHT } else { KIND_RIGHT }, 0),
        v if v == VK_HOME.0 as u32 => (if shift { KIND_SELECT_HOME } else { KIND_HOME }, 0),
        v if v == VK_END.0 as u32 => (if shift { KIND_SELECT_END } else { KIND_END }, 0),
        v if v == VK_RETURN.0 as u32 => (KIND_COMMIT, 0),
        v if v == VK_ESCAPE.0 as u32 => (KIND_CANCEL, 0),
        _ => {
            // SAFETY: reading the keyboard state and layout; no pointers held.
            let character = unsafe { char_for(vk) }?;
            if character.is_control() {
                return None;
            }
            (KIND_CHAR, character as usize)
        }
    };
    Some(key)
}

/// Decodes a posted `EDIT_KEY` message back into an `EditKey`.
pub(crate) fn decode(kind: usize, character: isize) -> Option<EditKey> {
    Some(match kind {
        KIND_CHAR => EditKey::Char(char::from_u32(character as u32)?),
        KIND_BACKSPACE => EditKey::Backspace,
        KIND_DELETE => EditKey::Delete,
        KIND_LEFT => EditKey::Move {
            dir: MoveDir::Left,
            select: false,
        },
        KIND_RIGHT => EditKey::Move {
            dir: MoveDir::Right,
            select: false,
        },
        KIND_HOME => EditKey::Move {
            dir: MoveDir::Home,
            select: false,
        },
        KIND_END => EditKey::Move {
            dir: MoveDir::End,
            select: false,
        },
        KIND_SELECT_LEFT => EditKey::Move {
            dir: MoveDir::Left,
            select: true,
        },
        KIND_SELECT_RIGHT => EditKey::Move {
            dir: MoveDir::Right,
            select: true,
        },
        KIND_SELECT_HOME => EditKey::Move {
            dir: MoveDir::Home,
            select: true,
        },
        KIND_SELECT_END => EditKey::Move {
            dir: MoveDir::End,
            select: true,
        },
        KIND_SELECT_ALL => EditKey::SelectAll,
        KIND_COMMIT => EditKey::Commit,
        KIND_CANCEL => EditKey::Cancel,
        KIND_BACKSPACE_WORD => EditKey::BackspaceWord,
        KIND_DELETE_WORD => EditKey::DeleteWord,
        KIND_WORD_LEFT => EditKey::Move {
            dir: MoveDir::WordLeft,
            select: false,
        },
        KIND_WORD_RIGHT => EditKey::Move {
            dir: MoveDir::WordRight,
            select: false,
        },
        KIND_SELECT_WORD_LEFT => EditKey::Move {
            dir: MoveDir::WordLeft,
            select: true,
        },
        KIND_SELECT_WORD_RIGHT => EditKey::Move {
            dir: MoveDir::WordRight,
            select: true,
        },
        _ => return None,
    })
}

/// Decodes a posted `CLIPBOARD_KEY` wParam into a clipboard action.
pub(crate) fn clipboard_action(action: usize) -> Option<ClipboardAction> {
    Some(match action {
        ACTION_COPY => ClipboardAction::Copy,
        ACTION_CUT => ClipboardAction::Cut,
        ACTION_PASTE => ClipboardAction::Paste,
        _ => return None,
    })
}

/// Decodes a posted `SELECTION_KEY` wParam into a selection key.
pub(crate) fn selection_key(action: usize) -> Option<SelectionKey> {
    Some(match action {
        SEL_OPEN => SelectionKey::Open,
        SEL_CLEAR => SelectionKey::Clear,
        SEL_PERMANENT_DELETE => SelectionKey::PermanentDelete,
        SEL_REFRESH => SelectionKey::Refresh,
        SEL_MENU => SelectionKey::Menu,
        SEL_HOME => SelectionKey::Home,
        SEL_END => SelectionKey::End,
        SEL_PAGE_UP => SelectionKey::PageUp,
        SEL_PAGE_DOWN => SelectionKey::PageDown,
        _ => return None,
    })
}

/// Sets or clears one status bit for `key` in a keyboard-state array.
fn set_bit(state: &mut [u8; 256], key: VIRTUAL_KEY, mask: u8, on: bool) {
    let byte = &mut state[key.0 as usize];
    if on {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}

/// The character a key produces under the current layout and modifiers, or
/// `None`. The space's thread never receives keyboard messages (spaces are
/// never activated), so `GetKeyboardState` is frozen and its modifier bits
/// cannot be trusted. Each modifier is read from live key state and set or
/// cleared, so a released key cannot stay down. Caps Lock comes from
/// `GetKeyState`'s toggle bit: `GetAsyncKeyState`'s low bit is a separate,
/// unreliable "pressed since the last call" flag, not the toggle.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate
unsafe fn char_for(vk: u32) -> Option<char> {
    let mut state = [0u8; 256];
    // SAFETY: `state` is a valid 256-byte buffer.
    unsafe { GetKeyboardState(&mut state) }.ok()?;
    state[(vk & 0xFF) as usize] |= 0x80;
    for key in [
        VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_CONTROL, VK_LMENU, VK_RMENU,
    ] {
        // SAFETY: plain key-state query.
        let down = unsafe { GetAsyncKeyState(key.0 as i32) } < 0;
        set_bit(&mut state, key, 0x80, down);
    }
    // Caps Lock is a toggle: its state byte's low bit. Read it from
    // `GetKeyState`, whose low bit is the toggle, not the pressed bit.
    // SAFETY: plain key-state query.
    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0;
    set_bit(&mut state, VK_CAPITAL, 0x01, caps);
    // SAFETY: thread-local layout query; no pointers held.
    let layout = unsafe { GetKeyboardLayout(0) };
    let mut buffer = [0u16; 8];
    // SAFETY: `state` and `buffer` are valid for the call; a dead key returns
    // a negative count and is ignored.
    let count = unsafe { ToUnicodeEx(vk, 0, &state, &mut buffer, 0, Some(layout)) };
    if count <= 0 {
        return None;
    }
    char::decode_utf16(buffer[..count as usize].iter().copied())
        .next()
        .and_then(Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_bit_sets_and_clears_a_key_status_byte() {
        let mut state = [0u8; 256];
        set_bit(&mut state, VK_SHIFT, 0x80, true);
        assert_eq!(state[VK_SHIFT.0 as usize], 0x80);
        set_bit(&mut state, VK_SHIFT, 0x80, false);
        assert_eq!(state[VK_SHIFT.0 as usize], 0x00);
        set_bit(&mut state, VK_CAPITAL, 0x01, true);
        assert_eq!(state[VK_CAPITAL.0 as usize], 0x01);
        // Setting the pressed bit must not disturb the toggle bit and back.
        state[VK_CAPITAL.0 as usize] = 0x01;
        set_bit(&mut state, VK_CAPITAL, 0x80, true);
        assert_eq!(state[VK_CAPITAL.0 as usize], 0x81);
        set_bit(&mut state, VK_CAPITAL, 0x01, false);
        assert_eq!(state[VK_CAPITAL.0 as usize], 0x80);
    }

    #[test]
    fn decodes_posted_kinds_to_edit_keys() {
        assert_eq!(decode(KIND_CHAR, 'x' as isize), Some(EditKey::Char('x')));
        assert_eq!(decode(KIND_BACKSPACE, 0), Some(EditKey::Backspace));
        assert_eq!(decode(KIND_DELETE, 0), Some(EditKey::Delete));
        assert_eq!(decode(KIND_COMMIT, 0), Some(EditKey::Commit));
        assert_eq!(decode(KIND_CANCEL, 0), Some(EditKey::Cancel));
        assert_eq!(decode(KIND_SELECT_ALL, 0), Some(EditKey::SelectAll));
        assert_eq!(decode(KIND_BACKSPACE_WORD, 0), Some(EditKey::BackspaceWord));
        assert_eq!(decode(KIND_DELETE_WORD, 0), Some(EditKey::DeleteWord));
        assert_eq!(
            decode(KIND_WORD_LEFT, 0),
            Some(EditKey::Move {
                dir: MoveDir::WordLeft,
                select: false
            })
        );
        assert_eq!(
            decode(KIND_SELECT_WORD_RIGHT, 0),
            Some(EditKey::Move {
                dir: MoveDir::WordRight,
                select: true
            })
        );
        assert_eq!(
            decode(KIND_SELECT_LEFT, 0),
            Some(EditKey::Move {
                dir: MoveDir::Left,
                select: true
            })
        );
        assert_eq!(
            decode(KIND_RIGHT, 0),
            Some(EditKey::Move {
                dir: MoveDir::Right,
                select: false
            })
        );
        assert_eq!(decode(999, 0), None);
    }

    #[test]
    fn decodes_clipboard_actions() {
        assert_eq!(clipboard_action(ACTION_COPY), Some(ClipboardAction::Copy));
        assert_eq!(clipboard_action(ACTION_CUT), Some(ClipboardAction::Cut));
        assert_eq!(clipboard_action(ACTION_PASTE), Some(ClipboardAction::Paste));
        assert_eq!(clipboard_action(99), None);
    }

    #[test]
    fn decodes_selection_keys() {
        assert_eq!(selection_key(SEL_OPEN), Some(SelectionKey::Open));
        assert_eq!(selection_key(SEL_CLEAR), Some(SelectionKey::Clear));
        assert_eq!(
            selection_key(SEL_PERMANENT_DELETE),
            Some(SelectionKey::PermanentDelete)
        );
        assert_eq!(selection_key(SEL_REFRESH), Some(SelectionKey::Refresh));
        assert_eq!(selection_key(SEL_MENU), Some(SelectionKey::Menu));
        assert_eq!(selection_key(SEL_HOME), Some(SelectionKey::Home));
        assert_eq!(selection_key(SEL_END), Some(SelectionKey::End));
        assert_eq!(selection_key(SEL_PAGE_UP), Some(SelectionKey::PageUp));
        assert_eq!(selection_key(SEL_PAGE_DOWN), Some(SelectionKey::PageDown));
        assert_eq!(selection_key(99), None);
    }

    #[test]
    fn decodes_nav_keys_with_modifiers() {
        assert_eq!(
            nav_key(NAV_LEFT),
            Some(NavKey {
                dir: NavDir::Left,
                extend: false,
                focus: false
            })
        );
        assert_eq!(
            nav_key(NAV_DOWN | (1 << 2)),
            Some(NavKey {
                dir: NavDir::Down,
                extend: true,
                focus: false
            })
        );
        assert_eq!(
            nav_key(NAV_RIGHT | (1 << 3)),
            Some(NavKey {
                dir: NavDir::Right,
                extend: false,
                focus: true
            })
        );
        assert_eq!(arrow_dir(VK_UP.0 as u32), Some(NAV_UP));
        assert_eq!(arrow_dir(VK_A.0 as u32), None);
    }

    #[test]
    fn system_keys_cancel_the_rename() {
        for key in [VK_LWIN, VK_RWIN, VK_LMENU, VK_RMENU, VK_APPS] {
            assert!(
                is_system_key(key.0 as u32),
                "{key:?} should be a system key"
            );
        }
        assert!(!is_system_key(VK_A.0 as u32));
        assert!(!is_system_key(VK_BACK.0 as u32));
    }

    #[test]
    fn classify_consumes_editing_keys_and_ignores_others() {
        assert_eq!(classify(VK_BACK.0 as u32, false), Some((KIND_BACKSPACE, 0)));
        assert_eq!(
            classify(VK_LEFT.0 as u32, true),
            Some((KIND_SELECT_LEFT, 0))
        );
        assert_eq!(classify(VK_RETURN.0 as u32, false), Some((KIND_COMMIT, 0)));
        assert_eq!(classify(VK_ESCAPE.0 as u32, false), Some((KIND_CANCEL, 0)));
    }
}
