//! Comparing file names the way Windows does: ordinal, one character at a
//! time, so a name written in another case still matches.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf, Prefix};

use windows::Win32::Foundation::LPARAM;
use windows::Win32::Globalization::{
    CSTR_EQUAL, CompareStringOrdinal, LCMAP_UPPERCASE, LCMapStringEx, LOCALE_NAME_INVARIANT,
};

/// Whether `a` and `b` are the same file name, ignoring case the way Windows
/// does: `É` matches `é`, but `ß` does not match `SS`, because the comparison
/// maps one character at a time instead of expanding a case fold. Microsoft
/// documents this as the function to compare file names with.
/// https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal
pub fn same_name_ignoring_case(a: &OsStr, b: &OsStr) -> bool {
    let a: Vec<u16> = a.encode_wide().collect();
    let b: Vec<u16> = b.encode_wide().collect();
    // SAFETY: both slices are valid UTF-16 of the length the call is given,
    // and it only reads them.
    unsafe { CompareStringOrdinal(&a, &b, true) == CSTR_EQUAL }
}

/// `name` folded to uppercase with the invariant locale's mapping, which is
/// the form `same_name_ignoring_case` compares. Two names fold to the same
/// text when Windows treats them as the same name, so a key built from the
/// fold agrees with the comparison.
/// https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-lcmapstringex
pub fn upper_name(name: &OsStr) -> OsString {
    let source: Vec<u16> = name.encode_wide().collect();
    // SAFETY: `source` is valid UTF-16 of the length the call is given, and a
    // null destination only asks for the mapped length.
    let needed = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            &source,
            None,
            None,
            None,
            LPARAM(0),
        )
    };
    if needed <= 0 {
        return name.to_os_string();
    }
    let mut mapped = vec![0u16; needed as usize];
    // SAFETY: `mapped` holds `needed` elements, the length the call is given,
    // so it can write at most what the first call measured.
    let written = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            &source,
            Some(mapped.as_mut_slice()),
            None,
            None,
            LPARAM(0),
        )
    };
    if written <= 0 {
        return name.to_os_string();
    }
    mapped.truncate(written as usize);
    OsString::from_wide(&mapped)
}

/// Whether `path` is `root` or lies below it, comparing whole components by
/// the same rule Windows uses for names. Unlike a string-prefix test,
/// `...\DeskSpace.App` is not inside `...\DeskSpace`. A drive or share is the
/// same with or without the `\\?\` prefix that `canonicalize` and
/// `GetFinalPathNameByHandleW` add, so a final path compares with a plain one.
pub fn is_inside(path: &Path, root: &Path) -> bool {
    let mut path = path.components();
    root.components()
        .all(|root| path.next().is_some_and(|part| same_component(part, root)))
}

/// Whether `a` and `b` are on the same drive or share, with or without the
/// `\\?\` prefix. A relative path is on no volume.
pub fn same_volume(a: &Path, b: &Path) -> bool {
    match (a.components().next(), b.components().next()) {
        (Some(Component::Prefix(a)), Some(Component::Prefix(b))) => same_prefix(a.kind(), b.kind()),
        _ => false,
    }
}

/// Whether `path` is in a Recycle Bin: any of its folders is `$Recycle.Bin`,
/// in any case. Deleting a folder in Explorer moves it there.
pub fn in_recycle_bin(path: &Path) -> bool {
    path.components()
        .any(|part| same_name_ignoring_case(part.as_os_str(), OsStr::new("$Recycle.Bin")))
}

