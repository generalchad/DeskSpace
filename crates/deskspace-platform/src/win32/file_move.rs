//! Moving a file or folder without ever replacing what is at the target.
//!
//! `std::fs::rename` replaces an existing file on Windows, so a name checked
//! as free can still be overwritten if a file appears there before the move.
//! `MoveFileExW` with no flags makes the check and the move one step.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
use windows::core::PCWSTR;

/// Moves `from` to `to`, a file or a folder, and fails if `to` exists.
///
/// No `MOVEFILE_REPLACE_EXISTING`, so an existing `to` fails the call with
/// `io::ErrorKind::AlreadyExists` and both keep their contents. No
/// `MOVEFILE_COPY_ALLOWED`, so a move to another volume fails with
/// `io::ErrorKind::CrossesDevices` instead of copying.
/// https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw
pub fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let from = long_path(from)?;
    let to = long_path(to)?;
    // SAFETY: both buffers are NUL-terminated UTF-16 that outlive the call,
    // which only reads them.
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVE_FILE_FLAGS(0),
        )
    }
    .map_err(io_error)
}

/// `path` made absolute and given the `\\?\` prefix, NUL-terminated, so the
/// call takes it at any length; the app has no long-path manifest, and
/// `std::fs::rename`, which this replaces, adds the prefix too. The prefix
/// turns off the API's own clean-up of `/`, `.`, and `..`, which
/// `std::path::absolute` has done already.
/// https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation
fn long_path(path: &Path) -> io::Result<Vec<u16>> {
    let absolute: Vec<u16> = std::path::absolute(path)?
        .as_os_str()
        .encode_wide()
        .collect();
    // A NUL inside would end the string early and name another file.
    if absolute.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a path cannot contain a NUL character",
        ));
    }
    let (prefix, rest) = if starts_with(&absolute, r"\\?\") || starts_with(&absolute, r"\\.\") {
        ("", &absolute[..])
    } else if starts_with(&absolute, r"\\") {
        // `\\server\share` becomes `\\?\UNC\server\share`.
        (r"\\?\UNC", &absolute[1..])
    } else {
        (r"\\?\", &absolute[..])
    };
    Ok(prefix
        .encode_utf16()
        .chain(rest.iter().copied())
        .chain(std::iter::once(0))
        .collect())
}

fn starts_with(units: &[u16], prefix: &str) -> bool {
    prefix
        .encode_utf16()
        .enumerate()
        .all(|(index, unit)| units.get(index) == Some(&unit))
}

