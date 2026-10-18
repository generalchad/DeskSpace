//! Creating shortcuts: `.lnk` files from shell items, for drags that carry a
//! shell ID list instead of files (a Start-menu tile or a taskbar app, ROADMAP
//! F-50) and for apps picked from the Applications folder (F-53), and `.lnk`
//! or `.url` files from a target typed in the "Custom shortcut" dialog.
//!
//! Explorer's behavior when a shell item is dropped on a folder is to write a
//! shortcut beside the folder's contents. The shortcut points at the item's
//! shell ID list rather than a path, so a packaged app with no filesystem path
//! gets a working `.lnk` too.

use std::path::{Path, PathBuf};

use windows::Win32::Storage::FileSystem::SearchPathW;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILGetSize, IShellItem, IShellLinkW, SHGetIDListFromObject, SIGDN, SIGDN_FILESYSPATH,
    SIGDN_NORMALDISPLAY, ShellLink,
};
use windows::core::{Interface, PCWSTR};

use crate::error::PlatformError;
use crate::input::ShellItem;
use crate::ui::{CustomShortcut, ShortcutTarget};
use crate::win32::system::wide;

/// Writes a `.lnk` at `path` that opens `item` when used, the way Explorer
/// creates a shortcut for a Start-menu or taskbar item. The link's target is
/// the item's shell ID list, so a packaged app with no file path works the
/// same as a plain one. `path`'s parent folder must already exist, and OLE
/// must be initialized on this thread (the runtime does both).
pub fn create_shortcut(item: &ShellItem, path: &Path) -> Result<(), PlatformError> {
    // SAFETY: OLE is initialized on this thread; the item's ID list bytes
    // outlive the call.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetIDList(item.id_list.as_ptr().cast::<ITEMIDLIST>())?;
        save_link(&link, path)
    }
}

/// Writes the shortcut `shortcut` describes at `path`: a `.lnk` to a path
/// (with its arguments, starting in the target's folder), or a `.url`
/// Internet shortcut to a URL. The caller picks `path`'s extension from
/// `ShortcutTarget::extension`.
pub fn create_custom_shortcut(shortcut: &CustomShortcut, path: &Path) -> Result<(), PlatformError> {
    match &shortcut.target {
        ShortcutTarget::Path(target) => create_path_shortcut(target, &shortcut.arguments, path),
        ShortcutTarget::Url(url) => {
            std::fs::write(path, internet_shortcut(url)).map_err(PlatformError::from)
        }
    }
}

fn create_path_shortcut(target: &Path, arguments: &str, path: &Path) -> Result<(), PlatformError> {
    let target_text = wide(&target.to_string_lossy());
    let arguments = wide(arguments.trim());
    let folder = target
        .parent()
        .filter(|_| target.is_file())
        .map(|folder| wide(&folder.to_string_lossy()));
    // SAFETY: OLE is initialized on this thread; every string is
    // NUL-terminated and outlives the calls.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(target_text.as_ptr()))?;
        link.SetArguments(PCWSTR(arguments.as_ptr()))?;
        if let Some(folder) = &folder {
            link.SetWorkingDirectory(PCWSTR(folder.as_ptr()))?;
        }
        save_link(&link, path)
    }
}

/// Saves a configured shell link to `path`.
unsafe fn save_link(link: &IShellLinkW, path: &Path) -> Result<(), PlatformError> {
    let file = wide(&path.to_string_lossy().replace('/', "\\"));
    // SAFETY: `link` is live and `file` outlives the call.
    unsafe {
        let persist: IPersistFile = link.cast()?;
        persist.Save(PCWSTR(file.as_ptr()), true)?;
    }
    Ok(())
}

/// The text of a `.url` Internet shortcut: the `[InternetShortcut]` section
/// with its `URL` key, which is all Explorer needs to open it.
fn internet_shortcut(url: &str) -> String {
    format!("[InternetShortcut]\r\nURL={url}\r\n")
}

