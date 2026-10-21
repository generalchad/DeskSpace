//! Decodes shell bitmaps, icons, and WIC sources into owned Icon pixels.

use super::graphics::{Graphics, graphics};
use super::*;
use windows::Win32::UI::WindowsAndMessaging::HICON;

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

/// Copies an `HICON` into an owned premultiplied-BGRA `Icon`. WIC applies the
/// icon's AND mask, so a legacy icon with no alpha channel comes out with its
/// masked pixels transparent, as a 32 bpp icon's alpha does. The caller keeps
/// ownership of `hicon`. Needs COM (for WIC).
/// https://learn.microsoft.com/en-us/windows/win32/api/wincodec/nf-wincodec-iwicimagingfactory-createbitmapfromhicon
pub(crate) fn decode_icon(hicon: HICON) -> Option<Icon> {
    let graphics = graphics().ok()?;
    // SAFETY: `hicon` is a live icon owned by the caller for this call.
    let source = unsafe { graphics.wic.CreateBitmapFromHICON(hicon).ok()? };
    to_icon(&graphics, &source)
}

/// Converts any WIC source to top-down premultiplied BGRA pixels.
pub(super) fn to_icon(graphics: &Graphics, source: &IWICBitmap) -> Option<Icon> {
    let (width, height, pixels) = to_pixels(graphics, source)?;
    Some(Icon::new(width, height, pixels))
}

/// A WIC source's top-down premultiplied-BGRA pixels and size.
pub(super) fn to_pixels(graphics: &Graphics, source: &IWICBitmap) -> Option<(u32, u32, Box<[u8]>)> {
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
        Some((width, height, pixels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, ICONINFO};

    /// A 2x1 icon from 32 bpp BGRA `color` and a 1 bpp AND `mask` row (set
    /// bits are transparent; the row is padded to a WORD).
    fn icon(color: [[u8; 4]; 2], mask: u8) -> HICON {
        // SAFETY: both bitmaps are built from buffers of the size they
        // describe; CreateIconIndirect copies them, so they are deleted here.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let color_bitmap = CreateBitmap(2, 1, 1, 32, Some(color.as_ptr().cast()));
            let mask_bitmap = CreateBitmap(2, 1, 1, 1, Some([mask, 0].as_ptr().cast()));
            let info = ICONINFO {
                fIcon: true.into(),
                hbmMask: mask_bitmap,
                hbmColor: color_bitmap,
                ..Default::default()
            };
            let icon = CreateIconIndirect(&info).expect("icon");
            let _ = DeleteObject(color_bitmap.into());
            let _ = DeleteObject(mask_bitmap.into());
            icon
        }
    }

    fn decoded(hicon: HICON) -> Vec<u8> {
        let pixels = decode_icon(hicon).expect("decode").pixels().to_vec();
        // SAFETY: the icon was created by `icon` and is not used again.
        let _ = unsafe { DestroyIcon(hicon) };
        pixels
    }

    #[test]
    fn a_legacy_icon_takes_its_transparency_from_the_mask() {
        // No alpha anywhere: the left pixel is masked out, the right opaque.
        let pixels = decoded(icon([[0, 0, 0, 0], [10, 20, 30, 0]], 0b1000_0000));
        assert_eq!(pixels, [0, 0, 0, 0, 10, 20, 30, 255]);
    }

    #[test]
    fn an_alpha_icon_comes_out_premultiplied() {
        let pixels = decoded(icon([[200, 100, 50, 255], [200, 100, 50, 51]], 0));
        assert_eq!(pixels, [200, 100, 50, 255, 40, 20, 10, 51]);
    }
}
