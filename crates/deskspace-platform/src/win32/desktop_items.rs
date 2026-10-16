//! Reading the desktop's items and their positions through the shell,
//! with a folder-walk fallback.

use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use deskspace_core::item::{ItemFacts, ItemKind};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_COMPRESSED, FILE_ATTRIBUTE_ENCRYPTED, FILE_ATTRIBUTE_HIDDEN,
    FILE_ATTRIBUTE_SYSTEM,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IFolderView, IShellFolder, IShellItem, KF_FLAG_DEFAULT, SHCNE_CREATE,
    SHCNE_DELETE, SHCNF_FLUSHNOWAIT, SHCNF_PATHW, SHChangeNotify, SHCreateItemFromParsingName,
    SHCreateItemWithParent, SHGetKnownFolderPath, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
    SVGIO_ALLVIEW,
};
use windows::core::{Interface, PCWSTR};

use crate::error::PlatformError;
use crate::win32::display_settings::{ShellDisplay, desktop_display};
use crate::win32::system::wide;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub path: PathBuf,
    pub label: String,
    pub kind: ItemKind,
    /// The item's top-left in the desktop view's coordinates, when the shell
    /// view supplied it. The folder-walk fallback has no positions, so it is
    /// `None` there and the caller decides where to place the item.
    pub position: Option<Point>,
}

/// The desktop's items. Prefers Explorer's own view, which reflects exactly
/// what the desktop shows (including the public desktop and the hidden-file
/// setting) and carries each item's position. Falls back to walking the user's
/// Desktop folder, without positions, when the view cannot be reached (for
/// example before OLE is initialized at startup).
pub fn enumerate_desktop_items() -> Result<Vec<DesktopEntry>, PlatformError> {
    if let Some(entries) = enumerate_via_view() {
        return Ok(entries);
    }
    let directory = desktop_directory()?;
    let display = desktop_display();
    enumerate_via_folder(&directory, &display)
}

/// The items Explorer's desktop view is showing, with positions, or `None` when
/// the view is not available.
fn enumerate_via_view() -> Option<Vec<DesktopEntry>> {
    let view = crate::win32::shell_view::desktop_shell_view()?;
    let folder: IFolderView = view.cast().ok()?;
    // SAFETY: `folder` is a live desktop view; the call fills an out-parameter.
    let parent: IShellFolder = unsafe { folder.GetFolder() }.ok()?;
    // SAFETY: as above.
    let count = unsafe { folder.ItemCount(SVGIO_ALLVIEW) }.ok()?;
    let mut entries = Vec::with_capacity(count.max(0) as usize);

    for index in 0..count {
        // SAFETY: `folder` is a live view; each call fills an out-parameter.
        // The child PIDL it returns is freed below with the shell allocator.
        let pidl = match unsafe { folder.Item(index) } {
            Ok(pidl) if !pidl.is_null() => pidl,
            _ => continue,
        };
        // SAFETY: `pidl` is a child PIDL of the view's folder; the call only
        // fills a POINT, in the view's coordinates.
        let position = unsafe { folder.GetItemPosition(pidl) }
            .ok()
            .map(|point| Point::new(point.x, point.y));
        // SAFETY: `parent` is the desktop folder and `pidl` is one of its
        // children; the returned item holds its own reference.
        let item: Result<IShellItem, _> = unsafe { SHCreateItemWithParent(None, &parent, pidl) };
        // SAFETY: `pidl` came from `folder.Item`; free it with the allocator the
        // shell documents for it.
        unsafe { CoTaskMemFree(Some(pidl as *const core::ffi::c_void)) };

        let Ok(item) = item else {
            continue;
        };
        // A virtual desktop item (This PC, Recycle Bin) has no file behind it;
        // D1 takeover covers real filesystem entries only.
        let Some(path) = filesystem_path(&item) else {
            continue;
        };
        entries.push(DesktopEntry {
            label: display_name(&path).unwrap_or_else(|| fallback_label(&path)),
            kind: ItemKind::classify(&path, path.is_dir()),
            path,
            position,
        });
    }
    Some(entries)
}

