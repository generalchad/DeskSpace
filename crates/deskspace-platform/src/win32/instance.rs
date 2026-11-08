//! One DeskSpace per settings file, so a second launch hands off to the
//! running instance instead of starting a second copy over it.
//!
//! The key names both the `Local\` mutex that claims the settings file and
//! the controller window's title, which a second launch uses to find it. A
//! scratch `--config` therefore gets its own key and runs beside the real
//! one.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, HANDLE, SetLastError,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;

use crate::win32::names::upper_name;
use crate::win32::system::wide;

/// FNV-1a's offset basis and prime, written out because `DefaultHasher` may
/// change between Rust versions, and so between an installed build and its
/// update.
/// https://datatracker.ietf.org/doc/html/draft-eastlake-fnv
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

/// The key for `config`: `DeskSpace-` and the FNV-1a hash of the absolute
/// path's uppercase text, as 16 lowercase hex digits.
///
/// The path is never looked up on disk, so a settings file that does not
/// exist yet keys the same as one that does. Case and separators do not
/// matter, because Windows does not distinguish them in a path.
pub fn instance_key(config: &Path) -> String {
    let absolute = std::path::absolute(config).unwrap_or_else(|_| config.to_path_buf());
    let upper = upper_name(absolute.as_os_str());
    let units: Vec<u16> = upper
        .encode_wide()
        .map(|unit| {
            if unit == u16::from(b'/') {
                u16::from(b'\\')
            } else {
                unit
            }
        })
        .collect();
    format!("DeskSpace-{:016x}", fnv1a(&units))
}

/// A 64-bit FNV-1a hash over the bytes of `input`'s UTF-16 units, low byte
/// first, as Windows stores them.
fn fnv1a(input: &[u16]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for unit in input {
        for byte in unit.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Claims the settings file's instance; `None` when another process holds it.
/// The guard keeps the claim for as long as it lives.
///
/// A mutex that cannot be created is logged and treated as unclaimed, so a
/// failure in the claim never leaves DeskSpace unable to start.
pub fn claim(config: &Path) -> Option<InstanceGuard> {
    let name = wide(&format!("Local\\{}", instance_key(config)));
    // SAFETY: `name` is NUL-terminated. Last error is cleared first, because
    // `CreateMutexW` reports an existing mutex only through it.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        match CreateMutexW(None, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(handle);
                    return None;
                }
                Some(InstanceGuard(Some(handle)))
            }
            Err(error) => {
                tracing::warn!(%error, "could not claim the settings file's instance; starting anyway");
                Some(InstanceGuard(None))
            }
        }
    }
}

/// Keeps the settings file's mutex taken for as long as it lives.
pub struct InstanceGuard(Option<HANDLE>);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            // SAFETY: the handle came from `CreateMutexW` above and is closed
            // exactly once, here.
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const REAL: &str = r"C:\Users\me\AppData\Roaming\DeskSpace\DeskSpace\config\deskspace.toml";
    const SCRATCH: &str = r"C:\Users\me\AppData\Local\Temp\deskspace-dev\deskspace.toml";

    #[test]
    fn the_hash_of_no_input_is_the_offset_basis() {
        assert_eq!(fnv1a(&[]), FNV_OFFSET_BASIS);
        assert_eq!(fnv1a(&[]), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn two_spellings_of_one_path_give_one_key() {
        assert_eq!(
            instance_key(Path::new(REAL)),
            instance_key(Path::new(
                r"c:\users\ME\appdata\roaming\deskspace\DESKSPACE\config\deskspace.TOML"
            ))
        );
        assert_eq!(
            instance_key(Path::new(REAL)),
            instance_key(Path::new(
                r"C:/Users/me/AppData/Roaming/DeskSpace/DeskSpace/config/deskspace.toml"
            ))
        );
    }

    #[test]
    fn two_paths_give_two_keys() {
        assert_ne!(
            instance_key(Path::new(r"C:\One\deskspace.toml")),
            instance_key(Path::new(r"C:\Two\deskspace.toml"))
        );
    }

    #[test]
    fn a_scratch_config_differs_from_the_real_one() {
        assert_ne!(
            instance_key(Path::new(REAL)),
            instance_key(Path::new(SCRATCH))
        );
    }

    #[test]
    fn a_key_names_the_instance_in_lowercase_hex() {
        let key = instance_key(Path::new(REAL));
        let hash = key
            .strip_prefix("DeskSpace-")
            .expect("the key names its instance");
        assert_eq!(hash.len(), 16, "{key}");
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "{key}"
        );
    }

    #[test]
    fn one_claim_holds_the_settings_file_until_the_guard_drops() {
        let path = PathBuf::from(format!(
            r"C:\DeskSpace-test-{}\deskspace.toml",
            std::process::id()
        ));
        let guard = claim(&path).expect("the first claim succeeds");
        assert!(
            claim(&path).is_none(),
            "a second claim must wait for the first"
        );
        drop(guard);
        assert!(claim(&path).is_some(), "the claim is freed with the guard");
    }
}
