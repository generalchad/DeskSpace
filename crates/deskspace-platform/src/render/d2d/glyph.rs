//! Rasterizes an icon-set glyph into an `Icon` with Direct2D's SVG support
//! (ROADMAP D16). Windows 10 before 1703 has no `ID2D1DeviceContext5`, so a
//! glyph fails there and callers show no icon.

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_MAP_OPTIONS_READ, ID2D1DeviceContext5, ID2D1Image,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{Error, Interface};

use super::create_device;
use super::geometry::{PIXEL_DPI, PREMULTIPLIED_BGRA, color_f};
use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::glyph::svg_document;
use deskspace_core::Glyph;
use deskspace_core::theme::Color;

/// `glyph` drawn `px` pixels square in `color`, premultiplied BGRA.
///
/// The pixels come from a Direct2D device context's SVG document drawn into a
/// target bitmap and copied to a CPU-readable one:
/// https://learn.microsoft.com/en-us/windows/win32/api/d2d1_3/nf-d2d1_3-id2d1devicecontext5-createsvgdocument
/// https://learn.microsoft.com/en-us/windows/win32/api/d2d1_3/nf-d2d1_3-id2d1devicecontext5-drawsvgdocument
pub(crate) fn rasterize(glyph: &Glyph, px: i32, color: Color) -> Result<Icon, PlatformError> {
    let side = px.clamp(1, 1024) as u32;
    let device = create_device()?;
    let svg = svg_document(glyph, color);
    // SAFETY: the device, context, bitmaps, and stream below are live for
    // every call that uses them, and the mapped rows are read only while the
    // bitmap is mapped.
    unsafe {
        // `CreateDeviceContext` hands back the oldest interface; the SVG
        // methods need version 5, which an older Direct2D refuses.
        let context: ID2D1DeviceContext5 = device
            .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
            .cast()?;
        context.SetDpi(PIXEL_DPI, PIXEL_DPI);
        let size = D2D_SIZE_U {
            width: side,
            height: side,
        };
        let target = context.CreateBitmap(
            size,
            None,
            0,
            &D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: PREMULTIPLIED_BGRA,
                dpiX: PIXEL_DPI,
                dpiY: PIXEL_DPI,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            },
        )?;
        let readback = context.CreateBitmap(
            size,
            None,
            0,
            &D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: PREMULTIPLIED_BGRA,
                dpiX: PIXEL_DPI,
                dpiY: PIXEL_DPI,
                bitmapOptions: D2D1_BITMAP_OPTIONS(
                    D2D1_BITMAP_OPTIONS_CPU_READ.0 | D2D1_BITMAP_OPTIONS_CANNOT_DRAW.0,
                ),
                ..Default::default()
            },
        )?;

        let stream = SHCreateMemStream(Some(svg.as_bytes()))
            .ok_or_else(|| PlatformError::Win32(Error::from_hresult(E_FAIL)))?;
        let document = context.CreateSvgDocument(
            &stream,
            D2D_SIZE_F {
                width: side as f32,
                height: side as f32,
            },
        )?;

        context.SetTarget(Some(&target.cast::<ID2D1Image>()?));
        context.BeginDraw();
        context.Clear(Some(&color_f(Color::TRANSPARENT)));
        context.DrawSvgDocument(&document);
        context.EndDraw(None, None)?;
        context.SetTarget(None::<&ID2D1Image>);

        // The source cannot stay a target while it is copied from, and the
        // destination must be mapped after the copy.
        readback.CopyFromBitmap(None, &target, None)?;
        let map = readback.Map(D2D1_MAP_OPTIONS_READ)?;
        let row_bytes = side as usize * 4;
        let mut pixels = vec![0u8; row_bytes * side as usize].into_boxed_slice();
        for row in 0..side as usize {
            let source = map.bits.add(row * map.pitch as usize);
            std::ptr::copy_nonoverlapping(
                source,
                pixels[row * row_bytes..].as_mut_ptr(),
                row_bytes,
            );
        }
        readback.Unmap()?;
        Ok(Icon::new(side, side, pixels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    fn square() -> Glyph {
        Glyph {
            key: "test:square".to_owned(),
            body: "<path d='M2 2h16v16H2z'/>".to_owned(),
            width: 20,
            height: 20,
        }
    }

    fn pixel(icon: &Icon, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * icon.width() + x) * 4) as usize;
        let pixels = icon.pixels();
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }

    #[test]
    fn it_draws_the_glyph_in_its_color() {
        // SAFETY: the shared Direct2D factory makes a WIC factory, which needs
        // COM on this thread; already initialized is fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let icon = rasterize(&square(), 20, Color::rgb(255, 0, 0)).expect("a glyph");
        assert_eq!((icon.width(), icon.height()), (20, 20));
        assert_eq!(pixel(&icon, 10, 10), [0, 0, 255, 255], "opaque red");
        assert_eq!(pixel(&icon, 0, 0)[3], 0, "outside the square is clear");
        assert_eq!(pixel(&icon, 19, 19)[3], 0);
    }

    #[test]
    fn a_translucent_color_reaches_the_pixels() {
        // SAFETY: as above.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let icon = rasterize(&square(), 20, Color::rgba(255, 0, 0, 128)).expect("a glyph");
        let pixel = pixel(&icon, 10, 10);
        assert!((126..=130).contains(&pixel[3]), "{pixel:?}");
        // Premultiplied: red is alpha times 255, not 255.
        assert!(pixel[2] < 200, "{pixel:?}");
    }
}
