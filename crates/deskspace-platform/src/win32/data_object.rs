//! A minimal `IDataObject` for dragging several files at once.
//!
//! The shell's own data object (`IShellItem::BindToHandler(BHID_DataObject)`)
//! covers a single item, and `SHCreateDataObject` needs every item to share one
//! parent folder, which a space's selection need not. This object serves the
//! two formats a file drop target needs, `CF_HDROP` and "Preferred
//! DropEffect", and stores whatever else `SetData` is handed (the shell's drag
//! image), releasing it when the drag ends.

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    DATA_S_SAMEFORMATETC, DV_E_FORMATETC, E_OUTOFMEMORY, GlobalFree, HGLOBAL,
    OLE_E_ADVISENOTSUPPORTED, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl,
    IEnumFORMATETC, IEnumFORMATETC_Impl, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{CF_HDROP, DROPEFFECT, DROPEFFECT_MOVE, ReleaseStgMedium};
use windows::core::{BOOL, HRESULT, PCWSTR, Result as WinResult};

use super::clipboard::dropfiles_buffer;
use crate::win32::system::wide;

/// A data object carrying `paths` as a `CF_HDROP` file list.
pub(crate) fn for_files(paths: Vec<PathBuf>) -> IDataObject {
    FileDataObject {
        paths,
        preferred: Cell::new(DROPEFFECT_MOVE),
        attachments: RefCell::new(Vec::new()),
    }
    .into()
}

#[windows_core::implement(IDataObject)]
struct FileDataObject {
    paths: Vec<PathBuf>,
    /// The effect the drag source prefers, unless a target sets another.
    preferred: Cell<DROPEFFECT>,
    /// Formats handed to `SetData` with `fRelease` (for example the drag
    /// image), served back to targets and released when this object drops.
    attachments: RefCell<Vec<(FORMATETC, STGMEDIUM)>>,
}

impl Drop for FileDataObject {
    fn drop(&mut self) {
        for (_, mut medium) in self.attachments.borrow_mut().drain(..) {
            // SAFETY: the medium was transferred to us by SetData.
            unsafe { ReleaseStgMedium(&mut medium) };
        }
    }
}

impl IDataObject_Impl for FileDataObject_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> WinResult<STGMEDIUM> {
        // SAFETY: COM passes a valid FORMATETC.
        let format = unsafe { *pformatetcin };
        if format.cfFormat == CF_HDROP.0 {
            let buffer = dropfiles_buffer(&self.paths);
            return Ok(medium(hglobal(&buffer)?));
        }
        if format.cfFormat == preferred_format() {
            return Ok(medium(hglobal(&self.preferred.get().0.to_le_bytes())?));
        }
        // Anything else `SetData` stored (the drag image) is copied back.
        let stored = self
            .attachments
            .borrow()
            .iter()
            .find(|(stored, _)| stored.cfFormat == format.cfFormat)
            .and_then(|(_, medium)| clone_medium(medium));
        stored.ok_or_else(|| DV_E_FORMATETC.into())
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> WinResult<()> {
        Err(DV_E_FORMATETC.into())
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        // SAFETY: COM passes a valid FORMATETC.
        let format = unsafe { *pformatetc };
        let known = format.cfFormat == CF_HDROP.0
            || format.cfFormat == preferred_format()
            || self
                .attachments
                .borrow()
                .iter()
                .any(|(stored, _)| stored.cfFormat == format.cfFormat);
        if known { S_OK } else { DV_E_FORMATETC }
    }

