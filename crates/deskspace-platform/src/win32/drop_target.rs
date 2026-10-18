//! OLE drop target for space windows, the desktop layer's included: the app
//! opens a new space for a drop on the layer (ROADMAP D7).
//!
//! Registering an `IDropTarget` makes a space accept file drags directly and,
//! unlike the legacy `WM_DROPFILES` path, lets it choose the drop effect. That
//! choice is what drives the move/copy cursor and tells the drag source whether
//! to keep its original, so it replaces `WS_EX_ACCEPTFILES` entirely.
//!
//! Modifier meaning matches the rest of DeskSpace (see `docs/CONTEXT.md`): a drop
//! moves the file into the space, and Shift keeps the original in place (the
//! space refers to it). This is intentionally the opposite of Explorer's
//! Shift=move, but it matches the app's "Shift preserves the original"
//! convention and the drag-out behavior.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::mem::{ManuallyDrop, size_of};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use deskspace_core::geometry::Point;

use windows::Win32::Foundation::{GlobalFree, HWND, POINT, POINTL};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IDataObject,
    STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::{GetClipboardFormatNameW, RegisterClipboardFormatW};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE,
    IDropTarget, IDropTarget_Impl, MK_ALT, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::{MK_RBUTTON, MK_SHIFT, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{
    CLSID_DragDropHelper, DROPDESCRIPTION, DROPIMAGE_NOIMAGE, DROPIMAGETYPE, DragQueryFileW, HDROP,
    IDropTargetHelper, IShellItemArray, SHCreateShellItemArrayFromDataObject,
};
use windows::Win32::UI::WindowsAndMessaging::GetParent;
use windows::core::{PCWSTR, Ref, Result as WinResult};

use crate::error::PlatformError;
use crate::input::{DropAction, Event, ShellItem, SpaceEvent};
use crate::ui::MenuEntry;
use crate::win32::menu;
use crate::win32::runtime;
use crate::win32::shortcuts::shell_item;
use crate::win32::space_animation;
use crate::win32::system::wide;

/// Clipboard format naming the effect a drag source would like the target to
/// use. Setting it to `DROPEFFECT_MOVE` makes dragging an item out of a space
/// move the file (for example onto the desktop) instead of leaving a copy.
/// https://learn.microsoft.com/en-us/windows/win32/shell/clipboard#cfstr_preferreddropeffect
pub(crate) const PREFERRED_DROP_EFFECT: &str = "Preferred DropEffect";

/// Clipboard format a target sets on the source's data object after it moved
/// the files itself, so the source does not move them again (optimized move).
/// https://learn.microsoft.com/en-us/windows/win32/shell/clipboard#cfstr_performeddropeffect
const PERFORMED_DROP_EFFECT: &str = "Performed DropEffect";

/// The Alt key in a drag's key state. OLE defines `MK_ALT` for drag and drop
/// only, so the `windows` crate has it as a plain `u32`, not as one of the
/// `MODIFIERKEYS_FLAGS`.
/// https://learn.microsoft.com/en-us/windows/win32/api/oleidl/nf-oleidl-idroptarget-dragenter
const ALT_KEY: MODIFIERKEYS_FLAGS = MODIFIERKEYS_FLAGS(MK_ALT);

/// What the drag currently over the window carries, remembered from
/// `DragEnter` because `DragOver` gets no data object. What a drop does with
/// it is `drop_plan`'s decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragKind {
    /// Nothing a space accepts.
    None,
    /// A `CF_HDROP` list of real filesystem paths.
    Files,
    /// Shell items without a usable file list: a Start-menu app, a taskbar
    /// app, a packaged app.
    ShellItems,
}

/// What a drop onto a space does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropPlan {
    /// The space refuses the drag.
    None,
    /// The files go into the space as the action says: moved into the
    /// storage, copied there, or listed where they are.
    Files(DropAction),
    /// The space gets a shortcut to each dragged item; nothing is moved.
    Shortcuts,
}

#[windows_core::implement(IDropTarget)]
struct SpaceDropTarget {
    id: u64,
    window: HWND,
    hosted: bool,
    /// The kind of the current drag, remembered from `DragEnter`.
    accepts: Cell<DragKind>,
    /// Whether the current drag started with the right button down, remembered
    /// from `DragEnter`. A right drop shows a menu instead of using the
    /// modifiers, as Explorer does.
    right_drag: Cell<bool>,
}