/// The user's Desktop folder walked directly, name-sorted, without positions.
fn enumerate_via_folder(
    directory: &Path,
    display: &ShellDisplay,
) -> Result<Vec<DesktopEntry>, PlatformError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !listed_by_explorer(&path, display) {
            continue;
        }
        entries.push(DesktopEntry {
            label: display_name(&path).unwrap_or_else(|| fallback_label(&path)),
            kind: ItemKind::classify(&path, path.is_dir()),
            path,
            position: None,
        });
    }
    entries.sort_by_key(|entry| entry.label.to_lowercase());
    Ok(entries)
}

/// What the file system says about `path`, read once when an item is listed;
/// nothing watches it afterwards, so the values can go stale. An unreadable
/// path yields `ItemFacts::default()`. The compressed and encrypted attributes
/// drive Explorer's label colors:
/// <https://learn.microsoft.com/windows/win32/fileio/file-attribute-constants>
pub fn item_facts(path: &Path) -> ItemFacts {
    let Ok(metadata) = std::fs::metadata(path) else {
        return ItemFacts::default();
    };
    let attributes = metadata.file_attributes();
    ItemFacts {
        is_folder: metadata.is_dir(),
        size: metadata.is_file().then_some(metadata.len()),
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_secs()),
        compressed: attributes & FILE_ATTRIBUTE_COMPRESSED.0 != 0,
        encrypted: attributes & FILE_ATTRIBUTE_ENCRYPTED.0 != 0,
    }
}

/// Whether Explorer would list this file, given its hidden and protected
/// operating-system file settings.
fn listed_by_explorer(path: &Path, display: &ShellDisplay) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    keep_attributes(name, metadata.file_attributes(), display)
}

/// Whether a name with `attributes` is listed, given Explorer's hidden and
/// protected operating-system file settings. The folder's `desktop.ini` is
/// never shown.
fn keep_attributes(name: &str, attributes: u32, display: &ShellDisplay) -> bool {
    if name.eq_ignore_ascii_case("desktop.ini") {
        return false;
    }
    if attributes & FILE_ATTRIBUTE_HIDDEN.0 != 0 && !display.show_hidden {
        return false;
    }
    if attributes & FILE_ATTRIBUTE_SYSTEM.0 != 0 && !display.show_superhidden {
        return false;
    }
    true
}

/// The filesystem path of a shell item, or `None` for a virtual item with no
/// file behind it. `SIGDN_FILESYSPATH` fails for those.
fn filesystem_path(item: &IShellItem) -> Option<PathBuf> {
    // SAFETY: the returned string is copied before it is freed with the shell
    // allocator the API documents.
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
        path
    }
}

/// The label Explorer shows for a path, honoring its "hide extensions" setting
/// (and stripping `.lnk`, using friendly folder names, and so on).
fn display_name(path: &Path) -> Option<String> {
    let wide = wide(&path.to_string_lossy().replace('/', "\\"));
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
    let from = wide(&from.to_string_lossy());
    let to = wide(&to.to_string_lossy());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_attributes_filters_hidden_protected_and_desktop_ini() {
        let hidden = FILE_ATTRIBUTE_HIDDEN.0;
        let system = FILE_ATTRIBUTE_SYSTEM.0;
        let shown = ShellDisplay {
            show_hidden: true,
            show_superhidden: true,
            ..ShellDisplay::default()
        };
        let hidden_off = ShellDisplay::default();

        assert!(keep_attributes("file.txt", 0, &hidden_off));
        assert!(!keep_attributes("desktop.ini", 0, &shown));
        assert!(!keep_attributes("Desktop.INI", 0, &shown));
        assert!(!keep_attributes("h.txt", hidden, &hidden_off));
        assert!(keep_attributes("h.txt", hidden, &shown));
        assert!(!keep_attributes("s.txt", system, &hidden_off));
        assert!(keep_attributes("s.txt", system, &shown));
    }

    #[test]
    fn item_facts_reads_files_and_folders_and_defaults_when_missing() {
        let folder = std::env::temp_dir().join(format!("deskspace-facts-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("five.txt");
        std::fs::write(&file, "hello").unwrap();

        let file_facts = item_facts(&file);
        let folder_facts = item_facts(&folder);
        let missing_facts = item_facts(&folder.join("missing.txt"));
        let _ = std::fs::remove_dir_all(&folder);

        assert!(!file_facts.is_folder);
        assert_eq!(file_facts.size, Some(5));
        assert!(file_facts.modified.is_some());
        assert!(folder_facts.is_folder);
        assert_eq!(folder_facts.size, None);
        assert_eq!(missing_facts, ItemFacts::default());
    }
}
