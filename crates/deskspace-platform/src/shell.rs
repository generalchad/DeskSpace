use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deskspace_core::item::ItemKind;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, KF_FLAG_DEFAULT, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW,
    SHGetKnownFolderPath,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
use windows::core::PCWSTR;

use crate::error::PlatformError;

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

fn desktop_directory() -> Result<PathBuf, PlatformError> {
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)?;
        let result = raw.to_string().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        result.map_err(|_| PlatformError::InvalidFormat("desktop path"))
    }
}

pub struct IconCache {
    size: i32,
    handles: HashMap<PathBuf, isize>,
}

impl IconCache {
    pub fn new(size: i32) -> Self {
        Self {
            size,
            handles: HashMap::new(),
        }
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    pub fn icon_for(&mut self, path: &Path) -> Option<isize> {
        if let Some(handle) = self.handles.get(path) {
            return Some(*handle);
        }
        let handle = load_icon(path)?;
        self.handles.insert(path.to_path_buf(), handle);
        Some(handle)
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

impl Drop for IconCache {
    fn drop(&mut self) {
        for handle in self.handles.values() {
            unsafe {
                let _ = DestroyIcon(HICON(*handle as *mut core::ffi::c_void));
            }
        }
    }
}

fn load_icon(path: &Path) -> Option<isize> {
    let mut wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().collect();
    wide.push(0);

    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON;
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
    }
    Some(info.hIcon.0 as isize)
}
