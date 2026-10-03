//! Low-level keyboard capture for inline title editing.
//!
//! Spaces are never activated, so they cannot receive `WM_CHAR`. While the app
//! is renaming a space it installs a `WH_KEYBOARD_LL` hook, translates each
//! key press into an `EditKey`, posts it to the controller, and consumes the
//! event so it does not reach the focused app. The hook runs for every key in
//! the session and Windows drops it if this thread is slow to pump messages
//! (`LowLevelHooksTimeout`), so it is installed only while renaming.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc

use std::cell::Cell;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VIRTUAL_KEY, VK_APPS,
    VK_BACK, VK_CONTROL, VK_DELETE, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_LMENU, VK_LWIN,
    VK_MENU, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::edit::{EditKey, MoveDir};

/// Posted to the controller with the decoded key: wParam is a kind code and
/// lParam the character for `KIND_CHAR`.
pub(crate) const EDIT_KEY: u32 = WM_APP + 3;

/// Posted to the controller when Delete is pressed while an item is selected.
pub(crate) const DELETE_KEY: u32 = WM_APP + 4;

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

const VK_A_CODE: u32 = 0x41;

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static CONTROLLER: Cell<Option<HWND>> = const { Cell::new(None) };
    /// Capturing every editing keystroke (inline rename).
    static EDITING: Cell<bool> = const { Cell::new(false) };
    /// Watching only for Delete while an item is selected.
    static WATCH_DELETE: Cell<bool> = const { Cell::new(false) };
}

/// Starts or stops capturing keys for `controller` for inline editing.
/// Idempotent.
pub(crate) fn capture(controller: HWND, enabled: bool) {
    EDITING.set(enabled);
    refresh(controller);
}

/// Starts or stops watching for Delete while an item is selected. Idempotent.
pub(crate) fn watch_delete(controller: HWND, enabled: bool) {
    WATCH_DELETE.set(enabled);
    refresh(controller);
}

/// Installs or removes the hook so it is on exactly when a mode needs it.
fn refresh(controller: HWND) {
    let wanted = EDITING.get() || WATCH_DELETE.get();
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

/// Removes the hook and clears both modes.
pub(crate) fn stop() {
    if let Some(hook) = HOOK.take() {
        // SAFETY: `hook` was returned by SetWindowsHookExW on this thread.
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    CONTROLLER.set(None);
    EDITING.set(false);
    WATCH_DELETE.set(false);
}

/// Whether a modifier key is currently held.
fn key_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: plain key-state query.
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

/// Whether `vk` is `key`.
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
    if code < 0 {
        // SAFETY: every hook must pass the event on.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let message = wparam.0 as u32;
    if message != WM_KEYDOWN && message != WM_SYSKEYDOWN {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    // SAFETY: for a keyboard hook, lParam points at a KBDLLHOOKSTRUCT.
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    // Delete-only watch: while an item is selected, capture Delete and pass
    // every other key through untouched.
    if WATCH_DELETE.get() && !EDITING.get() {
        if is_key(event.vkCode, VK_DELETE) {
            post_message(DELETE_KEY, 0, 0);
            return LRESULT(1);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    // Alt and Windows combinations belong to the system (Alt+Tab, Win+D,
    // Start menu): cancel the rename and let them through.
    if key_down(VK_MENU) || key_down(VK_LWIN) || key_down(VK_RWIN) || is_system_key(event.vkCode) {
        post(KIND_CANCEL, 0);
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    if key_down(VK_CONTROL) {
        if event.vkCode == VK_A_CODE {
            post(KIND_SELECT_ALL, 0);
            return LRESULT(1);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    match classify(event.vkCode, key_down(VK_SHIFT)) {
        Some((kind, ch)) => {
            post(kind, ch);
            LRESULT(1)
        }
        // SAFETY: not an editing key; pass it on unchanged.
        None => unsafe { CallNextHookEx(None, code, wparam, lparam) },
    }
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
        _ => return None,
    })
}

/// The character a key produces under the current layout, or `None`.
unsafe fn char_for(vk: u32) -> Option<char> {
    let mut state = [0u8; 256];
    // SAFETY: `state` is a valid 256-byte buffer.
    unsafe { GetKeyboardState(&mut state) }.ok()?;
    // The hook runs before the key is queued; mark it down in the state we
    // pass, because the keyboard-state array may not include it yet.
    state[(vk & 0xFF) as usize] |= 0x80;
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
    fn decodes_posted_kinds_to_edit_keys() {
        assert_eq!(decode(KIND_CHAR, 'x' as isize), Some(EditKey::Char('x')));
        assert_eq!(decode(KIND_BACKSPACE, 0), Some(EditKey::Backspace));
        assert_eq!(decode(KIND_DELETE, 0), Some(EditKey::Delete));
        assert_eq!(decode(KIND_COMMIT, 0), Some(EditKey::Commit));
        assert_eq!(decode(KIND_CANCEL, 0), Some(EditKey::Cancel));
        assert_eq!(decode(KIND_SELECT_ALL, 0), Some(EditKey::SelectAll));
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
    fn system_keys_cancel_the_rename() {
        for key in [VK_LWIN, VK_RWIN, VK_LMENU, VK_RMENU, VK_APPS] {
            assert!(
                is_system_key(key.0 as u32),
                "{key:?} should be a system key"
            );
        }
        assert!(!is_system_key(VK_A_CODE));
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
