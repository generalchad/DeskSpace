//! Decodes shell bitmaps and WIC sources into owned Icon pixels.

use super::graphics::{Graphics, graphics};
use super::*;

/// Copies a 32-bit `HBITMAP` into an owned premultiplied-BGRA `Icon`. The
/// image factory returns icons the shell renders at any size, which `HICON`
/// cannot carry. The caller keeps ownership of `hbitmap`. Needs COM (for WIC).
pub(crate) fn decode_bitmap(hbitmap: HBITMAP) -> Option<Icon> {
    let graphics = graphics().ok()?;
    // SAFETY: `hbitmap` is a live bitmap owned by the caller for this call.
    let source = unsafe {
        graphics
            .wic
            .CreateBitmapFromHBITMAP(hbitmap, HPALETTE::default(), WICBitmapUseAlpha)
            .ok()?
    };
    to_icon(&graphics, &source)
}

/// Converts any WIC source to top-down premultiplied BGRA pixels.
pub(super) fn to_icon(graphics: &Graphics, source: &IWICBitmap) -> Option<Icon> {
    // SAFETY: the pixel buffer is sized from the converter's reported
    // dimensions; `source` is a live WIC bitmap.
    unsafe {
        let converter = graphics.wic.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                source,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .ok()?;
        let (mut width, mut height) = (0u32, 0u32);
        converter.GetSize(&mut width, &mut height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4].into_boxed_slice();
        converter
            .CopyPixels(std::ptr::null(), width * 4, &mut pixels)
            .ok()?;
        Some(Icon::new(width, height, pixels))
    }
}
