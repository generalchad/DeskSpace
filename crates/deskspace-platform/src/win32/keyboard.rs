//! Low-level keyboard capture for inline title editing and item keys.
//!
//! Spaces are never activated, so they cannot receive `WM_CHAR` or the usual
//! clipboard/Delete accelerators. While the app is renaming a space it installs
//! a `WH_KEYBOARD_LL` hook, translates each key press into an `EditKey`, posts
//! it to the controller, and consumes the event so it does not reach the
//! focused app. Outside a rename the same hook can watch keys for the current
//! selection or space instead of editing; `fn watched_key` is the source of truth
//! for which key needs which flag, and the docs/ENGINEERING.md "Keys" table repeats it. The
//! hook runs for every key in the session and Windows drops it if this thread is
//! slow to pump messages (`LowLevelHooksTimeout`), so it is installed only
//! while one of those modes is active.
//! https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc

use std::cell::Cell;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VIRTUAL_KEY,
    VK_0, VK_A, VK_ADD, VK_APPS, VK_BACK, VK_C, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_F2, VK_F5, VK_F10, VK_HOME, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_NEXT, VK_NUMPAD0, VK_OEM_MINUS, VK_OEM_PLUS, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SUBTRACT, VK_UP, VK_V, VK_X, VK_Y, VK_Z,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GetAncestor, GetForegroundWindow, HHOOK, KBDLLHOOKSTRUCT,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

use crate::edit::{EditKey, MoveDir};
use crate::input::{ClipboardAction, NavDir, NavKey, SelectionKey};
use crate::win32::{desktop_shell, messages};

pub(crate) const SEL_OPEN: usize = 0;
pub(crate) const SEL_CLEAR: usize = 1;
pub(crate) const SEL_PERMANENT_DELETE: usize = 2;
pub(crate) const SEL_REFRESH: usize = 3;
pub(crate) const SEL_MENU: usize = 4;
pub(crate) const SEL_HOME: usize = 5;
pub(crate) const SEL_END: usize = 6;
pub(crate) const SEL_PAGE_UP: usize = 7;
pub(crate) const SEL_PAGE_DOWN: usize = 8;
pub(crate) const SEL_ZOOM_IN: usize = 9;
pub(crate) const SEL_ZOOM_OUT: usize = 10;
pub(crate) const SEL_ZOOM_RESET: usize = 11;
pub(crate) const SEL_UNDO: usize = 12;
pub(crate) const SEL_REDO: usize = 13;

/// Direction codes packed into `NAV_KEY`'s wParam low bits.
pub(crate) const NAV_LEFT: usize = 0;
pub(crate) const NAV_RIGHT: usize = 1;
pub(crate) const NAV_UP: usize = 2;
pub(crate) const NAV_DOWN: usize = 3;
/// Shift: extend the selection along the move.
const NAV_EXTEND: usize = 1 << 2;
/// Ctrl: move only the focus, leaving the selection.
const NAV_FOCUS: usize = 1 << 3;

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

