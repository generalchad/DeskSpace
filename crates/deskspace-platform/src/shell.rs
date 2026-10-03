use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deskspace_core::item::ItemKind;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, KF_FLAG_DEFAULT, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW,
    SHGetKnownFolderPath,
};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::d2d::decode_icon;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub path: PathBuf,
    pub label: String,
    pub kind: ItemKind,
}

pub fn enumerate_desktop_items() -> Result<Vec<DesktopEntry>, PlatformError> {
    let directory = desktop_directory()?;
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        entries.push(DesktopEntry {
            label: path
                .file_stem()
                .or_else(|| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or(name)
                .to_string(),
            kind: ItemKind::classify(&path),
            path,
        });
    }

    entries.sort_by_key(|entry| entry.label.to_lowercase());
    Ok(entries)
}

/// The current user's Desktop folder.
pub fn desktop_directory() -> Result<PathBuf, PlatformError> {
    // SAFETY: the returned string is copied before it is freed with the
    // allocator the shell documents for it (CoTaskMemFree).
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)?;
        let result = raw.to_string().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        result.map_err(|_| PlatformError::InvalidFormat("desktop path"))
    }
}

/// Icons decoded per path, bounded by a least-recently-used limit. Evicting
/// is always safe: scenes hold their own reference-counted `Icon`s.
pub struct IconCache {
    size: i32,
    capacity: usize,
    clock: u64,
    entries: HashMap<PathBuf, CachedIcon>,
}

struct CachedIcon {
    icon: Icon,
    last_used: u64,
}

impl IconCache {
    /// Enough for every visible item on a crowded desktop; about 4 MB of
    /// pixels at 32 px.
    pub const DEFAULT_CAPACITY: usize = 1024;

    pub fn new(size: i32) -> Self {
        Self::with_capacity(size, Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(size: i32, capacity: usize) -> Self {
        Self {
            size,
            capacity: capacity.max(1),
            clock: 0,
            entries: HashMap::new(),
        }
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    pub fn icon_for(&mut self, path: &Path) -> Option<Icon> {
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(path) {
            entry.last_used = self.clock;
            return Some(entry.icon.clone());
        }
        let icon = load_icon(path)?;
        self.insert(path.to_path_buf(), icon.clone());
        Some(icon)
    }

    /// Forgets the icon for `path`, so the next lookup reloads it (for
    /// example after the file changed).
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert(&mut self, path: PathBuf, icon: Icon) {
        if self.entries.len() >= self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(path, _)| path.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            path,
            CachedIcon {
                icon,
                last_used: self.clock,
            },
        );
    }
}

fn load_icon(path: &Path) -> Option<Icon> {
    let mut wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().collect();
    wide.push(0);

    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON;
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure of
    // the size passed. The returned HICON is ours and destroyed below, after
    // its pixels have been copied out.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = decode_icon(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon() -> Icon {
        Icon::new(1, 1, vec![0; 4].into_boxed_slice())
    }

    #[test]
    fn evicts_the_least_recently_used_icon() {
        let mut cache = IconCache::with_capacity(32, 2);
        cache.insert("a".into(), icon());
        cache.clock += 1;
        cache.insert("b".into(), icon());
        cache.clock += 1;
        // Touch "a" so "b" becomes the oldest.
        cache.entries.get_mut(Path::new("a")).unwrap().last_used = cache.clock;
        cache.insert("c".into(), icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(Path::new("a")));
        assert!(!cache.entries.contains_key(Path::new("b")));
    }

    #[test]
    fn invalidate_forgets_a_path() {
        let mut cache = IconCache::new(32);
        cache.insert("a".into(), icon());
        cache.invalidate(Path::new("a"));
        assert!(cache.is_empty());
    }

    #[test]
    fn evicted_icons_stay_usable_by_their_holders() {
        let mut cache = IconCache::with_capacity(32, 1);
        let held = icon();
        cache.insert("a".into(), held.clone());
        cache.insert("b".into(), icon());
        assert_eq!(held.pixels().len(), 4);
    }
}
