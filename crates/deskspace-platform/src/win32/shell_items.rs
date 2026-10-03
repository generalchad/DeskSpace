//! Explorer behavior for items in spaces: open, the shell context menu, and
//! dragging items out.
//!
//! All three work on `IShellItem`s created from file paths, so the shell
//! supplies the same verbs, menus, and drag data that Explorer would.

use std::cell::RefCell;
use std::path::Path;

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, IDropSource,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::Shell::{
    BHID_DataObject, BHID_SFUIObject, CMF_NORMAL, CMINVOKECOMMANDINFO, IContextMenu, IContextMenu2,
    IContextMenu3, IShellItem, SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC,
    SHCreateItemFromParsingName, SHDoDragDrop, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, AppendMenuW, CreatePopupMenu, DestroyMenu, GA_ROOT,
    GetAncestor, GetClassNameW, GetCursorPos, MF_SEPARATOR, PostMessageW, SW_SHOWNORMAL,
    SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_DRAWITEM,
    WM_INITMENUPOPUP, WM_MEASUREITEM, WM_MENUCHAR, WM_NULL, WindowFromPoint,
};
use windows::core::{Interface, PCSTR, PCWSTR};

use crate::error::PlatformError;
use crate::ui::MenuEntry;
use crate::win32::system::wide;

/// Shell menu commands get ids from here up; app entries must stay below.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-icontextmenu-querycontextmenu
const SHELL_FIRST_ID: u32 = 0x8000;
const SHELL_LAST_ID: u32 = 0xEFFF;

thread_local! {
    /// The shell menu being tracked. Owner-drawn and lazily filled submenus
    /// ("Open with", "Send to") only work if the owner window forwards menu
    /// messages to it; the controller does that through
    /// `forward_menu_message`.
    static ACTIVE_MENU: RefCell<Option<IContextMenu>> = const { RefCell::new(None) };
}

/// What the user picked in an item menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemMenuChoice {
    /// One of the app's own entries.
    App(u32),
    /// A shell command (Open, Cut, Delete, Properties, ...) ran. The item may
    /// have been moved or deleted.
    Shell,
}

/// How an item drag ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragResult {
    /// The effect the drop target reported. Explorer reports no effect for
    /// "optimized" moves on the same volume, so check whether the file still
    /// exists rather than relying on `Move`.
    pub effect: DragEffect,
    /// The pointer was over the desktop (not a space or another window) when
    /// the drag ended.
    pub over_desktop: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEffect {
    None,
    Copy,
    Move,
    Link,
}

/// `path` as a NUL-terminated wide string with backslash separators. The
/// shell's parsing name APIs reject forward slashes ("The parameter is
/// incorrect"), and hand-edited or older configs can contain them.
fn shell_path(path: &Path) -> Vec<u16> {
    wide(&path.to_string_lossy().replace('/', "\\"))
}

fn shell_item(path: &Path) -> Result<IShellItem, PlatformError> {
    let path = shell_path(path);
    // SAFETY: `path` is NUL-terminated and outlives the call.
    Ok(unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)? })
}

/// Opens `path` with its default verb, like a double-click in Explorer.
/// `owner` parents any error dialog the shell shows.
pub(crate) fn open(owner: Option<HWND>, path: &Path) -> Result<(), PlatformError> {
    let file = shell_path(path);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // NOASYNC: the call returns after the launch is handed off, so it
        // does not depend on this thread pumping messages for DDE.
        // INVOKEIDLIST: run the item's default context-menu verb, as
        // Explorer does, so shortcuts and items without a file association
        // open the same way they do on the desktop.
        // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow
        fMask: SEE_MASK_NOASYNC | SEE_MASK_INVOKEIDLIST,
        hwnd: owner.unwrap_or_default(),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and `file` outlives the call.
    unsafe {
        // Spaces are never activated, so DeskSpace is not the foreground
        // process when an item opens, and Windows would keep the launched
        // app's window behind the active one (or, for an app that is already
        // running, such as a browser, only flash its taskbar button). Having
        // received the double-click, this process may pass its foreground
        // right on.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}

/// Shows the app's `entries`, a separator, and then the shell's context menu
/// for `path` at `at` (screen coordinates). App entry ids must be non-zero
/// and below 0x8000.
pub(crate) fn track_item_menu(
    owner: HWND,
    path: &Path,
    entries: &[MenuEntry],
    at: Point,
) -> Option<ItemMenuChoice> {
    let shell_menu: Option<IContextMenu> = shell_item(path)
        .and_then(|item| {
            // SAFETY: binding a handler on a live shell item.
            Ok(unsafe { item.BindToHandler(None, &BHID_SFUIObject)? })
        })
        .map_err(|error| tracing::debug!(%error, path = %path.display(), "no shell menu"))
        .ok();

    // SAFETY: the menu is created, tracked, and destroyed within this call;
    // labels outlive the AppendMenuW calls that copy them. The context menu
    // is kept alive in ACTIVE_MENU while tracking and released afterwards.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        crate::win32::menu::append_entries(menu, entries);
        if let Some(shell_menu) = &shell_menu {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let index = entries.len() as u32 + 1;
            let result =
                shell_menu.QueryContextMenu(menu, index, SHELL_FIRST_ID, SHELL_LAST_ID, CMF_NORMAL);
            if result.is_err() {
                tracing::debug!(?result, "shell context menu could not be filled");
            }
        }

        ACTIVE_MENU.with(|cell| *cell.borrow_mut() = shell_menu.clone());
        // See menu::track for why the owner must be foreground and why the
        // trailing WM_NULL is posted. TPM_NONOTIFY is deliberately not used:
        // the shell needs WM_INITMENUPOPUP to fill its submenus.
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
            at.x,
            at.y,
            owner,
            None,
        )
        .0 as u32;
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        ACTIVE_MENU.with(|cell| cell.borrow_mut().take());

        let choice = match (chosen, &shell_menu) {
            (0, _) => None,
            (id, Some(shell_menu)) if id >= SHELL_FIRST_ID => {
                // MAKEINTRESOURCE: the verb is the command offset, not a string.
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: owner,
                    lpVerb: PCSTR((id - SHELL_FIRST_ID) as usize as *const u8),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };
                if let Err(error) = shell_menu.InvokeCommand(&info) {
                    tracing::debug!(%error, "shell command failed or was cancelled");
                }
                Some(ItemMenuChoice::Shell)
            }
            (id, _) => Some(ItemMenuChoice::App(id)),
        };
        let _ = DestroyMenu(menu);
        choice
    }
}

