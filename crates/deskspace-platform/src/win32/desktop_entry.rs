//! Desktop integration outside the space windows: a per-user Shell verb that
//! puts "Show spaces" on the desktop's right-click menu while spaces are
//! hidden, and the signal a second launch posts to the running instance.
//!
//! The verb is a plain `DesktopBackground\Shell` entry, so Explorer shows it
//! without a shell-extension DLL; its command re-launches this executable with
//! the show flag. While an instance is running the flag finds the controller
//! window and posts to it instead of starting a second copy.
//! https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers

use windows::Win32::Foundation::{ERROR_SUCCESS, LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::win32::system::wide;
use crate::win32::{controller, messages};

/// The desktop background Shell verb that shows hidden spaces.
const SHOW_VERB: &str = r"Software\Classes\DesktopBackground\Shell\DeskSpaceShowSpaces";

/// Adds or removes the desktop menu entry that runs `command` (this executable
/// with the show flag), shown with `label`. Idempotent; an existing entry is
/// overwritten or removed.
pub fn set_desktop_show_entry(
    enabled: bool,
    label: &str,
    command: &str,
    icon: &str,
) -> Result<(), PlatformError> {
    let path = wide(SHOW_VERB);
    // SAFETY: the strings outlive each call, and every opened key is closed.
    unsafe {
        if !enabled {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()));
            return Ok(());
        }
        let key = create(HKEY_CURRENT_USER, &path)?;
        let result = set_string(key, None, label)
            .and_then(|()| set_string(key, Some("Icon"), icon))
            .and_then(|()| {
                // The `command` subkey's path is relative to the verb key, so
                // it must not repeat the full path.
                let command_key = create(key, &wide("command"))?;
                let result = set_string(command_key, None, command);
                let _ = RegCloseKey(command_key);
                result
            });
        let _ = RegCloseKey(key);
        result
    }
}

/// Posts the show request to a running DeskSpace instance, if one exists.
/// Returns whether a running controller window was found.
pub fn signal_show_spaces() -> bool {
    // SAFETY: plain window queries; the message id belongs to our controller.
    unsafe {
        let Ok(controller) = FindWindowW(controller::CLASS_NAME, PCWSTR::null()) else {
            return false;
        };
        if controller.is_invalid() {
            return false;
        }
        PostMessageW(
            Some(controller),
            messages::SHOW_SPACES,
            WPARAM(0),
            LPARAM(0),
        )
        .is_ok()
    }
}

/// Opens or creates `path` (NUL-terminated) under `parent`.
unsafe fn create(parent: HKEY, path: &[u16]) -> Result<HKEY, PlatformError> {
    let mut key = HKEY(std::ptr::null_mut());
    // SAFETY: `path` is NUL-terminated; `key` is a valid out-pointer.
    let status = unsafe {
        RegCreateKeyExW(
            parent,
            PCWSTR(path.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(PlatformError::Registry(status.0));
    }
    Ok(key)
}

/// Sets a `REG_SZ` value; a null `name` sets the key's default value.
unsafe fn set_string(key: HKEY, name: Option<&str>, value: &str) -> Result<(), PlatformError> {
    let value = wide(value);
    let name = name.map(wide);
    // SAFETY: `value` is NUL-terminated and its bytes are passed with the exact
    // length; `name` outlives the call when present.
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_deref()
                .map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr())),
            None,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                value.as_ptr().cast::<u8>(),
                value.len() * 2,
            )),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(PlatformError::Registry(status.0));
    }
    Ok(())
}
