use std::collections::HashMap;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use deskspace_core::item::ItemKind;
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_SYSTEM,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IShellItem, KF_FLAG_DEFAULT, SHCNE_CREATE, SHCNE_DELETE, SHCNF_FLUSHNOWAIT,
    SHCNF_PATHW, SHChangeNotify, SHCreateItemFromParsingName, SHFILEINFOW, SHGFI_ADDOVERLAYS,
    SHGFI_ICON, SHGFI_LARGEICON, SHGFI_LINKOVERLAY, SHGetFileInfoW, SHGetKnownFolderPath,
    SIGDN_NORMALDISPLAY,
};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::d2d::decode_icon;
use crate::win32::display_settings::{ShellDisplay, desktop_display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub path: PathBuf,
    pub label: String,
    pub kind: ItemKind,
}

pub fn enumerate_desktop_items() -> Result<Vec<DesktopEntry>, PlatformError> {
    let directory = desktop_directory()?;
    let display = desktop_display();
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("desktop.ini") || !listed_by_explorer(&path, &display) {
            continue;
        }
        entries.push(DesktopEntry {
            label: display_name(&path).unwrap_or_else(|| fallback_label(&path)),
            kind: ItemKind::classify(&path),
            path,
        });
    }

    entries.sort_by_key(|entry| entry.label.to_lowercase());
    Ok(entries)
}

/// Whether Explorer would list this file, given its hidden and protected
/// operating-system file settings.
fn listed_by_explorer(path: &Path, display: &ShellDisplay) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let attributes = metadata.file_attributes();
    if attributes & FILE_ATTRIBUTE_HIDDEN.0 != 0 && !display.show_hidden {
        return false;
    }
    if attributes & FILE_ATTRIBUTE_SYSTEM.0 != 0 && !display.show_superhidden {
        return false;
    }
    true
}

/// The label Explorer shows for a path, honoring its "hide extensions" setting
/// (and stripping `.lnk`, using friendly folder names, and so on).
fn display_name(path: &Path) -> Option<String> {
    let wide = crate::win32::system::wide(&path.to_string_lossy().replace('/', "\\"));
    // SAFETY: `wide` is NUL-terminated and outlives the parse; the returned
    // string is copied then freed with the shell's allocator.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let text = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
        text.filter(|text| !text.is_empty())
    }
}

fn fallback_label(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
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

/// Tells the shell that DeskSpace moved a file from `from` to `to`, so open
/// views (above all the desktop) update at once, as they do after
/// Explorer's own moves. Without it the desktop can keep showing an icon for
/// a file already moved into a space; dragging that stale icon then drops a
/// path that no longer exists.
/// https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shchangenotify
pub fn notify_moved(from: &Path, to: &Path) {
    let from = crate::win32::system::wide(&from.to_string_lossy());
    let to = crate::win32::system::wide(&to.to_string_lossy());
    // A delete in the old folder plus a create in the new one is understood
    // by every folder view, whether or not it watches both folders.
    // FLUSHNOWAIT: never block. During a drop Explorer waits inside
    // DoDragDrop for this process, so a synchronous flush could deadlock.
    // SAFETY: both strings are NUL-terminated and outlive the calls; the shell
    // copies the paths before returning.
    unsafe {
        SHChangeNotify(
            SHCNE_DELETE,
            SHCNF_PATHW | SHCNF_FLUSHNOWAIT,
            Some(from.as_ptr().cast()),
            None,
        );
        SHChangeNotify(
            SHCNE_CREATE,
            SHCNF_PATHW | SHCNF_FLUSHNOWAIT,
            Some(to.as_ptr().cast()),
            None,
        );
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
    // ADDOVERLAYS composites the shell's overlays (shortcut arrow, sync
    // badges) into the returned icon, and LINKOVERLAY forces the shortcut
    // arrow, so space icons match Explorer's.
    let flags = SHGFI_ICON | SHGFI_LARGEICON | SHGFI_ADDOVERLAYS | SHGFI_LINKOVERLAY;
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