/// Turns a typed path into an existing file or folder, the way the Run
/// dialog reads it: `%VARIABLES%` are expanded, and a bare program name
/// (`notepad`) is looked up on the search path with `.exe` assumed. `None`
/// when nothing exists there.
pub fn resolve_path(typed: &Path) -> Option<PathBuf> {
    let expanded = expand_environment(&typed.to_string_lossy());
    let path = PathBuf::from(&expanded);
    if path.exists() {
        return Some(path);
    }
    if path.components().count() == 1 {
        return search_path(&expanded);
    }
    None
}

/// Expands `%NAME%` environment variables.
/// https://learn.microsoft.com/en-us/windows/win32/api/processenv/nf-processenv-expandenvironmentstringsw
fn expand_environment(text: &str) -> String {
    let source = wide(text);
    let mut buffer = vec![0u16; 1024];
    // SAFETY: `source` is NUL-terminated; the buffer length bounds the write.
    let written = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut buffer)) };
    if written == 0 || written as usize > buffer.len() {
        return text.to_string();
    }
    String::from_utf16_lossy(&buffer[..written as usize - 1])
}

/// Finds a program by name on the search path, assuming `.exe`.
/// https://learn.microsoft.com/en-us/windows/win32/api/processenv/nf-processenv-searchpathw
fn search_path(name: &str) -> Option<PathBuf> {
    let name = wide(name);
    let extension = wide(".exe");
    let mut buffer = vec![0u16; 1024];
    // SAFETY: both strings are NUL-terminated; the buffer length bounds the
    // write.
    let length = unsafe {
        SearchPathW(
            PCWSTR::null(),
            PCWSTR(name.as_ptr()),
            PCWSTR(extension.as_ptr()),
            Some(&mut buffer),
            None,
        )
    } as usize;
    (length > 0 && length < buffer.len())
        .then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}

/// One `ShellItem` from a live shell item, or `None` when its ID list cannot be
/// read.
pub(crate) fn shell_item(item: &IShellItem) -> Option<ShellItem> {
    // SAFETY: the ID list is copied before it is freed with the shell
    // allocator the API documents.
    let id_list = unsafe {
        let pidl = SHGetIDListFromObject(item).ok()?;
        let size = ILGetSize(Some(pidl)) as usize;
        let id_list = std::slice::from_raw_parts(pidl.cast::<u8>(), size).to_vec();
        CoTaskMemFree(Some(pidl.cast::<core::ffi::c_void>()));
        id_list
    };
    Some(ShellItem {
        name: display_name(item, SIGDN_NORMALDISPLAY).unwrap_or_default(),
        path: display_name(item, SIGDN_FILESYSPATH).map(PathBuf::from),
        id_list,
    })
}

/// A shell item's name in the `form` asked for, or `None` when the shell
/// cannot give one (`SIGDN_FILESYSPATH` fails for a virtual item).
pub(crate) fn display_name(item: &IShellItem, form: SIGDN) -> Option<String> {
    // SAFETY: the returned string is copied before it is freed with the shell
    // allocator the API documents.
    unsafe {
        let name = item.GetDisplayName(form).ok()?;
        let text = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_internet_shortcut_names_its_url() {
        assert_eq!(
            internet_shortcut("https://example.com"),
            "[InternetShortcut]\r\nURL=https://example.com\r\n"
        );
    }

    #[test]
    fn typed_paths_expand_variables_and_search_for_programs() {
        let windows = resolve_path(Path::new("%SystemRoot%")).expect("the Windows folder");
        assert!(windows.is_dir());
        let notepad = resolve_path(Path::new("notepad")).expect("notepad on the path");
        assert!(
            notepad
                .to_string_lossy()
                .to_lowercase()
                .ends_with("notepad.exe")
        );
        assert_eq!(resolve_path(Path::new(r"C:\no\such\thing.exe")), None);
    }
}