impl SpaceDropTarget {
    /// What a drag carries, or nothing while the space is on its way out
    /// (hiding, or deleted and fading), which takes no drops (ROADMAP D12).
    fn kind_of(&self, data: Option<&IDataObject>) -> DragKind {
        if space_animation::is_leaving(self.window) {
            return DragKind::None;
        }
        drag_kind(data)
    }
}

impl IDropTarget_Impl for SpaceDropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        // SAFETY: OLE passes a valid, writable effect; the data object, if any,
        // is borrowed only for the duration of the call.
        let allowed = unsafe { *effect };
        let kind = self.kind_of(data.as_ref());
        let right = keys.contains(MK_RBUTTON);
        self.right_drag.set(right);
        tracing::debug!(
            space = self.id,
            allowed = allowed.0,
            keys = keys.0,
            ?kind,
            right,
            "drag entered space"
        );
        if let Some(data) = data.as_ref() {
            log_formats(data);
        }
        self.accepts.set(kind);
        let chosen = drag_effect(kind, right, keys, allowed);
        // SAFETY: as above.
        unsafe { *effect = chosen };
        // A shell source (Explorer, the desktop) puts a default action hint
        // like "Move" on the data object; clearing it here leaves the drag
        // showing only the item image, matching a DeskSpace-originated drag.
        if let Some(data) = data.as_ref().filter(|_| kind != DragKind::None) {
            let _ = set_drop_description(data, DROPIMAGE_NOIMAGE);
        }
        with_image_helper(|helper| {
            // SAFETY: forwarding this call's own arguments.
            unsafe { helper.DragEnter(self.window, data.as_ref(), &screen(point), chosen) }
        });
        Ok(())
    }

    fn DragOver(
        &self,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        // SAFETY: OLE passes a valid, writable effect.
        let allowed = unsafe { *effect };
        // A space that starts to hide under a drag stops taking it.
        let kind = if space_animation::is_leaving(self.window) {
            DragKind::None
        } else {
            self.accepts.get()
        };
        let chosen = drag_effect(kind, self.right_drag.get(), keys, allowed);
        // SAFETY: as above.
        unsafe { *effect = chosen };
        // SAFETY: forwarding this call's own arguments.
        with_image_helper(|helper| unsafe { helper.DragOver(&screen(point), chosen) });
        Ok(())
    }

    fn DragLeave(&self) -> WinResult<()> {
        self.accepts.set(DragKind::None);
        self.right_drag.set(false);
        // SAFETY: matches the forwarded DragEnter.
        with_image_helper(|helper| unsafe { helper.DragLeave() });
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        // SAFETY: OLE passes a valid, writable effect and a live data object.
        let allowed = unsafe { *effect };
        let data = data.as_ref();
        let kind = self.kind_of(data);
        let right = self.right_drag.replace(false);
        self.accepts.set(DragKind::None);
        // The image is removed before the app runs: handling the drop can take
        // a moment (file moves) and must not leave the image frozen on screen.
        let (plan, chosen) = if right && kind == DragKind::Files {
            // A right drag carrying files shows a menu, and the chosen entry
            // decides the plan and the effect. The menu returns before this
            // does, so the effect is known to the source when the drag
            // resumes. The menu can be open a while, so the image goes first.
            // SAFETY: forwarding this call's own arguments.
            with_image_helper(|helper| unsafe {
                helper.Drop(data, &screen(point), DROPEFFECT_NONE)
            });
            let plan = ask_right_drop(self.window, point, allowed);
            (plan, right_drop_effect(plan))
        } else {
            let (plan, chosen) = drop_plan(kind, keys, allowed);
            // SAFETY: forwarding this call's own arguments.
            with_image_helper(|helper| unsafe { helper.Drop(data, &screen(point), chosen) });
            (plan, chosen)
        };
        // SAFETY: as above.
        unsafe { *effect = chosen };
        let Some(data) = data else {
            return Ok(());
        };
        let (x, y) = desktop_point(self.window, self.hosted, point);
        match plan {
            DropPlan::None => {}
            DropPlan::Files(action) => self.drop_files(data, x, y, action),
            DropPlan::Shortcuts => self.drop_shortcuts(data, x, y),
        }
        Ok(())
    }
}

