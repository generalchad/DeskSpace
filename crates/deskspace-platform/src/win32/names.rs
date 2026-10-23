//! Comparing file names the way Windows does: ordinal, one character at a
//! time, so a name written in another case still matches.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};

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

#[cfg(test)]
mod tests {
    use super::*;

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