/// Lets the shell menu being tracked handle the owner-drawn and submenu
/// messages it needs. Returns the result to return from the window procedure,
/// or `None` when no shell menu is active or it did not handle the message.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-icontextmenu3-handlemenumsg2
pub(crate) fn forward_menu_message(
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if !matches!(
        message,
        WM_INITMENUPOPUP | WM_DRAWITEM | WM_MEASUREITEM | WM_MENUCHAR
    ) {
        return None;
    }
    // Cloned out so no RefCell borrow is held while the shell runs.
    let menu = ACTIVE_MENU.with(|cell| cell.borrow().clone())?;
    // SAFETY: forwarding the owner's own message arguments unchanged.
    unsafe {
        if let Ok(menu3) = menu.cast::<IContextMenu3>() {
            let mut result = LRESULT(0);
            return menu3
                .HandleMenuMsg2(message, wparam, lparam, Some(&mut result))
                .ok()
                .map(|_| result);
        }
        let menu2 = menu.cast::<IContextMenu2>().ok()?;
        menu2.HandleMenuMsg(message, wparam, lparam).ok()?;
    }
    Some(LRESULT(0))
}

/// Starts a shell drag of `path` and blocks until it ends, like dragging the
/// item in Explorer: drop targets choose to move, copy, or link it. Mouse
/// capture is released first because OLE tracks the mouse itself.
pub(crate) fn drag_out(path: &Path) -> Result<DragResult, PlatformError> {
    let item = shell_item(path)?;
    // SAFETY: binding a handler on a live shell item; SHDoDragDrop runs a
    // modal loop on this (OLE-initialised) thread and uses its default drop
    // source and drag image when none is given.
    let effect = unsafe {
        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject)?;
        let _ = ReleaseCapture();
        SHDoDragDrop(
            None,
            &data,
            None::<&IDropSource>,
            DROPEFFECT_MOVE | DROPEFFECT_COPY | DROPEFFECT_LINK,
        )?
    };
    Ok(DragResult {
        effect: drag_effect(effect),
        over_desktop: cursor_over_desktop(),
    })
}

fn drag_effect(effect: DROPEFFECT) -> DragEffect {
    if effect.0 & DROPEFFECT_MOVE.0 != 0 {
        DragEffect::Move
    } else if effect.0 & DROPEFFECT_COPY.0 != 0 {
        DragEffect::Copy
    } else if effect.0 & DROPEFFECT_LINK.0 != 0 {
        DragEffect::Link
    } else {
        DragEffect::None
    }
}

/// Whether the window under the cursor belongs to the desktop: Progman, or
/// the WorkerW that hosts the icons and wallpaper.
fn cursor_over_desktop() -> bool {
    let mut point = POINT::default();
    // SAFETY: plain queries; the class buffer is sized for the call.
    unsafe {
        if GetCursorPos(&mut point).is_err() {
            return false;
        }
        let window = WindowFromPoint(point);
        if window.is_invalid() {
            return false;
        }
        let root = GetAncestor(window, GA_ROOT);
        let mut class = [0u16; 32];
        let length = GetClassNameW(root, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..length]);
        class == "Progman" || class == "WorkerW"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_paths_use_backslashes() {
        assert_eq!(
            shell_path(Path::new("C:/Users/me/a.lnk")),
            wide(r"C:\Users\me\a.lnk")
        );
    }

    #[test]
    fn move_wins_when_several_effects_are_reported() {
        assert_eq!(
            drag_effect(DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_COPY.0)),
            DragEffect::Move
        );
        assert_eq!(drag_effect(DROPEFFECT_LINK), DragEffect::Link);
        assert_eq!(drag_effect(DROPEFFECT(0)), DragEffect::None);
    }
}