impl SpaceDropTarget {
    /// Raises `FilesDropped` for the drag's file list, telling the app which
    /// `action` the user chose (a modifier on a left drop, the menu on a right
    /// one).
    fn drop_files(&self, data: &IDataObject, x: i32, y: i32, action: DropAction) {
        let Some(paths) = file_paths(data).filter(|paths| !paths.is_empty()) else {
            return;
        };
        tracing::debug!(space = self.id, ?action, ?paths, "files dropped on space");
        runtime::dispatch(Event::Space {
            id: self.id,
            event: SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                action,
            },
        });
        // A move is performed by the app itself (the event above runs before
        // this returns for drags that started in another process). Marking it
        // as already performed stops the source from moving the files a second
        // time; for an in-process (space-to-space) drag it is a no-op, because
        // DeskSpace's own source never deletes on `Move`. A copy or keep-in-
        // place leaves the source's original alone, so nothing is marked.
        if action == DropAction::Move {
            let _ = set_drop_effect(data, PERFORMED_DROP_EFFECT, DROPEFFECT_MOVE);
        }
    }

    /// Raises `ShellItemsDropped` for the drag's items, whether the source
    /// expressed them as a file list or a Shell ID List. Nothing is moved, so
    /// "Performed DropEffect" is left unset.
    fn drop_shortcuts(&self, data: &IDataObject, x: i32, y: i32) {
        let Some(items) = shell_items(data) else {
            return;
        };
        tracing::debug!(
            space = self.id,
            names = ?items.iter().map(|item| &item.name).collect::<Vec<_>>(),
            "shell items dropped on space"
        );
        runtime::dispatch(Event::Space {
            id: self.id,
            event: SpaceEvent::ShellItemsDropped { x, y, items },
        });
    }
}

thread_local! {
    /// The shell's drag-image manager, shared by every space. A drop target
    /// must forward its four calls to it, or the source's drag image is not
    /// drawn while the pointer is over the target's window.
    /// https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-idroptargethelper
    /// Created on first use; released by `release_image_helper` before OLE
    /// is uninitialized.
    static IMAGE_HELPER: RefCell<Option<IDropTargetHelper>> = const { RefCell::new(None) };
}

/// Runs `f` with the drag-image helper; the helper's failures only affect the
/// image, never the drop, so they are ignored.
fn with_image_helper(f: impl FnOnce(&IDropTargetHelper) -> WinResult<()>) {
    // Cloned out so no borrow is held while the shell runs.
    let helper = IMAGE_HELPER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            // SAFETY: creating an in-process object on the OLE-initialised UI
            // thread, where drop target calls arrive.
            *slot = unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| tracing::debug!(%error, "no drag-image helper"))
                .ok();
        }
        slot.clone()
    });
    if let Some(helper) = helper {
        let _ = f(&helper);
    }
}

/// Releases the shared helper; call before `OleUninitialize`.
pub(crate) fn release_image_helper() {
    IMAGE_HELPER.with(|cell| cell.borrow_mut().take());
}

fn screen(point: &POINTL) -> POINT {
    POINT {
        x: point.x,
        y: point.y,
    }
}

/// Registers `window` as an OLE drop target. The window must already exist and
/// OLE must be initialized on this thread (the runtime does both). A failure is
/// not fatal: the space simply will not accept drops.
pub(crate) fn register(window: HWND, id: u64, hosted: bool) -> Result<(), PlatformError> {
    let target: IDropTarget = SpaceDropTarget {
        id,
        window,
        hosted,
        accepts: Cell::new(DragKind::None),
        right_drag: Cell::new(false),
    }
    .into();
    // SAFETY: `window` was created by this runtime and OLE is initialized on
    // this thread. RegisterDragDrop takes its own reference, so `target` may be
    // dropped when this returns.
    unsafe { RegisterDragDrop(window, &target)? };
    Ok(())
}

/// Removes the drop target registered by `register`. Must be called before the
/// window is destroyed.
pub(crate) fn revoke(window: HWND) {
    // SAFETY: matching the RegisterDragDrop for this window.
    unsafe {
        let _ = RevokeDragDrop(window);
    }
}

/// Writes `effect` under the named drop-effect clipboard format into `data`,
/// transferring a 4-byte `DROPEFFECT` to the object. Returns whether the object
/// accepted it.
pub(crate) fn set_drop_effect(data: &IDataObject, name: &str, effect: DROPEFFECT) -> bool {
    write_format(data, name, &effect.0.to_le_bytes())
}