/// The `io::Error` for a failed Win32 call. The `windows` crate's own
/// conversion passes the whole HRESULT as the OS error, so its `kind()` is
/// never `AlreadyExists` or `CrossesDevices`; the Win32 code is the HRESULT's
/// low word when its facility is `FACILITY_WIN32`.
/// https://learn.microsoft.com/en-us/windows/win32/api/winerror/nf-winerror-hresult_from_win32
fn io_error(error: windows::core::Error) -> io::Error {
    const WIN32_HRESULT: u32 = 0x8007_0000;
    let code = error.code().0 as u32;
    if code & 0xFFFF_0000 == WIN32_HRESULT {
        io::Error::from_raw_os_error((code & 0xFFFF) as i32)
    } else {
        io::Error::other(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh folder under the system temp folder, unique to `name` and this
    /// process, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("deskspace-move-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn wide_text(units: &[u16]) -> String {
        String::from_utf16(units.strip_suffix(&[0]).unwrap()).unwrap()
    }

    #[test]
    fn a_free_target_receives_the_file() {
        let dir = Scratch::new("free");
        let from = dir.path().join("a.txt");
        let to = dir.path().join("b.txt");
        std::fs::write(&from, "a").unwrap();

        move_no_replace(&from, &to).unwrap();

        assert!(!from.exists());
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "a");
    }

    #[test]
    fn an_existing_file_is_never_replaced() {
        let dir = Scratch::new("existing");
        let from = dir.path().join("a.txt");
        let to = dir.path().join("b.txt");
        std::fs::write(&from, "a").unwrap();
        std::fs::write(&to, "b").unwrap();

        let error = move_no_replace(&from, &to).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(std::fs::read_to_string(&from).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "b");
    }

    #[test]
    fn a_name_in_another_case_counts_as_taken() {
        let dir = Scratch::new("case");
        let from = dir.path().join("a.txt");
        let to = dir.path().join("B.TXT");
        std::fs::write(&from, "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();

        let error = move_no_replace(&from, &to).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
            "b"
        );
    }

    #[test]
    fn a_folder_moves_with_its_contents_and_never_onto_a_file() {
        let dir = Scratch::new("folder");
        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("inner.txt"), "x").unwrap();
        let taken = dir.path().join("taken");
        std::fs::write(&taken, "file").unwrap();

        let error = move_no_replace(&folder, &taken).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "file");

        let moved = dir.path().join("moved");
        move_no_replace(&folder, &moved).unwrap();
        assert_eq!(
            std::fs::read_to_string(moved.join("inner.txt")).unwrap(),
            "x"
        );
    }

    #[test]
    fn a_path_past_max_path_still_moves() {
        let dir = Scratch::new("long");
        let deep = dir.path().join("d".repeat(120)).join("e".repeat(120));
        std::fs::create_dir_all(&deep).unwrap();
        let from = deep.join(format!("{}.txt", "f".repeat(60)));
        let to = deep.join(format!("{}.txt", "g".repeat(60)));
        assert!(to.as_os_str().len() > 260);
        std::fs::write(&from, "long").unwrap();

        move_no_replace(&from, &to).unwrap();

        assert_eq!(std::fs::read_to_string(&to).unwrap(), "long");
    }

    #[test]
    fn a_missing_source_is_not_found() {
        let dir = Scratch::new("missing");
        let error = move_no_replace(&dir.path().join("none"), &dir.path().join("b")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
    }

    /// `StoreError::OtherVolume` relies on this mapping; a second volume is
    /// not something a test can count on.
    #[test]
    fn another_volume_reads_as_crosses_devices() {
        const ERROR_NOT_SAME_DEVICE: u32 = 17;
        let error =
            windows::core::Error::from(windows::core::HRESULT::from_win32(ERROR_NOT_SAME_DEVICE));
        assert_eq!(io_error(error).kind(), io::ErrorKind::CrossesDevices);
    }

    #[test]
    fn paths_get_the_verbatim_prefix_once() {
        assert_eq!(
            wide_text(&long_path(Path::new(r"C:\a\..\b\c.txt")).unwrap()),
            r"\\?\C:\b\c.txt"
        );
        assert_eq!(
            wide_text(&long_path(Path::new("C:/a/b.txt")).unwrap()),
            r"\\?\C:\a\b.txt"
        );
        assert_eq!(
            wide_text(&long_path(Path::new(r"\\server\share\b.txt")).unwrap()),
            r"\\?\UNC\server\share\b.txt"
        );
        assert_eq!(
            wide_text(&long_path(Path::new(r"\\?\C:\a\b.txt")).unwrap()),
            r"\\?\C:\a\b.txt"
        );
        assert_eq!(
            wide_text(&long_path(Path::new(r"\\?\UNC\server\share\b.txt")).unwrap()),
            r"\\?\UNC\server\share\b.txt"
        );
        assert_eq!(
            wide_text(&long_path(Path::new(r"\\.\C:\a\b.txt")).unwrap()),
            r"\\.\C:\a\b.txt"
        );
        let relative = std::env::current_dir().unwrap().join("b.txt");
        assert_eq!(
            wide_text(&long_path(Path::new("b.txt")).unwrap()),
            format!(r"\\?\{}", relative.display())
        );
    }

    #[test]
    fn a_path_with_a_nul_inside_is_refused() {
        let error = long_path(Path::new("C:\\a\0b.txt")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
