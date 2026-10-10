//! The per-user `Run` key entry that starts DeskSpace at sign-in (F-34).
//! ROADMAP D6: a reinstall and the uninstaller both write/remove the same
//! value, so the name is fixed.
//! https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys

use std::path::Path;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::win32::system::wide;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// The command line written to the Run value for `exe`: the quoted path, so a
/// path with spaces survives.
pub fn run_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Adds or removes the per-user `Run` value that starts DeskSpace at sign-in.
/// The value is the current executable's quoted path, so it follows a moved
/// install. Idempotent.
pub fn set_start_with_windows(enabled: bool) -> Result<(), PlatformError> {
    // SAFETY: the strings outlive each call, and the opened key is closed.
    unsafe {
        let key = open_run_key()?;
        let result = if enabled {
            match std::env::current_exe() {
                Ok(exe) => set_string(key, &wide(&run_command(&exe))),
                Err(source) => Err(PlatformError::Io(source)),
            }
        } else {
            delete_value(key)
        };
        let _ = RegCloseKey(key);
        result
    }
}

/// Opens (or creates) the per-user `Run` key with write access.
unsafe fn open_run_key() -> Result<HKEY, PlatformError> {
    let path = wide(RUN_KEY);
    let mut key = HKEY(std::ptr::null_mut());
    // SAFETY: `path` is NUL-terminated; `key` is a valid out-pointer.
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
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

unsafe fn set_string(key: HKEY, value: &[u16]) -> Result<(), PlatformError> {
    // SAFETY: `value` is NUL-terminated and passed with its exact byte length.
    let status = unsafe {
        RegSetValueExW(
            key,
            windows::core::w!("DeskSpace"),
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

unsafe fn delete_value(key: HKEY) -> Result<(), PlatformError> {
    // SAFETY: `key` is a live key opened with KEY_SET_VALUE.
    let status = unsafe { RegDeleteValueW(key, windows::core::w!("DeskSpace")) };
    if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
        return Err(PlatformError::Registry(status.0));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_command_quotes_the_path() {
        assert_eq!(
            run_command(Path::new(r"C:\Program Files\DeskSpace\deskspace.exe")),
            r#""C:\Program Files\DeskSpace\deskspace.exe""#
        );
    }
}