/// Clears the drag's action hint by storing an empty drop description with the
/// given stock image. A shell source (Explorer, the desktop) sets a default
/// "Move"/"Copy" label on the data object; overriding it here leaves only the
/// item image, as DeskSpace's own drags show.
/// https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/ns-shlobj_core-dropdescription
pub(crate) fn set_drop_description(data: &IDataObject, image: DROPIMAGETYPE) -> bool {
    let description = DROPDESCRIPTION {
        r#type: image,
        ..Default::default()
    };
    // SAFETY: reading the plain-old-data struct's bytes for the copy below.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&description as *const DROPDESCRIPTION).cast::<u8>(),
            size_of::<DROPDESCRIPTION>(),
        )
    };
    write_format(data, "DropDescription", bytes)
}

/// Copies `bytes` into a movable global block and hands it to `data` under the
/// registered clipboard format `name`. Returns whether the object accepted it.
pub(crate) fn write_format(data: &IDataObject, name: &str, bytes: &[u8]) -> bool {
    let name = wide(name);
    // SAFETY: the name is NUL-terminated and outlives the call.
    let format = unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) };
    if format == 0 {
        return false;
    }
    // SAFETY: allocating a movable block, copying the bytes in, then handing
    // ownership to the data object (or freeing it on failure).
    let Ok(block) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }) else {
        return false;
    };
    // SAFETY: `block` is a live movable block allocated just above.
    let pointer = unsafe { GlobalLock(block) };
    if pointer.is_null() {
        // SAFETY: the data object never received `block`; it is ours to free.
        let _ = unsafe { GlobalFree(Some(block)) };
        return false;
    }
    // SAFETY: `block` holds at least `bytes.len()` bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len()) };
    // SAFETY: `block` is locked and was fully written above.
    let _ = unsafe { GlobalUnlock(block) };

    let format_etc = FORMATETC {
        cfFormat: format as u16,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    let medium = STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: block },
        pUnkForRelease: ManuallyDrop::new(None),
    };
    // SAFETY: `fRelease: true` transfers the block to `data` on success; on
    // failure the block is still ours and is released below.
    if unsafe { data.SetData(&format_etc, &medium, true) }.is_ok() {
        return true;
    }
    let mut medium = medium;
    // SAFETY: the medium was not taken by the failed SetData.
    unsafe { ReleaseStgMedium(&mut medium) };
    false
}

/// What a drop of `kind` does, and the effect reported to the source, given
/// the held `keys` and the effects the source `allowed`. As in Explorer, a
/// `LINK` drop always makes shortcuts: a file list links when Alt is held
/// (Explorer's create-shortcut modifier; its Ctrl+Shift is not used, because
/// Shift means keep-in-place on a space) or when the source offers neither
/// `MOVE` nor `COPY`. Otherwise files move, or stay put with Shift.
fn drop_plan(
    kind: DragKind,
    keys: MODIFIERKEYS_FLAGS,
    allowed: DROPEFFECT,
) -> (DropPlan, DROPEFFECT) {
    let allows = |effect: DROPEFFECT| allowed.0 & effect.0 != 0;
    match kind {
        DragKind::None => (DropPlan::None, DROPEFFECT_NONE),
        DragKind::ShellItems => match shell_drop_effect(allowed) {
            DROPEFFECT_NONE => (DropPlan::None, DROPEFFECT_NONE),
            effect => (DropPlan::Shortcuts, effect),
        },
        DragKind::Files if keys.contains(ALT_KEY) && allows(DROPEFFECT_LINK) => {
            (DropPlan::Shortcuts, DROPEFFECT_LINK)
        }
        DragKind::Files => match file_drop_effect(keys, allowed) {
            DROPEFFECT_MOVE => (DropPlan::Files(DropAction::Move), DROPEFFECT_MOVE),
            DROPEFFECT_COPY => (DropPlan::Files(DropAction::KeepInPlace), DROPEFFECT_COPY),
            _ if allows(DROPEFFECT_LINK) => (DropPlan::Shortcuts, DROPEFFECT_LINK),
            _ => (DropPlan::None, DROPEFFECT_NONE),
        },
    }
}

/// The move-or-copy effect for a file drop, limited to `allowed`. Shift
/// preserves the original, so it asks for a copy; otherwise the drop moves the
/// file into the space. `NONE` when the source offers neither.
fn file_drop_effect(keys: MODIFIERKEYS_FLAGS, allowed: DROPEFFECT) -> DROPEFFECT {
    let preferred = if keys.contains(MK_SHIFT) {
        [DROPEFFECT_COPY, DROPEFFECT_MOVE]
    } else {
        [DROPEFFECT_MOVE, DROPEFFECT_COPY]
    };
    preferred
        .into_iter()
        .find(|want| allowed.0 & want.0 != 0)
        .unwrap_or(DROPEFFECT_NONE)
}