/// `path` without the `\\?\` prefix `GetFinalPathNameByHandleW` adds, so it
/// reads as the user would write it: `\\?\C:\A` is `C:\A`, and
/// `\\?\UNC\server\share\A` is `\\server\share\A`. Any other path is
/// returned as it is.
pub fn plain_path(path: &Path) -> PathBuf {
    let mut parts = path.components();
    let Some(Component::Prefix(prefix)) = parts.next() else {
        return path.to_path_buf();
    };
    let mut plain = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", char::from(drive))),
        Prefix::VerbatimUNC(server, share) => {
            let mut root = OsString::from(r"\\");
            root.push(server);
            root.push(r"\");
            root.push(share);
            root.push(r"\");
            PathBuf::from(root)
        }
        _ => return path.to_path_buf(),
    };
    plain.extend(parts.filter(|part| !matches!(part, Component::RootDir)));
    plain
}

fn same_component(a: Component, b: Component) -> bool {
    match (a, b) {
        (Component::Prefix(a), Component::Prefix(b)) => same_prefix(a.kind(), b.kind()),
        (Component::Prefix(_), _) | (_, Component::Prefix(_)) => false,
        (a, b) => same_name_ignoring_case(a.as_os_str(), b.as_os_str()),
    }
}

/// Whether two path prefixes name the same drive or share, ignoring the
/// verbatim `\\?\` form.
fn same_prefix(a: Prefix, b: Prefix) -> bool {
    match (a, b) {
        (Prefix::Disk(a) | Prefix::VerbatimDisk(a), Prefix::Disk(b) | Prefix::VerbatimDisk(b)) => {
            a.eq_ignore_ascii_case(&b)
        }
        (
            Prefix::UNC(server_a, share_a) | Prefix::VerbatimUNC(server_a, share_a),
            Prefix::UNC(server_b, share_b) | Prefix::VerbatimUNC(server_b, share_b),
        ) => {
            same_name_ignoring_case(server_a, server_b) && same_name_ignoring_case(share_a, share_b)
        }
        (Prefix::Verbatim(a), Prefix::Verbatim(b)) | (Prefix::DeviceNS(a), Prefix::DeviceNS(b)) => {
            same_name_ignoring_case(a, b)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_inside_compares_whole_components() {
        assert!(is_inside(Path::new(r"C:\A\B\x.txt"), Path::new(r"C:\a\b")));
        assert!(is_inside(Path::new(r"C:\A"), Path::new(r"C:\a")));
        assert!(is_inside(Path::new("C:/A/B"), Path::new(r"C:\a")));
        assert!(!is_inside(
            Path::new(r"C:\DeskSpace.App"),
            Path::new(r"C:\DeskSpace")
        ));
        assert!(!is_inside(
            Path::new(r"C:\DeskSpace"),
            Path::new(r"C:\DeskSpace.App")
        ));
        assert!(!is_inside(Path::new(r"C:\Other"), Path::new(r"C:\A")));
        assert!(!is_inside(Path::new(r"D:\A"), Path::new(r"C:\A")));
    }

    #[test]
    fn a_final_path_is_inside_its_plain_spelling() {
        let root = Path::new(r"C:\Users\me\AppData\Roaming\DeskSpace\storage");
        assert!(is_inside(
            Path::new(r"\\?\C:\Users\me\AppData\Roaming\DeskSpace\storage\sub"),
            root
        ));
        assert!(is_inside(
            Path::new(r"c:\users\ME\appdata\roaming\deskspace\STORAGE"),
            Path::new(r"\\?\C:\Users\me\AppData\Roaming\DeskSpace\storage")
        ));
        assert!(is_inside(
            Path::new(r"\\?\UNC\Server\Share\storage\sub"),
            Path::new(r"\\server\share\storage")
        ));
        assert!(!is_inside(
            Path::new(r"\\?\UNC\server\other\storage"),
            Path::new(r"\\server\share\storage")
        ));
        assert!(!is_inside(
            Path::new(r"\\?\D:\Users\me\AppData\Roaming\DeskSpace\storage"),
            root
        ));
    }

    #[test]
    fn a_final_path_reads_plain() {
        assert_eq!(
            plain_path(Path::new(r"\\?\C:\Users\me\Work")),
            Path::new(r"C:\Users\me\Work")
        );
        assert_eq!(
            plain_path(Path::new(r"\\?\UNC\server\share\Work")),
            Path::new(r"\\server\share\Work")
        );
        assert_eq!(plain_path(Path::new(r"\\?\C:\")), Path::new(r"C:\"));
        assert_eq!(
            plain_path(Path::new(r"D:\Already\Plain")),
            Path::new(r"D:\Already\Plain")
        );
    }

    #[test]
    fn volumes_compare_with_or_without_the_verbatim_prefix() {
        assert!(same_volume(Path::new(r"\\?\c:\New"), Path::new(r"C:\Old")));
        assert!(same_volume(
            Path::new(r"\\?\UNC\Server\Share\New"),
            Path::new(r"\\server\share\Old")
        ));
        assert!(!same_volume(Path::new(r"D:\New"), Path::new(r"C:\Old")));
        assert!(!same_volume(Path::new(r"New"), Path::new(r"C:\Old")));
    }

    #[test]
    fn the_recycle_bin_is_found_in_any_case() {
        assert!(in_recycle_bin(Path::new(
            r"\\?\C:\$RECYCLE.BIN\S-1-5-21-1\$R1A2B3C"
        )));
        assert!(in_recycle_bin(Path::new(r"D:\$Recycle.Bin\S-1\$RX\Sub")));
        assert!(!in_recycle_bin(Path::new(r"C:\Users\me\Recycle.Bin")));
    }

    #[test]
    fn accented_letters_match_in_any_case() {
        assert!(same_name_ignoring_case(
            OsStr::new("É.txt"),
            OsStr::new("é.TXT")
        ));
        assert_eq!(
            upper_name(OsStr::new("É.txt")),
            upper_name(OsStr::new("é.txt"))
        );
    }

    #[test]
    fn an_eszett_does_not_match_two_s() {
        assert!(!same_name_ignoring_case(
            OsStr::new("ß.txt"),
            OsStr::new("SS.txt")
        ));
        assert_ne!(
            upper_name(OsStr::new("ß.txt")),
            upper_name(OsStr::new("SS.txt"))
        );
    }

    #[test]
    fn names_that_differ_match_only_in_case() {
        assert!(same_name_ignoring_case(
            OsStr::new("Report.txt"),
            OsStr::new("report.TXT")
        ));
        assert!(!same_name_ignoring_case(
            OsStr::new("Report.txt"),
            OsStr::new("Report.csv")
        ));
    }
}
