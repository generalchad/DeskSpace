//! The type scale in force for this run (ROADMAP F-68's typography): the
//! `Typography` every font is built from, and the installed families the Debug
//! window offers. Session state only; nothing here is saved.

use std::cell::RefCell;

use deskspace_core::typography::Typography;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
};
use windows::core::BOOL;
use windows::core::PCWSTR;

thread_local! {
    // Process state: the type scale in force. Windows' window procedures get no
    // context pointer, so it cannot be passed. `set` replaces it; a fresh run
    // starts from `Typography::default`.
    static CURRENT: RefCell<Option<Typography>> = const { RefCell::new(None) };
}

/// The type scale every font is built from now.
pub(crate) fn current() -> Typography {
    CURRENT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_default()
}

/// Replaces the type scale. Fonts already made keep their look; new windows
/// and the next paint read this.
pub(crate) fn set(typography: Typography) {
    CURRENT.with(|cell| *cell.borrow_mut() = Some(typography.clamped()));
}

/// Whether `family` is installed, for choosing the default UI family.
pub(crate) fn is_installed(
    fonts: &windows::Win32::Graphics::DirectWrite::IDWriteFontCollection,
    family: PCWSTR,
) -> bool {
    let mut index = 0;
    let mut exists = BOOL::default();
    // SAFETY: out-pointers are valid locals.
    unsafe { fonts.FindFamilyName(family, &mut index, &mut exists) }.is_ok() && exists.as_bool()
}

/// Every installed family, sorted and deduplicated, for the Debug box's family
/// choice. Empty when DirectWrite cannot list them.
pub(crate) fn installed_families() -> Vec<String> {
    let mut names = Vec::new();
    // SAFETY: the factory and collection are live for the call; each name
    // buffer is sized from the length the call reports.
    unsafe {
        let created: windows::core::Result<IDWriteFactory> =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED);
        let Ok(dwrite) = created else {
            return names;
        };
        let mut collection = None;
        if dwrite
            .GetSystemFontCollection(&mut collection, false)
            .is_err()
        {
            return names;
        }
        let Some(collection) = collection else {
            return names;
        };
        for index in 0..collection.GetFontFamilyCount() {
            let Ok(family) = collection.GetFontFamily(index) else {
                continue;
            };
            let Ok(strings) = family.GetFamilyNames() else {
                continue;
            };
            // The first locale the collection lists is the family's own.
            let Ok(length) = strings.GetStringLength(0) else {
                continue;
            };
            if length == 0 {
                continue;
            }
            // `GetString` writes a terminator, so the buffer is one longer
            // than the reported length.
            let mut buffer = vec![0u16; length as usize + 1];
            if strings.GetString(0, &mut buffer).is_err() {
                continue;
            }
            let name = String::from_utf16_lossy(&buffer[..length as usize]);
            if !name.trim().is_empty() {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_font_collection_lists_families() {
        let families = installed_families();
        // A buffer one short of the reported length made every GetString fail
        // and left the Debug box with no family but Windows default.
        assert!(families.len() > 10, "{} families", families.len());
        let lowered: Vec<String> = families.iter().map(|name| name.to_lowercase()).collect();
        assert!(
            lowered.windows(2).all(|pair| pair[0] <= pair[1]),
            "sorted ignoring case"
        );
        assert!(families.iter().all(|name| !name.contains('\0')));
    }
}
