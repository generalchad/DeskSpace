//! Creating `.lnk` shortcuts from shell items, for drags that carry a shell ID
//! list instead of files (a Start-menu tile or a taskbar app, ROADMAP F-50).
//!
//! Explorer's behavior when such an item is dropped on a folder is to write a
//! shortcut beside the folder's contents. The shortcut points at the item's
//! shell ID list rather than a path, so a packaged app with no filesystem path
//! gets a working `.lnk` too.

use std::path::Path;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{Interface, PCWSTR};

use crate::error::PlatformError;
use crate::input::ShellItem;
use crate::win32::system::wide;

/// Writes a `.lnk` at `path` that opens `item` when used, the way Explorer
/// creates a shortcut for a Start-menu or taskbar item. The link's target is
/// the item's shell ID list, so a packaged app with no file path works the
/// same as a plain one. `path`'s parent folder must already exist, and OLE
/// must be initialized on this thread (the runtime does both).
pub fn create_shortcut(item: &ShellItem, path: &Path) -> Result<(), PlatformError> {
    let target = wide(&path.to_string_lossy().replace('/', "\\"));
    // SAFETY: OLE is initialized on this thread; the item's ID list bytes and
    // the NUL-terminated target string outlive every call.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetIDList(item.id_list.as_ptr().cast::<ITEMIDLIST>())?;
        let persist: IPersistFile = link.cast()?;
        persist.Save(PCWSTR(target.as_ptr()), true)?;
    }
    Ok(())
}