/// The effect to report while a drag hovers over the space. A right drag
/// carrying files reports `MOVE` whatever the modifiers, because the drop menu
/// decides the real action and the cursor should not flicker as keys change;
/// everything else follows `drop_plan`.
fn drag_effect(
    kind: DragKind,
    right: bool,
    keys: MODIFIERKEYS_FLAGS,
    allowed: DROPEFFECT,
) -> DROPEFFECT {
    if right && kind == DragKind::Files && allowed.0 & DROPEFFECT_MOVE.0 != 0 {
        DROPEFFECT_MOVE
    } else {
        drop_plan(kind, keys, allowed).1
    }
}

/// The effect a right drop reports for a chosen plan: a move moves, so the
/// source deletes its original; a copy or keep-in-place leaves it alone, which
/// `COPY` tells the source; shortcuts link.
fn right_drop_effect(plan: DropPlan) -> DROPEFFECT {
    match plan {
        DropPlan::None => DROPEFFECT_NONE,
        DropPlan::Files(DropAction::Move) => DROPEFFECT_MOVE,
        DropPlan::Files(DropAction::Copy | DropAction::KeepInPlace) => DROPEFFECT_COPY,
        DropPlan::Shortcuts => DROPEFFECT_LINK,
    }
}

/// The right-drag drop menu's entry ids. Non-zero because `TrackPopupMenuEx`
/// returns 0 for a dismissed menu.
const DROP_MOVE: u32 = 1;
const DROP_COPY: u32 = 2;
const DROP_KEEP: u32 = 3;
const DROP_CANCEL: u32 = 4;
const DROP_LINK: u32 = 5;

/// The entries of the menu a right drag shows on a space, in order. "Create
/// shortcuts here" is greyed when the source does not `allow` a link, so the
/// menu never reports an effect the source refused.
fn right_drop_entries(allowed: DROPEFFECT) -> Vec<MenuEntry> {
    vec![
        MenuEntry::item(DROP_MOVE, "Move here"),
        MenuEntry::item(DROP_COPY, "Copy here"),
        MenuEntry::item(DROP_LINK, "Create shortcuts here")
            .enabled(allowed.0 & DROPEFFECT_LINK.0 != 0),
        MenuEntry::item(DROP_KEEP, "Show here without moving"),
        MenuEntry::Separator,
        MenuEntry::item(DROP_CANCEL, "Cancel"),
    ]
}

/// What a chosen menu entry means. `DropPlan::None` for Cancel, and for a
/// dismissed menu (whose id 0 matches no arm), so the drop reports no effect
/// and raises no event.
fn right_drop_choice(id: u32) -> DropPlan {
    match id {
        DROP_MOVE => DropPlan::Files(DropAction::Move),
        DROP_COPY => DropPlan::Files(DropAction::Copy),
        DROP_KEEP => DropPlan::Files(DropAction::KeepInPlace),
        DROP_LINK => DropPlan::Shortcuts,
        _ => DropPlan::None,
    }
}

/// Shows the right-drag menu at the (screen) drop `point` and returns the
/// chosen plan, `DropPlan::None` when the user cancelled or dismissed it. The
/// menu is owned by the space window; no runtime state is read across the
/// call.
fn ask_right_drop(window: HWND, point: &POINTL, allowed: DROPEFFECT) -> DropPlan {
    let entries = right_drop_entries(allowed);
    menu::track(window, &entries, Point::new(point.x, point.y))
        .map_or(DropPlan::None, right_drop_choice)
}

fn drop_files_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// What `data` offers a space. A list of real files wins over shell items: a
/// drag can carry both, and a real file should move, not become a shortcut.
/// The Start menu also puts a `CF_HDROP` on its drags, but it names apps by
/// their AppUserModelID (for example `com.squirrel.element-desktop.Element`),
/// not by path, so such a list counts as shell items (`TASKS.md` L1).
fn drag_kind(data: Option<&IDataObject>) -> DragKind {
    let Some(data) = data else {
        return DragKind::None;
    };
    if file_paths(data).is_some_and(|paths| is_file_list(&paths)) {
        DragKind::Files
    } else if has_shell_items(data) {
        DragKind::ShellItems
    } else {
        DragKind::None
    }
}

/// Whether a `CF_HDROP` list names real files: it is not empty and every
/// entry is an absolute path. A path that is absolute but missing (a stale
/// desktop icon) still counts, so the app reports and skips it as before.
fn is_file_list(paths: &[PathBuf]) -> bool {
    !paths.is_empty() && paths.iter().all(|path| path.is_absolute())
}

