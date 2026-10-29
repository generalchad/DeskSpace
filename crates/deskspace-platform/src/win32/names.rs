//! Comparing file names the way Windows does: ordinal, one character at a
//! time, so a name written in another case still matches.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, Prefix};

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
