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
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, BHID_SFUIObject, CLSID_DragDropHelper, CMF_NORMAL, CMIC_MASK_PTINVOKE,
    CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, IContextMenu, IContextMenu2, IContextMenu3,
    IDataObjectAsyncCapability, IDragSourceHelper, IDragSourceHelper2, ILCreateFromPathW,
    ILFindChild, ILFree, IShellFolder, IShellItem, SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC,
    SHCreateDataObject, SHCreateItemFromParsingName, SHDRAGIMAGE, SHDoDragDrop, SHELLEXECUTEINFOW,
    SHGetDesktopFolder, SVGIO_BACKGROUND, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, CreatePopupMenu, DestroyMenu, GetMenuItemCount, HMENU,
    InsertMenuW, MF_BYPOSITION, MF_SEPARATOR, PostMessageW, SW_SHOWNORMAL, SetForegroundWindow,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_DRAWITEM, WM_INITMENUPOPUP,
    WM_MEASUREITEM, WM_MENUCHAR, WM_NULL,
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
    /// have been moved or deleted; a desktop background command (New, Paste,
    /// Sort by, ...) may have changed the desktop's items.
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
pub(crate) fn shell_path(path: &Path) -> Vec<u16> {
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

/// Opens `url` (for example the release page) in the default browser.
pub(crate) fn open_url(owner: Option<HWND>, url: &str) -> Result<(), PlatformError> {
    let url = wide(url);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        hwnd: owner.unwrap_or_default(),
        lpFile: PCWSTR(url.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and `url` outlives the call. As for
    // `open`, DeskSpace is not the foreground process, so it passes its
    // foreground right on so the browser comes to the front.
    unsafe {
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
    let item_menu = || {
        shell_item(path)
            .and_then(|item| {
                // SAFETY: binding a handler on a live shell item.
                Ok(unsafe { item.BindToHandler(None, &BHID_SFUIObject)? })
            })
            .map_err(|error| tracing::debug!(%error, path = %path.display(), "no shell menu"))
            .ok()
    };
    track_shell_menu(owner, &[&item_menu], entries, at)
}

/// Shows the app's `entries`, a separator, and then the desktop's background
/// menu (View, Sort by, Refresh, New, Personalize, ...) at `at`, as Explorer
/// shows it on an empty spot of the desktop. Takeover hides Explorer's view,
/// so the desktop layer offers this menu in its place.
pub(crate) fn track_desktop_menu(
    owner: HWND,
    entries: &[MenuEntry],
    at: Point,
) -> Option<ItemMenuChoice> {
    track_shell_menu(
        owner,
        &[&desktop_view_menu, &desktop_folder_menu],
        entries,
        at,
    )
}

/// The desktop view's own background menu. Asking the view (not the Desktop
/// folder) is what brings in its View and Sort by submenus and the
/// `DesktopBackground` shell extensions:
/// <https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellview-getitemobject>
fn desktop_view_menu() -> Option<IContextMenu> {
    let Some(view) = crate::win32::shell_view::desktop_shell_view() else {
        tracing::warn!("desktop view not reachable for its background menu");
        return None;
    };
    // SAFETY: `view` is the live desktop view; the call fills an out-parameter
    // with an owned interface.
    unsafe { view.GetItemObject::<IContextMenu>(SVGIO_BACKGROUND) }
        .map_err(|error| tracing::warn!(%error, "no desktop view background menu"))
        .ok()
}

/// The Desktop folder's background menu, built in this process. It lacks the
/// view's View, Sort by, Refresh, and Paste, but keeps New, Display settings,
/// Personalize, and the folder-background extensions, so the desktop menu
/// still works when the view's menu cannot be filled. Seen 2026-10-17: the
/// view's menu object arrived but added no entries, leaving only DeskSpace's
/// own (DeskSpace was probably running elevated).
/// <https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellfolder-createviewobject>
fn desktop_folder_menu() -> Option<IContextMenu> {
    // SAFETY: the desktop folder is a process-wide shell object; both calls
    // return owned interfaces.
    unsafe {
        SHGetDesktopFolder()
            .and_then(|folder: IShellFolder| folder.CreateViewObject(HWND::default()))
    }
    .map_err(|error| tracing::warn!(%error, "no desktop folder background menu"))
    .ok()
}

/// Inserts `shell_menu`'s entries into `menu` at `index`. Returns whether it
/// added any: a handler can report success and add nothing.
///
/// # Safety
/// `menu` must be a live menu.
unsafe fn fill_shell_menu(menu: HMENU, index: u32, shell_menu: &IContextMenu) -> bool {
    // SAFETY: `menu` is live per the caller; the shell only inserts into it.
    unsafe {
        let before = GetMenuItemCount(Some(menu));
        let result =
            shell_menu.QueryContextMenu(menu, index, SHELL_FIRST_ID, SHELL_LAST_ID, CMF_NORMAL);
        let added = GetMenuItemCount(Some(menu)) > before;
        if !added {
            tracing::warn!(?result, "shell context menu added no entries");
        }
        added
    }
}

/// A way to get a shell context menu; `track_shell_menu` tries them in order.
type MenuSource<'a> = &'a dyn Fn() -> Option<IContextMenu>;

/// Tracks `entries`, a separator, and the first of `sources` that adds any
/// entries, as one popup, and runs the shell command chosen. The separator
/// appears only between two non-empty parts.
fn track_shell_menu(
    owner: HWND,
    sources: &[MenuSource],
    entries: &[MenuEntry],
    at: Point,
) -> Option<ItemMenuChoice> {
    // SAFETY: the menu is created, tracked, and destroyed within this call;
    // labels outlive the calls that copy them. The context menu is kept alive
    // in ACTIVE_MENU while tracking and released afterwards.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        crate::win32::menu::append_entries(menu, entries);
        let app_count = GetMenuItemCount(Some(menu)).max(0) as u32;
        let shell_menu = sources
            .iter()
            .filter_map(|source| source())
            .find(|candidate| fill_shell_menu(menu, app_count, candidate));
        if shell_menu.is_some() && app_count > 0 {
            let _ = InsertMenuW(
                menu,
                app_count,
                MF_BYPOSITION | MF_SEPARATOR,
                0,
                PCWSTR::null(),
            );
        }

        ACTIVE_MENU.with(|cell| *cell.borrow_mut() = shell_menu.clone());
        // See menu::track for why the owner must be foreground and why the
        // trailing WM_NULL is posted. TPM_NONOTIFY is deliberately not used:
        // the shell needs WM_INITMENUPOPUP to fill its submenus.
        let _ = SetForegroundWindow(owner);
        // The menu owns every key while it is open; the keyboard hook passes
        // keys through meanwhile.
        crate::win32::keyboard::set_menu_open(true);
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
            at.x,
            at.y,
            owner,
            None,
        )
        .0 as u32;
        crate::win32::keyboard::set_menu_open(false);
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        ACTIVE_MENU.with(|cell| cell.borrow_mut().take());

        let choice = match (chosen, &shell_menu) {
            (0, _) => None,
            (id, Some(shell_menu)) if id >= SHELL_FIRST_ID => {
                // MAKEINTRESOURCE: the verb is the command offset, not a string.
                // The EX form carries the click, so Paste and New place their
                // item there instead of in Explorer's next free slot.
                // https://learn.microsoft.com/windows/win32/api/shobjidl_core/ns-shobjidl_core-cminvokecommandinfoex
                let info = CMINVOKECOMMANDINFOEX {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
                    fMask: CMIC_MASK_PTINVOKE,
                    hwnd: owner,
                    lpVerb: PCSTR((id - SHELL_FIRST_ID) as usize as *const u8),
                    nShow: SW_SHOWNORMAL.0,
                    ptInvoke: POINT { x: at.x, y: at.y },
                    ..Default::default()
                };
                // `cbSize` tells the handler it was given the EX layout, which
                // starts with the plain one.
                let info = std::ptr::from_ref(&info).cast::<CMINVOKECOMMANDINFO>();
                if let Err(error) = shell_menu.InvokeCommand(info) {
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
    positions: &[Point],
    image: Option<&DragImage>,
) -> Result<DragResult, PlatformError> {
    if paths.is_empty() {
        return Ok(DragResult {
            effect: DragEffect::None,
        });
    }
    // SAFETY: binding a handler on a live shell item; SHDoDragDrop runs a
    // modal loop on this (OLE-initialised) thread and uses its default drop
    // source. Both paths yield the shell's own data object.
    let effect = unsafe {
        let data: IDataObject = if paths.len() == 1 {
            shell_item(&paths[0])?.BindToHandler(None, &BHID_DataObject)?
        } else {
            shell_data_object(paths)?
        };
        // The app prunes moved items as soon as the drag returns, so the drop
        // target must finish the transfer inside the drag loop.
        if let Ok(capability) = data.cast::<IDataObjectAsyncCapability>() {
            let _ = capability.SetAsyncMode(false);
        }
        if !super::drop_target::set_drop_effect(
            &data,
            super::drop_target::PREFERRED_DROP_EFFECT,
            DROPEFFECT_MOVE,
        ) {
            tracing::debug!(
                "could not set the preferred drop effect; the target's default applies"
            );
        }
        // Without the group's layout the desktop drops the first item at the
        // pointer and stacks the rest in the next free cells, one column.
        // A single item already lands at the pointer, so it is left alone.
        if paths.len() > 1 && positions.len() == paths.len() {
            let offsets = shell_object_offsets(positions);
            if !super::drop_target::write_format(&data, SHELL_OBJECT_OFFSETS, &offsets) {
                tracing::debug!("could not set the item offsets; the target lays the items out");
            }
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

/// The shell's data object for several items, as Explorer builds for a
/// selection. It carries the shell ID list (`CFSTR_SHELLIDLIST`), which the
/// desktop needs to place dropped icons at the pointer, and accepts the drag
/// image helper's private formats. A hand-written `CF_HDROP`-only object
/// lost both: multi-item drags showed no image and landed in the desktop's
/// top-left cell.
///
/// The items are passed relative to their deepest common folder. When they
/// share no folder (different drives) the desktop is the parent and the IDs
/// are full; the ID list format allows relative IDs of any depth.
/// https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/ns-shlobj_core-cida
/// https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shcreatedataobject
fn shell_data_object(paths: &[PathBuf]) -> Result<IDataObject, PlatformError> {
    let items = paths
        .iter()
        .map(|path| Pidl::from_path(path).ok_or(PlatformError::InvalidFormat("item path")))
        .collect::<Result<Vec<_>, _>>()?;
    let parent = common_parent(paths).and_then(|folder| Pidl::from_path(&folder));
    // SAFETY: every ID list is a live allocation owned by `items` / `parent`,
    // which outlive the calls; ILFindChild returns a pointer into `item` (or
    // null) and SHCreateDataObject copies the IDs it is given.
    unsafe {
        let children: Option<Vec<*const ITEMIDLIST>> = parent.as_ref().and_then(|parent| {
            items
                .iter()
                .map(|item| {
                    let child = ILFindChild(parent.as_ptr(), item.as_ptr());
                    (!child.is_null()).then_some(child.cast_const())
                })
                .collect()
        });
        let object = match (&parent, children) {
            (Some(parent), Some(children)) => {
                SHCreateDataObject(Some(parent.as_ptr()), Some(&children), None::<&IDataObject>)
            }
            _ => {
                let full: Vec<*const ITEMIDLIST> = items.iter().map(Pidl::as_ptr).collect();
                SHCreateDataObject(None, Some(&full), None::<&IDataObject>)
            }
        };
        Ok(object?)
    }
}

/// `CFSTR_SHELLIDLISTOFFSET`: where a dropped group's items go relative to
/// each other.
/// https://learn.microsoft.com/en-us/windows/win32/shell/clipboard#cfstr_shellidlistoffset
const SHELL_OBJECT_OFFSETS: &str = "Shell Object Offsets";

/// The `CFSTR_SHELLIDLISTOFFSET` payload for items at screen `positions`: an
/// array of `POINT`s, first the group's top-left corner in screen pixels, then
/// each item relative to it, in item order.
fn shell_object_offsets(positions: &[Point]) -> Vec<u8> {
    let origin = Point::new(
        positions.iter().map(|point| point.x).min().unwrap_or(0),
        positions.iter().map(|point| point.y).min().unwrap_or(0),
    );
    std::iter::once(origin)
        .chain(
            positions
                .iter()
                .map(|point| Point::new(point.x - origin.x, point.y - origin.y)),
        )
        .flat_map(|point| {
            let mut bytes = [0u8; 8];
            bytes[..4].copy_from_slice(&point.x.to_le_bytes());
            bytes[4..].copy_from_slice(&point.y.to_le_bytes());
            bytes
        })
        .collect()
}

/// The deepest folder containing every path, or `None` when they share no
/// rooted folder (for example, files on different drives).
fn common_parent(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut parents = paths.iter().map(|path| path.parent());
    let first: Vec<_> = parents.next()??.components().collect();
    let mut shared = first.len();
    for parent in parents {
        shared = first
            .iter()
            .zip(parent?.components())
            .take(shared)
            .take_while(|(a, b)| **a == *b)
            .count();
    }
    let folder: PathBuf = first[..shared].iter().collect();
    folder.has_root().then_some(folder)
}

/// An absolute shell ID list, freed on drop.
struct Pidl(*mut ITEMIDLIST);

impl Pidl {
    fn from_path(path: &Path) -> Option<Self> {
        let wide = shell_path(path);
        // SAFETY: `wide` is NUL-terminated and outlives the call.
        let pidl = unsafe { ILCreateFromPathW(PCWSTR(wide.as_ptr())) };
        (!pidl.is_null()).then_some(Self(pidl))
    }

    fn as_ptr(&self) -> *const ITEMIDLIST {
        self.0
    }
}

impl Drop for Pidl {
    fn drop(&mut self) {
        // SAFETY: allocated by ILCreateFromPathW and freed exactly once here.
        unsafe { ILFree(Some(self.0)) };
    }
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

    fn points(bytes: &[u8]) -> Vec<(i32, i32)> {
        bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|pair| {
                (
                    i32::from_le_bytes(pair[..4].try_into().unwrap()),
                    i32::from_le_bytes(pair[4..].try_into().unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn shell_object_offsets_lead_with_the_group_corner() {
        let positions = [
            Point::new(300, 120),
            Point::new(396, 120),
            Point::new(300, 224),
        ];
        assert_eq!(
            points(&shell_object_offsets(&positions)),
            [(300, 120), (0, 0), (96, 0), (0, 104)]
        );
    }

    #[test]
    fn shell_object_offsets_keep_item_order_and_handle_negative_screens() {
        // A monitor left of the primary has negative coordinates, and the
        // first item need not be the top-left one.
        let positions = [Point::new(-100, 50), Point::new(-196, 154)];
        assert_eq!(
            points(&shell_object_offsets(&positions)),
            [(-196, 50), (96, 0), (0, 104)]
        );
    }

    #[test]
    fn common_parent_is_the_shared_folder() {
        let paths = [
            PathBuf::from(r"C:\store\a.txt"),
            PathBuf::from(r"C:\store\b.txt"),
        ];
        assert_eq!(common_parent(&paths), Some(PathBuf::from(r"C:\store")));
    }

    #[test]
    fn common_parent_walks_up_to_the_deepest_shared_ancestor() {
        let paths = [
            PathBuf::from(r"C:\Users\me\Desktop\a.txt"),
            PathBuf::from(r"C:\Users\me\store\b.txt"),
            PathBuf::from(r"C:\Users\me\c.txt"),
        ];
        assert_eq!(common_parent(&paths), Some(PathBuf::from(r"C:\Users\me")));
    }

    #[test]
    fn common_parent_can_be_a_drive_root() {
        let paths = [PathBuf::from(r"C:\a\x.txt"), PathBuf::from(r"C:\b\y.txt")];
        assert_eq!(common_parent(&paths), Some(PathBuf::from(r"C:\")));
    }

    #[test]
    fn different_drives_share_no_parent() {
        let paths = [PathBuf::from(r"C:\a\x.txt"), PathBuf::from(r"D:\a\y.txt")];
        assert_eq!(common_parent(&paths), None);
    }

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
