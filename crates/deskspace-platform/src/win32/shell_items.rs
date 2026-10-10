//! Explorer behavior for items in spaces: open, the shell context menu, and
//! dragging items out.
//!
//! All three work on `IShellItem`s created from file paths, so the shell
//! supplies the same verbs, menus, and drag data that Explorer would.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IDataObject};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, IDropSource,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::Shell::{
    BHID_DataObject, BHID_SFUIObject, CLSID_DragDropHelper, CMF_NORMAL, CMINVOKECOMMANDINFO,
    IContextMenu, IContextMenu2, IContextMenu3, IDragSourceHelper, IDragSourceHelper2, IShellItem,
    SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC, SHCreateItemFromParsingName, SHDRAGIMAGE,
    SHDoDragDrop, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, AppendMenuW, CreatePopupMenu, DestroyMenu, MF_SEPARATOR,
    PostMessageW, SW_SHOWNORMAL, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenuEx, WM_DRAWITEM, WM_INITMENUPOPUP, WM_MEASUREITEM, WM_MENUCHAR, WM_NULL,
};
use windows::core::{Interface, PCSTR, PCWSTR};

use crate::error::PlatformError;
use crate::ui::MenuEntry;
use crate::win32::drag_image::{self, DragImage};
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
    /// exists rather than relying on `Move`. The source data object does not
    /// offer asynchronous drops (`IDataObjectAsyncCapability` reports async
    /// mode off), so the transfer has finished when the drag returns.
    pub effect: DragEffect,
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

/// Shows the shell's Properties dialog for `path`, as Alt+Enter does in
/// Explorer. `owner` parents the dialog.
pub(crate) fn properties(owner: Option<HWND>, path: &Path) -> Result<(), PlatformError> {
    let file = shell_path(path);
    let verb = wide("properties");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_INVOKEIDLIST,
        hwnd: owner.unwrap_or_default(),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and `file`/`verb` outlive the call.
    unsafe {
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

/// Starts a shell drag of `paths` and blocks until it ends, like dragging the
/// items in Explorer: drop targets choose to move, copy, or link them. The drag
/// defaults to a move (for example onto the desktop), so the items leave the
/// space instead of leaving copies behind; a target that honors Shift or Ctrl
/// can still choose another effect. `image` follows the pointer; without one
/// the shell substitutes its own. Mouse capture is released first because OLE
/// tracks the mouse itself.
pub(crate) fn drag_out(
    owner: Option<HWND>,
    paths: &[PathBuf],
    image: Option<&DragImage>,
) -> Result<DragResult, PlatformError> {
    if paths.is_empty() {
        return Ok(DragResult {
            effect: DragEffect::None,
        });
    }
    // SAFETY: binding a handler on a live shell item; SHDoDragDrop runs a
    // modal loop on this (OLE-initialised) thread and uses its default drop
    // source. A single item uses the shell's own data object (richer formats);
    // several use ours, which carries a multi-file `CF_HDROP` list.
    let effect = unsafe {
        let data: IDataObject = if paths.len() == 1 {
            shell_item(&paths[0])?.BindToHandler(None, &BHID_DataObject)?
        } else {
            super::data_object::for_files(paths.to_vec())
        };
        if !super::drop_target::set_drop_effect(
            &data,
            super::drop_target::PREFERRED_DROP_EFFECT,
            DROPEFFECT_MOVE,
        ) {
            tracing::debug!(
                "could not set the preferred drop effect; the target's default applies"
            );
        }
        if let Some(image) = image {
            attach_drag_image(&data, image);
        }
        let _ = ReleaseCapture();
        SHDoDragDrop(
            owner,
            &data,
            None::<&IDropSource>,
            DROPEFFECT_MOVE | DROPEFFECT_COPY | DROPEFFECT_LINK,
        )?
    };
    Ok(DragResult {
        effect: drag_effect(effect),
    })
}

/// Hands `image` to the shell's drag-image manager, which stores it in `data`
/// so every drop target (Explorer, the desktop, other spaces) draws it.
/// Without this the shell invents an opaque, oversized image.
///
/// `IDragSourceHelper2::SetFlags(0)` is used so the drag shows only the item
/// image: a drop target (the desktop, above all) requests a drop description
/// (`CFSTR_DROPDESCRIPTION`), which the shell otherwise draws as
/// `DROPIMAGE_MOVE`'s movement arrow plus a "Move to ..." label. Explorer's own
/// drags do not show that hint when the targets do not request it, so leaving it
/// off keeps drags out of a space identical to drags within one.
/// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-idragsourcehelper
fn attach_drag_image(data: &IDataObject, image: &DragImage) {
    // InitializeFromBitmap premultiplies itself, so it gets straight alpha.
    // https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-idragsourcehelper-initializefrombitmap
    let Some(bitmap) = drag_image::dib(&image.icon, false) else {
        return;
    };
    let drag = SHDRAGIMAGE {
        sizeDragImage: SIZE {
            cx: image.width(),
            cy: image.height(),
        },
        ptOffset: POINT {
            x: image.hotspot.x,
            y: image.hotspot.y,
        },
        hbmpDragImage: bitmap,
        // CLR_NONE: no color key; transparency comes from the alpha channel.
        crColorKey: COLORREF(0xFFFF_FFFF),
    };
    // SAFETY: the helpers are fresh in-process objects; on success one owns the
    // bitmap (the shell's own samples never free it afterwards), so it is
    // deleted here only when the hand-off fails.
    unsafe {
        let attached = CoCreateInstance::<_, IDragSourceHelper2>(
            &CLSID_DragDropHelper,
            None,
            CLSCTX_INPROC_SERVER,
        )
        .and_then(|helper| {
            // A zero flag mask leaves the drop description off, so no movement
            // arrow or "Move" label is drawn; the item image stands alone.
            let _ = helper.SetFlags(0);
            helper.InitializeFromBitmap(&drag, data)
        })
        .or_else(|_| {
            // Pre-Vista shells have only the v1 helper, which cannot suppress
            // the hint; the image still goes through.
            CoCreateInstance::<_, IDragSourceHelper>(
                &CLSID_DragDropHelper,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .and_then(|helper| helper.InitializeFromBitmap(&drag, data))
        });
        if let Err(error) = attached {
            tracing::debug!(%error, "drag image not set; the shell's default applies");
            let _ = DeleteObject(bitmap.into());
        }
    }
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