/// Whether `data` describes shell items (a Start-menu tile, a taskbar app,
/// including a packaged app with no file). The shell builds an item array from
/// whatever form the source used (`CFSTR_SHELLIDLIST`, a file descriptor
/// array, ...), which covers virtual drags that carry no `CF_HDROP`; a drag
/// the shell cannot name yields nothing.
fn has_shell_items(data: &IDataObject) -> bool {
    shell_item_array(data).is_some_and(|array| {
        // SAFETY: the array is live.
        unsafe { array.GetCount() }.is_ok_and(|count| count > 0)
    })
}

/// The shell's view of a drag's items, however the source expressed them, or
/// `None` when the object carries no shell items (for example plain text).
/// https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shcreateshellitemarrayfromdataobject
fn shell_item_array(data: &IDataObject) -> Option<IShellItemArray> {
    // SAFETY: `data` is a live data object; the helper builds a fresh array.
    unsafe { SHCreateShellItemArrayFromDataObject(data).ok() }
}

/// The effect a space reports for a virtual shell item: a shortcut is created,
/// so ask for a link - the cursor Explorer shows for this drag - then a copy,
/// then a move, always avoiding a move when the source offers anything else,
/// because a move would ask the source to destroy the original.
fn shell_drop_effect(allowed: DROPEFFECT) -> DROPEFFECT {
    [DROPEFFECT_LINK, DROPEFFECT_COPY, DROPEFFECT_MOVE]
        .into_iter()
        .find(|want| allowed.0 & want.0 != 0)
        .unwrap_or(DROPEFFECT_NONE)
}

/// The shell items a data object carries (a drag or the clipboard), each with
/// Explorer's display name and an absolute ID list that `create_shortcut` can
/// rebuild.
pub(crate) fn shell_items(data: &IDataObject) -> Option<Vec<ShellItem>> {
    let array = shell_item_array(data)?;
    // SAFETY: the array is live.
    let count = unsafe { array.GetCount() }.ok()?;
    let mut items = Vec::with_capacity(count as usize);
    for index in 0..count {
        // SAFETY: `index` is below the array's count.
        let Ok(item) = (unsafe { array.GetItemAt(index) }) else {
            continue;
        };
        if let Some(item) = shell_item(&item) {
            items.push(item);
        }
    }
    (!items.is_empty()).then_some(items)
}

/// Logs the formats a data object offers, so a drag source's behavior can be
/// diagnosed (the Start menu and the taskbar are undocumented). Called once
/// per `DragEnter`, so it is never on a hot path.
fn log_formats(data: &IDataObject) {
    // SAFETY: `data` is a live data object; the enumerator is released on drop.
    let Ok(enumerator) = (unsafe { data.EnumFormatEtc(DATADIR_GET.0 as u32) }) else {
        return;
    };
    let mut format = [FORMATETC::default(); 1];
    loop {
        let mut fetched = 0u32;
        // SAFETY: a one-element buffer and a valid count out-pointer.
        let status = unsafe { enumerator.Next(&mut format, Some(&mut fetched)) };
        if status.is_err() || fetched == 0 {
            break;
        }
        tracing::debug!(
            cf = format[0].cfFormat,
            name = %clipboard_format_name(format[0].cfFormat),
            tymed = format[0].tymed,
            aspect = format[0].dwAspect,
            "drag offered a format"
        );
    }
}

/// A clipboard format's registered name, or its number for the standard
/// formats (for example `CF_HDROP` is 15).
fn clipboard_format_name(format: u16) -> String {
    if format as u32 >= 0xC000 {
        let mut buffer = [0u16; 256];
        // SAFETY: `buffer` is writable and its length is passed.
        let length = unsafe { GetClipboardFormatNameW(format as u32, &mut buffer) };
        if length > 0 {
            return String::from_utf16_lossy(&buffer[..length as usize]);
        }
    }
    format.to_string()
}

/// Reads the `CF_HDROP` file list out of `data`, releasing the medium. Returns
/// `None` when the data object has no usable global file list.
fn file_paths(data: &IDataObject) -> Option<Vec<PathBuf>> {
    // SAFETY: `GetData` returns a medium owned by us; it is released exactly
    // once at the end.
    let mut medium = unsafe { data.GetData(&drop_files_format()).ok()? };
    // SAFETY: the tymed and handle are checked before the list is read.
    let paths = unsafe {
        if medium.tymed != TYMED_HGLOBAL.0 as u32 || medium.u.hGlobal.0.is_null() {
            None
        } else {
            Some(read_drop(HDROP(medium.u.hGlobal.0)))
        }
    };
    // SAFETY: releasing the medium returned by GetData.
    unsafe { ReleaseStgMedium(&mut medium) };
    paths
}

