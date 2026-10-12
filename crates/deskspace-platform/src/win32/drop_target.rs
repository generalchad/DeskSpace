//! OLE drop target for space windows.
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

use windows::Win32::Foundation::{GlobalFree, HWND, POINT, POINTL};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM,
    STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE, IDropTarget,
    IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::{MK_SHIFT, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{
    CLSID_DragDropHelper, DROPDESCRIPTION, DROPIMAGE_NOIMAGE, DROPIMAGETYPE, DragQueryFileW, HDROP,
    IDropTargetHelper,
};
use windows::Win32::UI::WindowsAndMessaging::GetParent;
use windows::core::{PCWSTR, Ref, Result as WinResult};

use crate::error::PlatformError;
use crate::input::{Event, SpaceEvent};
use crate::win32::runtime;
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

#[windows_core::implement(IDropTarget)]
struct SpaceDropTarget {
    id: u64,
    window: HWND,
    hosted: bool,
    /// Whether the current drag carries files, remembered from `DragEnter`
    /// because `DragOver` gets no data object.
    accepts: Cell<bool>,
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
        let accepts = data.as_ref().is_some_and(has_files);
        self.accepts.set(accepts);
        let chosen = if accepts {
            desired_effect(keys, allowed)
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = chosen };
        // A shell source (Explorer, the desktop) puts a default action hint
        // like "Move" on the data object; clearing it here leaves the drag
        // showing only the item image, matching a DeskSpace-originated drag.
        if let Some(data) = data.as_ref().filter(|_| accepts) {
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
        let chosen = if self.accepts.get() {
            desired_effect(keys, allowed)
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = chosen };
        // SAFETY: forwarding this call's own arguments.
        with_image_helper(|helper| unsafe { helper.DragOver(&screen(point), chosen) });
        Ok(())
    }

    fn DragLeave(&self) -> WinResult<()> {
        self.accepts.set(false);
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
        let accepts = data.is_some_and(has_files);
        let chosen = if accepts {
            desired_effect(keys, allowed)
        } else {
            DROPEFFECT_NONE
        };
        // SAFETY: as above.
        unsafe { *effect = chosen };
        self.accepts.set(false);
        // The image is removed before the app runs: handling the drop can take
        // a moment (file moves) and must not leave the image frozen on screen.
        // SAFETY: forwarding this call's own arguments.
        with_image_helper(|helper| unsafe { helper.Drop(data, &screen(point), chosen) });
        let (Some(data), true) = (data, chosen != DROPEFFECT_NONE) else {
            return Ok(());
        };
        let Some(paths) = file_paths(data) else {
            return Ok(());
        };
        if paths.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            space = self.id,
            effect = chosen.0,
            ?paths,
            exist = ?paths.iter().map(|path| path.exists()).collect::<Vec<_>>(),
            "files dropped on space"
        );
        let keep_in_place = chosen == DROPEFFECT_COPY;
        let (x, y) = desktop_point(self.window, self.hosted, point);
        runtime::dispatch(Event::Space {
            id: self.id,
            event: SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                keep_in_place,
            },
        });

        // The app performs the move itself (the event above runs before this
        // returns for drags that started in another process). Marking the move
        // as already performed stops the source from moving the files a second
        // time; for an in-process (space-to-space) drag it is a no-op, because
        // DeskSpace's own source never deletes on `Move`.
        if chosen == DROPEFFECT_MOVE {
            let _ = set_drop_effect(data, PERFORMED_DROP_EFFECT, DROPEFFECT_MOVE);
        }
        Ok(())
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
        accepts: Cell::new(false),
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

/// The effect a space accepts for `keys`, limited to `allowed` (the bits the
/// source offered). Shift preserves the original, so it asks for a copy;
/// otherwise the drop moves the file into the space.
fn desired_effect(keys: MODIFIERKEYS_FLAGS, allowed: DROPEFFECT) -> DROPEFFECT {
    let want = if keys.contains(MK_SHIFT) {
        DROPEFFECT_COPY
    } else {
        DROPEFFECT_MOVE
    };
    if allowed.0 & want.0 != 0 {
        want
    } else if allowed.0 & DROPEFFECT_COPY.0 != 0 {
        DROPEFFECT_COPY
    } else if allowed.0 & DROPEFFECT_MOVE.0 != 0 {
        DROPEFFECT_MOVE
    } else {
        DROPEFFECT_NONE
    }
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

fn has_files(data: &IDataObject) -> bool {
    // SAFETY: QueryGetData transfers no ownership.
    unsafe { data.QueryGetData(&drop_files_format()).is_ok() }
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
    use windows::Win32::System::Ole::DROPEFFECT_LINK;

    const ALL: DROPEFFECT = DROPEFFECT(DROPEFFECT_MOVE.0 | DROPEFFECT_COPY.0 | DROPEFFECT_LINK.0);

    #[test]
    fn shift_copies_and_default_moves() {
        assert_eq!(desired_effect(MODIFIERKEYS_FLAGS(0), ALL), DROPEFFECT_MOVE);
        assert_eq!(desired_effect(MK_SHIFT, ALL), DROPEFFECT_COPY);
    }

    #[test]
    fn falls_back_when_the_preferred_effect_is_not_allowed() {
        assert_eq!(
            desired_effect(MODIFIERKEYS_FLAGS(0), DROPEFFECT_COPY),
            DROPEFFECT_COPY
        );
        assert_eq!(desired_effect(MK_SHIFT, DROPEFFECT_MOVE), DROPEFFECT_MOVE);
    }

    #[test]
    fn offers_nothing_when_no_effect_is_allowed() {
        assert_eq!(
            desired_effect(MODIFIERKEYS_FLAGS(0), DROPEFFECT_NONE),
            DROPEFFECT_NONE
        );
    }
}
