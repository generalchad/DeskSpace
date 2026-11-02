//! Desktop integration outside the space windows: the per-user Shell verbs
//! that put DeskSpace's entries on the desktop's right-click menu, and the
//! signal a second launch posts to the running instance.
//!
//! Each verb is a plain `DesktopBackground\Shell` entry, so Explorer shows it
//! without a shell-extension DLL; its command re-launches this executable with
//! a flag. While an instance is running the flag finds the controller window
//! and posts to it instead of starting a second copy.
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

/// The prefix every DeskSpace verb shares under `DesktopBackground\Shell`.
const VERB_PREFIX: &str = "DeskSpace";

/// Every verb DeskSpace has written. A sync removes all of them first, so an
/// entry it no longer offers (Show spaces once the spaces are shown) does not
/// linger.
const KNOWN_VERBS: [&str; 3] = ["NewSpace", "Settings", "ShowSpaces"];

/// One entry on the desktop's background menu. `key` and `label` are fixed;
/// `command` and `icon` carry the running executable's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopVerb {
    pub key: &'static str,
    pub label: &'static str,
    pub command: String,
    pub icon: String,
}

/// Replaces every DeskSpace desktop verb with `verbs`. Idempotent: the known
/// keys are removed first, so an entry no longer offered does not linger.
pub fn set_desktop_verbs(verbs: &[DesktopVerb]) -> Result<(), PlatformError> {
    // SAFETY: the strings outlive each call, and every opened key is closed.
    unsafe {
        for key in KNOWN_VERBS {
            let path = verb_path(key);
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()));
        }
        let mut first_error = None;
        for verb in verbs {
            if let Err(error) = write_verb(verb)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

/// The full registry path of the verb `key`.
fn verb_path(key: &str) -> Vec<u16> {
    wide(&format!(
        r"Software\Classes\DesktopBackground\Shell\{VERB_PREFIX}{key}"
    ))
}

/// Writes one verb and its `command` subkey.
///
/// # Safety
/// The caller holds no registry handle across the call; every key opened here
/// is closed here.
unsafe fn write_verb(verb: &DesktopVerb) -> Result<(), PlatformError> {
    let path = verb_path(verb.key);
    // SAFETY: `path` is NUL-terminated and outlives the call.
    let key = unsafe { create(HKEY_CURRENT_USER, &path) }?;
    // SAFETY: `key` is live and closed below.
    let result = unsafe {
        set_string(key, None, verb.label)
            .and_then(|()| set_string(key, Some("Icon"), &verb.icon))
            .and_then(|()| {
                // The `command` subkey's path is relative to the verb key, so
                // it must not repeat the full path.
                let command_key = create(key, &wide("command"))?;
                let result = set_string(command_key, None, &verb.command);
                let _ = RegCloseKey(command_key);
                result
            })
    };
    // SAFETY: closing the key opened above.
    unsafe {
        let _ = RegCloseKey(key);
    }
    result
}

/// Posts `message` to a running DeskSpace instance, if one exists. Returns
/// whether a running controller window was found.
fn signal(message: u32) -> bool {
    // SAFETY: plain window queries; the message id belongs to our controller.
    unsafe {
        let Ok(controller) = FindWindowW(controller::CLASS_NAME, PCWSTR::null()) else {
            return false;
        };
        if controller.is_invalid() {
            return false;
        }
        PostMessageW(Some(controller), message, WPARAM(0), LPARAM(0)).is_ok()
    }
}

/// Posts the show request to a running DeskSpace instance.
pub fn signal_show_spaces() -> bool {
    signal(messages::SHOW_SPACES)
}

/// Posts the new-space request to a running DeskSpace instance.
pub fn signal_new_space() -> bool {
    signal(messages::NEW_SPACE)
}

/// Posts the open-settings request to a running DeskSpace instance.
pub fn signal_open_settings() -> bool {
    signal(messages::OPEN_SETTINGS)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_is_a_child_of_the_deskspace_key() {
        for key in KNOWN_VERBS {
            let path = String::from_utf16_lossy(&verb_path(key));
            let path = path.trim_end_matches('\0');
            assert!(
                path.starts_with(r"Software\Classes\DesktopBackground\Shell\DeskSpace"),
                "{path}"
            );
            assert!(path.ends_with(key), "{path}");
        }
    }
}