/// # Safety
/// `drop` must be a live `HDROP`.
unsafe fn read_drop(drop: HDROP) -> Vec<PathBuf> {
    // SAFETY: buffers are sized from DragQueryFileW's reported length plus NUL.
    unsafe {
        let count = DragQueryFileW(drop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = DragQueryFileW(drop, index, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            let copied = DragQueryFileW(drop, index, Some(&mut buffer)) as usize;
            paths.push(PathBuf::from(OsString::from_wide(&buffer[..copied])));
        }
        paths
    }
}

/// The drop point in `Space::rect` coordinates. OLE reports screen pixels;
/// hosted spaces live in the desktop host's coordinate space.
fn desktop_point(window: HWND, hosted: bool, point: &POINTL) -> (i32, i32) {
    let mut point = [POINT {
        x: point.x,
        y: point.y,
    }];
    if hosted {
        // SAFETY: plain coordinate mapping between this window and its parent.
        unsafe { MapWindowPoints(None, GetParent(window).ok(), &mut point) };
    }
    (point[0].x, point[0].y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: DROPEFFECT = DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_COPY.0 | DROPEFFECT_LINK.0);
    const NO_KEYS: MODIFIERKEYS_FLAGS = MODIFIERKEYS_FLAGS(0);

    #[test]
    fn files_move_by_default_and_stay_with_shift() {
        let plan = |keys| drop_plan(DragKind::Files, keys, ALL);
        assert_eq!(
            plan(NO_KEYS),
            (DropPlan::Files(DropAction::Move), DROPEFFECT_MOVE)
        );
        assert_eq!(
            plan(MK_SHIFT),
            (DropPlan::Files(DropAction::KeepInPlace), DROPEFFECT_COPY)
        );
    }

    #[test]
    fn files_fall_back_when_the_preferred_effect_is_not_allowed() {
        assert_eq!(
            drop_plan(DragKind::Files, NO_KEYS, DROPEFFECT_COPY),
            (DropPlan::Files(DropAction::KeepInPlace), DROPEFFECT_COPY)
        );
        assert_eq!(
            drop_plan(DragKind::Files, MK_SHIFT, DROPEFFECT_MOVE),
            (DropPlan::Files(DropAction::Move), DROPEFFECT_MOVE)
        );
    }

    #[test]
    fn files_offered_only_as_a_link_become_shortcuts() {
        assert_eq!(
            drop_plan(DragKind::Files, NO_KEYS, DROPEFFECT_LINK),
            (DropPlan::Shortcuts, DROPEFFECT_LINK)
        );
    }

    #[test]
    fn alt_makes_shortcuts_when_a_link_is_allowed() {
        assert_eq!(
            drop_plan(DragKind::Files, ALT_KEY, ALL),
            (DropPlan::Shortcuts, DROPEFFECT_LINK)
        );
        let move_or_copy = DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_COPY.0);
        assert_eq!(
            drop_plan(DragKind::Files, ALT_KEY, move_or_copy),
            (DropPlan::Files(DropAction::Move), DROPEFFECT_MOVE)
        );
    }

    #[test]
    fn shell_items_become_shortcuts_preferring_a_link() {
        let plan = |allowed| drop_plan(DragKind::ShellItems, NO_KEYS, allowed);
        assert_eq!(plan(ALL), (DropPlan::Shortcuts, DROPEFFECT_LINK));
        assert_eq!(
            plan(DROPEFFECT_COPY),
            (DropPlan::Shortcuts, DROPEFFECT_COPY)
        );
        assert_eq!(
            plan(DROPEFFECT_MOVE),
            (DropPlan::Shortcuts, DROPEFFECT_MOVE)
        );
    }

    #[test]
    fn nothing_allowed_or_nothing_carried_refuses_the_drop() {
        let refused = (DropPlan::None, DROPEFFECT_NONE);
        assert_eq!(
            drop_plan(DragKind::Files, NO_KEYS, DROPEFFECT_NONE),
            refused
        );
        assert_eq!(
            drop_plan(DragKind::ShellItems, NO_KEYS, DROPEFFECT_NONE),
            refused
        );
        assert_eq!(drop_plan(DragKind::None, NO_KEYS, ALL), refused);
    }

    #[test]
    fn a_start_menu_app_id_is_not_a_file_list() {
        let app_id = PathBuf::from("com.squirrel.element-desktop.Element");
        let known_folder = PathBuf::from(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\notepad.exe");
        assert!(!is_file_list(&[app_id]));
        assert!(!is_file_list(&[known_folder]));
        assert!(!is_file_list(&[]));
    }

    #[test]
    fn absolute_paths_are_a_file_list() {
        let paths = [
            PathBuf::from(r"C:\Users\me\Desktop\notes.txt"),
            PathBuf::from(r"\\server\share\report.docx"),
        ];
        assert!(is_file_list(&paths));
        let mixed = [paths[0].clone(), PathBuf::from("relative.txt")];
        assert!(!is_file_list(&mixed));
    }

    /// The ids of `entries`' items, and whether each is enabled, in order.
    fn item_states(entries: &[MenuEntry]) -> Vec<(u32, bool)> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { id, enabled, .. } => Some((*id, *enabled)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_right_drop_menu_offers_shortcuts_after_copy() {
        let ids: Vec<u32> = item_states(&right_drop_entries(ALL))
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            ids,
            [DROP_MOVE, DROP_COPY, DROP_LINK, DROP_KEEP, DROP_CANCEL]
        );
    }

    #[test]
    fn create_shortcuts_is_greyed_when_the_source_refuses_a_link() {
        let link_enabled = |allowed| {
            item_states(&right_drop_entries(allowed))
                .into_iter()
                .find(|&(id, _)| id == DROP_LINK)
                .map(|(_, enabled)| enabled)
        };
        assert_eq!(link_enabled(ALL), Some(true));
        let move_or_copy = DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_COPY.0);
        assert_eq!(link_enabled(move_or_copy), Some(false));
    }

    #[test]
    fn right_drop_choice_maps_each_entry() {
        assert_eq!(
            right_drop_choice(DROP_MOVE),
            DropPlan::Files(DropAction::Move)
        );
        assert_eq!(
            right_drop_choice(DROP_COPY),
            DropPlan::Files(DropAction::Copy)
        );
        assert_eq!(
            right_drop_choice(DROP_KEEP),
            DropPlan::Files(DropAction::KeepInPlace)
        );
        assert_eq!(right_drop_choice(DROP_LINK), DropPlan::Shortcuts);
        assert_eq!(right_drop_choice(DROP_CANCEL), DropPlan::None);
        // A dismissed menu reports 0, which matches no entry.
        assert_eq!(right_drop_choice(0), DropPlan::None);
    }

    #[test]
    fn a_right_drop_reports_the_effect_of_its_plan() {
        let effect = |action| right_drop_effect(DropPlan::Files(action));
        assert_eq!(effect(DropAction::Move), DROPEFFECT_MOVE);
        assert_eq!(effect(DropAction::Copy), DROPEFFECT_COPY);
        assert_eq!(effect(DropAction::KeepInPlace), DROPEFFECT_COPY);
        assert_eq!(right_drop_effect(DropPlan::Shortcuts), DROPEFFECT_LINK);
        assert_eq!(right_drop_effect(DropPlan::None), DROPEFFECT_NONE);
    }

    #[test]
    fn a_right_file_drag_hovers_as_a_move() {
        // The menu decides the action, so the hover does not track Shift.
        assert_eq!(
            drag_effect(DragKind::Files, true, NO_KEYS, ALL),
            DROPEFFECT_MOVE
        );
        assert_eq!(
            drag_effect(DragKind::Files, true, MK_SHIFT, ALL),
            DROPEFFECT_MOVE
        );
        // A source that allows no move still shows its own copy.
        assert_eq!(
            drag_effect(DragKind::Files, true, NO_KEYS, DROPEFFECT_COPY),
            DROPEFFECT_COPY
        );
        // Shell items still ask for the shortcut link, never a move.
        assert_eq!(
            drag_effect(DragKind::ShellItems, true, NO_KEYS, ALL),
            DROPEFFECT_LINK
        );
        // A left drag follows the modifiers as before.
        assert_eq!(
            drag_effect(DragKind::Files, false, NO_KEYS, ALL),
            DROPEFFECT_MOVE
        );
        assert_eq!(
            drag_effect(DragKind::Files, false, MK_SHIFT, ALL),
            DROPEFFECT_COPY
        );
    }
}