    fn GetCanonicalFormatEtc(
        &self,
        pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        // SAFETY: COM passes valid pointers.
        unsafe {
            *pformatetcout = *pformatectin;
            (*pformatetcout).ptd = std::ptr::null_mut();
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        pformatetc: *const FORMATETC,
        pmedium: *const STGMEDIUM,
        frelease: BOOL,
    ) -> WinResult<()> {
        // SAFETY: COM passes valid pointers; `pmedium` is copied out, not
        // dropped, so a caller that keeps ownership is unaffected.
        let format = unsafe { *pformatetc };
        let medium = unsafe { std::ptr::read(pmedium) };
        if format.cfFormat == preferred_format() {
            if let Some(effect) = effect_from(&medium) {
                self.preferred.set(effect);
            }
        }
        if frelease.as_bool() {
            self.attachments.borrow_mut().push((format, medium));
        }
        Ok(())
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> WinResult<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(OLE_E_ADVISENOTSUPPORTED.into());
        }
        let formats = vec![format_etc(CF_HDROP.0), format_etc(preferred_format())];
        Ok(FormatEnumerator {
            formats,
            index: Cell::new(0),
        }
        .into())
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: windows::core::Ref<IAdviseSink>,
    ) -> WinResult<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn DUnadvise(&self, _dwconnection: u32) -> WinResult<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn EnumDAdvise(&self) -> WinResult<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

/// The registered "Preferred DropEffect" clipboard format.
fn preferred_format() -> u16 {
    let name = wide(super::drop_target::PREFERRED_DROP_EFFECT);
    // SAFETY: `name` is NUL-terminated and outlives the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) as u16 }
}

fn format_etc(cf_format: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: cf_format,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// Allocates `bytes` as a movable global block owned by the caller.
fn hglobal(bytes: &[u8]) -> WinResult<HGLOBAL> {
    // SAFETY: allocating a movable block, copying `bytes` in, then handing it
    // out; on failure the block is freed here.
    unsafe {
        let block = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))?;
        let pointer = GlobalLock(block);
        if pointer.is_null() {
            let _ = GlobalFree(Some(block));
            return Err(E_OUTOFMEMORY.into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(block);
        Ok(block)
    }
}

/// Wraps a global block as a `STGMEDIUM` the caller owns.
fn medium(block: HGLOBAL) -> STGMEDIUM {
    STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: block },
        pUnkForRelease: ManuallyDrop::new(None),
    }
}

/// A copy of `medium` when it is a global block; other medium types are not
/// served back. Handles formats stored by `SetData`, such as the drag image.
fn clone_medium(stored: &STGMEDIUM) -> Option<STGMEDIUM> {
    // SAFETY: reading the union only for an HGLOBAL medium.
    let source = unsafe { stored.u.hGlobal };
    if stored.tymed != TYMED_HGLOBAL.0 as u32 || source.0.is_null() {
        return None;
    }
    // SAFETY: copying the source global into a fresh movable block.
    unsafe {
        let size = GlobalSize(source);
        if size == 0 {
            return None;
        }
        let block = GlobalAlloc(GMEM_MOVEABLE, size).ok()?;
        let destination = GlobalLock(block);
        let origin = GlobalLock(source);
        if destination.is_null() || origin.is_null() {
            let _ = GlobalUnlock(block);
            if !origin.is_null() {
                let _ = GlobalUnlock(source);
            }
            let _ = GlobalFree(Some(block));
            return None;
        }
        std::ptr::copy_nonoverlapping(origin.cast::<u8>(), destination.cast::<u8>(), size);
        let _ = GlobalUnlock(block);
        let _ = GlobalUnlock(source);
        Some(medium(block))
    }
}

fn effect_from(medium: &STGMEDIUM) -> Option<DROPEFFECT> {
    // SAFETY: reading a union field of a medium whose tymed says HGLOBAL.
    let block = unsafe { medium.u.hGlobal };
    if medium.tymed != TYMED_HGLOBAL.0 as u32 || block.0.is_null() {
        return None;
    }
    // SAFETY: the medium holds a single DROPEFFECT (a u32).
    unsafe {
        let pointer = GlobalLock(block);
        if pointer.is_null() {
            return None;
        }
        let effect = DROPEFFECT(std::ptr::read_unaligned(pointer.cast::<u32>()));
        let _ = GlobalUnlock(block);
        Some(effect)
    }
}

#[windows_core::implement(IEnumFORMATETC)]
struct FormatEnumerator {
    formats: Vec<FORMATETC>,
    index: Cell<usize>,
}

impl IEnumFORMATETC_Impl for FormatEnumerator_Impl {
    fn Next(&self, celt: u32, rgelt: *mut FORMATETC, pceltfetched: *mut u32) -> HRESULT {
        let mut fetched = 0u32;
        let mut index = self.index.get();
        while fetched < celt && index < self.formats.len() {
            // SAFETY: the caller provides room for `celt` entries.
            unsafe { rgelt.add(fetched as usize).write(self.formats[index]) };
            fetched += 1;
            index += 1;
        }
        self.index.set(index);
        if !pceltfetched.is_null() {
            // SAFETY: the caller provided the out-parameter.
            unsafe { *pceltfetched = fetched };
        }
        if fetched == celt { S_OK } else { S_FALSE }
    }

    fn Skip(&self, celt: u32) -> WinResult<()> {
        self.index
            .set((self.index.get() + celt as usize).min(self.formats.len()));
        Ok(())
    }

    fn Reset(&self) -> WinResult<()> {
        self.index.set(0);
        Ok(())
    }

    fn Clone(&self) -> WinResult<IEnumFORMATETC> {
        Ok(FormatEnumerator {
            formats: self.formats.clone(),
            index: Cell::new(self.index.get()),
        }
        .into())
    }
}