/// The hook's flags now: editing, watching a selection, and watching a
/// current space (`WATCH_PASTE`), for the debug window.
pub(crate) fn flags() -> (bool, bool, bool) {
    (EDITING.get(), WATCH_SELECTION.get(), WATCH_PASTE.get())
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

/// The modifiers held with a watched key.
#[derive(Clone, Copy, Debug, Default)]
struct Mods {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

impl Mods {
    fn current() -> Self {
        Self {
            ctrl: key_down(VK_CONTROL),
            shift: key_down(VK_SHIFT),
            alt: key_down(VK_MENU),
        }
    }
}

/// Which watch flags are set (the docs/ENGINEERING.md Keys table's columns).
#[derive(Clone, Copy, Debug, Default)]
struct Watch {
    /// An item is selected (`WATCH_SELECTION`).
    selection: bool,
    /// A space is current (`WATCH_PASTE`).
    paste: bool,
}

impl Watch {
    fn any(self) -> bool {
        self.selection || self.paste
    }
}

/// Handles one key press. Returns whether the event was consumed; when it
/// returns `false`, the caller passes the event on unchanged.
fn handle(event: &KBDLLHOOKSTRUCT) -> bool {
    // An open menu owns every key (Esc to close, Enter to activate, arrows to
    // navigate), so pass them all through while one is tracked.
    if MENU_OPEN.get() {
        return false;
    }
    let watch = Watch {
        selection: WATCH_SELECTION.get(),
        paste: WATCH_PASTE.get(),
    };
    if watch.any() && !EDITING.get() {
        return handle_watched(event.vkCode, watch);
    }
    handle_editing(event.vkCode)
}

/// Posts the message for a key the watch routes. The selection is kept while
/// another app is focused, as Explorer keeps a desktop selection, so these
/// keys belong to a space only while the desktop shell has the foreground;
/// otherwise an app's own Ctrl+C/V is stolen.
fn handle_watched(vk: u32, watch: Watch) -> bool {
    if !desktop_is_foreground() {
        return false;
    }
    let mods = Mods::current();
    if let Some((message, wparam)) = watched_key(vk, mods, watch) {
        post_message(message, wparam, 0);
        return true;
    }
    // Type-to-select reads the keyboard layout, so it is not part of the pure
    // table; it is the last key a current space takes, never with Ctrl.
    if watch.paste
        && !mods.ctrl
        && let Some(character) = typeahead_char(vk)
    {
        post_message(messages::TYPE_KEY, character as usize, 0);
        return true;
    }
    false
}

/// The message and `wparam` the watch posts for `vk`, or `None` to pass the
/// key through. This is the source of truth for the docs/ENGINEERING.md Keys
/// table, except type-to-select (`handle_watched`).
fn watched_key(vk: u32, mods: Mods, watch: Watch) -> Option<(u32, usize)> {
    if !watch.any() {
        return None;
    }
    if mods.alt && is_key(vk, VK_RETURN) {
        return Some((messages::PROPERTIES_KEY, 0));
    }
    if mods.ctrl {
        return watched_ctrl_key(vk, watch);
    }
    if watch.selection && is_key(vk, VK_RETURN) {
        return Some((messages::SELECTION_KEY, SEL_OPEN));
    }
    if is_key(vk, VK_F2) {
        return Some((messages::RENAME_KEY, 0));
    }
    if watch.selection && is_key(vk, VK_DELETE) {
        return Some(if mods.shift {
            (messages::SELECTION_KEY, SEL_PERMANENT_DELETE)
        } else {
            (messages::DELETE_KEY, 0)
        });
    }
    if is_key(vk, VK_ESCAPE) {
        return Some((messages::SELECTION_KEY, SEL_CLEAR));
    }
    if is_key(vk, VK_F5) {
        return Some((messages::SELECTION_KEY, SEL_REFRESH));
    }
    // The Menu key and Shift+F10 open the context menu, as on the desktop.
    if is_key(vk, VK_APPS) || (mods.shift && is_key(vk, VK_F10)) {
        return Some((messages::SELECTION_KEY, SEL_MENU));
    }
    // Scrolling and arrows need a current space, not just a selection (the
    // paste watch tracks that).
    if !watch.paste {
        return None;
    }
    if let Some(dir) = arrow_dir(vk) {
        let extend = if mods.shift { NAV_EXTEND } else { 0 };
        return Some((messages::NAV_KEY, dir | extend));
    }
    scroll_key(vk).map(|scroll| (messages::SELECTION_KEY, scroll))
}

/// `watched_key` for a key pressed with Ctrl.
fn watched_ctrl_key(vk: u32, watch: Watch) -> Option<(u32, usize)> {
    if is_key(vk, VK_A) {
        return Some((messages::SELECT_ALL_KEY, 0));
    }
    let clipboard = if watch.selection && is_key(vk, VK_C) {
        Some(ACTION_COPY)
    } else if watch.selection && is_key(vk, VK_X) {
        Some(ACTION_CUT)
    } else if watch.paste && is_key(vk, VK_V) {
        Some(ACTION_PASTE)
    } else {
        None
    };
    if let Some(action) = clipboard {
        return Some((messages::CLIPBOARD_KEY, action));
    }
    if watch.paste && is_key(vk, VK_Z) {
        return Some((messages::SELECTION_KEY, SEL_UNDO));
    }
    if watch.paste && is_key(vk, VK_Y) {
        return Some((messages::SELECTION_KEY, SEL_REDO));
    }
    // Ctrl+arrow moves the focus only, leaving the selection.
    if watch.paste
        && let Some(dir) = arrow_dir(vk)
    {
        return Some((messages::NAV_KEY, dir | NAV_FOCUS));
    }
    zoom_key(vk).map(|zoom| (messages::SELECTION_KEY, zoom))
}

/// The `SEL_*` scroll code for Home, End, PageUp, and PageDown.
fn scroll_key(vk: u32) -> Option<usize> {
    if is_key(vk, VK_HOME) {
        Some(SEL_HOME)
    } else if is_key(vk, VK_END) {
        Some(SEL_END)
    } else if is_key(vk, VK_PRIOR) {
        Some(SEL_PAGE_UP)
    } else if is_key(vk, VK_NEXT) {
        Some(SEL_PAGE_DOWN)
    } else {
        None
    }
}

/// Handles a key during an inline rename, posting it as an `EDIT_KEY`.
fn handle_editing(vk: u32) -> bool {
    // Alt and Windows combinations belong to the system (Alt+Tab, Win+D,
    // Start menu): cancel the rename and let them through.
    if key_down(VK_MENU) || key_down(VK_LWIN) || key_down(VK_RWIN) || is_system_key(vk) {
        post(KIND_CANCEL, 0);
        return false;
    }
    if key_down(VK_CONTROL) {
        if is_key(vk, VK_A) {
            post(KIND_SELECT_ALL, 0);
            return true;
        }
        let shift = key_down(VK_SHIFT);
        let kind = if vk == VK_LEFT.0 as u32 {
            Some(if shift {
                KIND_SELECT_WORD_LEFT
            } else {
                KIND_WORD_LEFT
            })
        } else if vk == VK_RIGHT.0 as u32 {
            Some(if shift {
                KIND_SELECT_WORD_RIGHT
            } else {
                KIND_WORD_RIGHT
            })
        } else if vk == VK_BACK.0 as u32 {
            Some(KIND_BACKSPACE_WORD)
        } else if vk == VK_DELETE.0 as u32 {
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

    match classify(vk, key_down(VK_SHIFT)) {
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
        extend: action & NAV_EXTEND != 0,
        focus: action & NAV_FOCUS != 0,
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
    post_message(messages::EDIT_KEY, kind, ch as isize);
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
/// The zoom a key pressed with Ctrl asks for, as browsers and Explorer
/// windows map it: Plus (the `=`/`+` key or the keypad's) zooms in, Minus
/// zooms out, and 0 resets.
fn zoom_key(vk: u32) -> Option<usize> {
    if is_key(vk, VK_OEM_PLUS) || is_key(vk, VK_ADD) {
        Some(SEL_ZOOM_IN)
    } else if is_key(vk, VK_OEM_MINUS) || is_key(vk, VK_SUBTRACT) {
        Some(SEL_ZOOM_OUT)
    } else if is_key(vk, VK_0) || is_key(vk, VK_NUMPAD0) {
        Some(SEL_ZOOM_RESET)
    } else {
        None
    }
}

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
        SEL_ZOOM_IN => SelectionKey::ZoomIn,
        SEL_ZOOM_OUT => SelectionKey::ZoomOut,
        SEL_ZOOM_RESET => SelectionKey::ZoomReset,
        SEL_UNDO => SelectionKey::Undo,
        SEL_REDO => SelectionKey::Redo,
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
        assert_eq!(selection_key(SEL_ZOOM_IN), Some(SelectionKey::ZoomIn));
        assert_eq!(selection_key(SEL_ZOOM_OUT), Some(SelectionKey::ZoomOut));
        assert_eq!(selection_key(SEL_ZOOM_RESET), Some(SelectionKey::ZoomReset));
        assert_eq!(selection_key(SEL_UNDO), Some(SelectionKey::Undo));
        assert_eq!(selection_key(SEL_REDO), Some(SelectionKey::Redo));
        assert_eq!(selection_key(99), None);
    }

    #[test]
    fn plus_minus_and_zero_map_to_zoom_on_both_keyboards() {
        let code = |key: VIRTUAL_KEY| key.0 as u32;
        assert_eq!(zoom_key(code(VK_OEM_PLUS)), Some(SEL_ZOOM_IN));
        assert_eq!(zoom_key(code(VK_ADD)), Some(SEL_ZOOM_IN));
        assert_eq!(zoom_key(code(VK_OEM_MINUS)), Some(SEL_ZOOM_OUT));
        assert_eq!(zoom_key(code(VK_SUBTRACT)), Some(SEL_ZOOM_OUT));
        assert_eq!(zoom_key(code(VK_0)), Some(SEL_ZOOM_RESET));
        assert_eq!(zoom_key(code(VK_NUMPAD0)), Some(SEL_ZOOM_RESET));
        assert_eq!(zoom_key(code(VK_A)), None);
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

    const PLAIN: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        ..PLAIN
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ..PLAIN
    };
    const ALT: Mods = Mods { alt: true, ..PLAIN };
    const NO_WATCH: Watch = Watch {
        selection: false,
        paste: false,
    };
    const SELECTION: Watch = Watch {
        selection: true,
        paste: false,
    };
    const PASTE: Watch = Watch {
        selection: false,
        paste: true,
    };

    type Case = (VIRTUAL_KEY, Mods, (u32, usize));

    /// Asserts each case is routed under every flag in `routed` and passed
    /// through under every flag in `ignored`.
    fn assert_routes(cases: &[Case], routed: &[Watch], ignored: &[Watch]) {
        for &(key, mods, expected) in cases {
            for &watch in routed {
                assert_eq!(
                    watched_key(key.0 as u32, mods, watch),
                    Some(expected),
                    "{key:?} {mods:?} under {watch:?}"
                );
            }
            for &watch in ignored {
                assert_eq!(
                    watched_key(key.0 as u32, mods, watch),
                    None,
                    "{key:?} {mods:?} under {watch:?}"
                );
            }
        }
    }

    #[test]
    fn keys_for_either_watch_flag() {
        let sel = |code| (messages::SELECTION_KEY, code);
        assert_routes(
            &[
                (VK_F2, PLAIN, (messages::RENAME_KEY, 0)),
                (VK_RETURN, ALT, (messages::PROPERTIES_KEY, 0)),
                (VK_A, CTRL, (messages::SELECT_ALL_KEY, 0)),
                (VK_ESCAPE, PLAIN, sel(SEL_CLEAR)),
                (VK_F5, PLAIN, sel(SEL_REFRESH)),
                (VK_APPS, PLAIN, sel(SEL_MENU)),
                (VK_F10, SHIFT, sel(SEL_MENU)),
                (VK_OEM_PLUS, CTRL, sel(SEL_ZOOM_IN)),
                (VK_ADD, CTRL, sel(SEL_ZOOM_IN)),
                (VK_OEM_MINUS, CTRL, sel(SEL_ZOOM_OUT)),
                (VK_SUBTRACT, CTRL, sel(SEL_ZOOM_OUT)),
                (VK_0, CTRL, sel(SEL_ZOOM_RESET)),
                (VK_NUMPAD0, CTRL, sel(SEL_ZOOM_RESET)),
            ],
            &[SELECTION, PASTE],
            &[NO_WATCH],
        );
    }

    #[test]
    fn keys_for_the_selection_watch() {
        assert_routes(
            &[
                (VK_RETURN, PLAIN, (messages::SELECTION_KEY, SEL_OPEN)),
                (VK_DELETE, PLAIN, (messages::DELETE_KEY, 0)),
                (
                    VK_DELETE,
                    SHIFT,
                    (messages::SELECTION_KEY, SEL_PERMANENT_DELETE),
                ),
                (VK_C, CTRL, (messages::CLIPBOARD_KEY, ACTION_COPY)),
                (VK_X, CTRL, (messages::CLIPBOARD_KEY, ACTION_CUT)),
            ],
            &[SELECTION],
            &[PASTE, NO_WATCH],
        );
    }

    #[test]
    fn keys_for_the_paste_watch() {
        let nav = |code| (messages::NAV_KEY, code);
        let sel = |code| (messages::SELECTION_KEY, code);
        assert_routes(
            &[
                (VK_V, CTRL, (messages::CLIPBOARD_KEY, ACTION_PASTE)),
                (VK_Z, CTRL, sel(SEL_UNDO)),
                (VK_Y, CTRL, sel(SEL_REDO)),
                (VK_LEFT, PLAIN, nav(NAV_LEFT)),
                (VK_RIGHT, PLAIN, nav(NAV_RIGHT)),
                (VK_UP, PLAIN, nav(NAV_UP)),
                (VK_DOWN, SHIFT, nav(NAV_DOWN | NAV_EXTEND)),
                (VK_UP, CTRL, nav(NAV_UP | NAV_FOCUS)),
                (VK_HOME, PLAIN, sel(SEL_HOME)),
                (VK_END, PLAIN, sel(SEL_END)),
                (VK_PRIOR, PLAIN, sel(SEL_PAGE_UP)),
                (VK_NEXT, PLAIN, sel(SEL_PAGE_DOWN)),
            ],
            &[PASTE],
            &[SELECTION, NO_WATCH],
        );
    }

    #[test]
    fn other_keys_pass_through_to_type_to_select_or_the_app() {
        for watch in [SELECTION, PASTE] {
            // Typed characters are left to `handle_watched`'s type-to-select.
            assert_eq!(watched_key(VK_A.0 as u32, PLAIN, watch), None);
            assert_eq!(watched_key(VK_A.0 as u32, SHIFT, watch), None);
            // Ctrl, Alt, and Shift combinations the table does not name.
            assert_eq!(watched_key(VK_F2.0 as u32, CTRL, watch), None);
            assert_eq!(watched_key(VK_F10.0 as u32, PLAIN, watch), None);
        }
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
