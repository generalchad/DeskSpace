//! Rasterizes an icon-set glyph into an `Icon` with Direct2D's SVG support
//! (ROADMAP D16). Windows 10 before 1703 has no `ID2D1DeviceContext5`, so a
//! glyph fails there and callers show no icon.
//!
//! The device context and its two bitmaps are made once per thread and kept:
//! making a Direct3D device for each glyph took about 55 ms, which froze the
//! icon picker for seconds on every page it drew.

use std::cell::RefCell;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_MAP_OPTIONS_READ, ID2D1Bitmap1, ID2D1DeviceContext5, ID2D1Image,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{Error, Interface};

use super::geometry::{PIXEL_DPI, PREMULTIPLIED_BGRA, color_f};
use super::{create_device, is_device_lost};
use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::glyph::svg_document;
use deskspace_core::Glyph;
use deskspace_core::theme::Color;

thread_local! {
    // Process state: the glyph rasterizer's device context, kept so a glyph
    // costs a draw rather than a new device. `rasterize` fills it, a lost
    // device empties it, and `release` drops it before COM goes
    // (`release_graphics`).
    static RASTERIZER: RefCell<Option<Rasterizer>> = const { RefCell::new(None) };
}

/// A device context that draws SVG documents, and the bitmaps for the last
/// size it drew.
struct Rasterizer {
    context: ID2D1DeviceContext5,
    bitmaps: Option<SizedBitmaps>,
}

/// A target to draw into and a CPU-readable copy, both `side` pixels square.
struct SizedBitmaps {
    side: u32,
    target: ID2D1Bitmap1,
    readback: ID2D1Bitmap1,
}

/// `glyph` drawn `px` pixels square in `color`, premultiplied BGRA.
///
/// The pixels come from a Direct2D device context's SVG document drawn into a
/// target bitmap and copied to a CPU-readable one:
/// https://learn.microsoft.com/en-us/windows/win32/api/d2d1_3/nf-d2d1_3-id2d1devicecontext5-createsvgdocument
/// https://learn.microsoft.com/en-us/windows/win32/api/d2d1_3/nf-d2d1_3-id2d1devicecontext5-drawsvgdocument
pub(crate) fn rasterize(glyph: &Glyph, px: i32, color: Color) -> Result<Icon, PlatformError> {
    let side = px.clamp(1, 1024) as u32;
    let svg = svg_document(glyph, color);
    match draw(&svg, side) {
        // The kept context went with its device; the next one is new.
        Err(PlatformError::Win32(error)) if is_device_lost(error.code()) => {
            release();
            draw(&svg, side)
        }
        drawn => drawn,
    }
}

/// Drops this thread's rasterizer. It holds COM objects, so it must go before
/// COM does.
pub(super) fn release() {
    RASTERIZER.with(|cell| cell.borrow_mut().take());
}

/// `svg` drawn `side` pixels square with this thread's rasterizer, made on
/// first use.
fn draw(svg: &str, side: u32) -> Result<Icon, PlatformError> {
    RASTERIZER.with(|cell| {
        // Direct2D sends no window messages, so nothing below re-enters this
        // borrow.
        let mut kept = cell.borrow_mut();
        let rasterizer = match kept.as_mut() {
            Some(rasterizer) => rasterizer,
            None => kept.insert(Rasterizer::new()?),
        };
        rasterizer.draw(svg, side)
    })
}

impl Rasterizer {
    fn new() -> Result<Self, PlatformError> {
        let device = create_device()?;
        // SAFETY: `device` is live; the context it makes is owned here.
        let context = unsafe {
            // `CreateDeviceContext` hands back the oldest interface; the SVG
            // methods need version 5, which an older Direct2D refuses.
            let context: ID2D1DeviceContext5 = device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
                .cast()?;
            context.SetDpi(PIXEL_DPI, PIXEL_DPI);
            context
        };
        Ok(Self {
            context,
            bitmaps: None,
        })
    }

    /// The bitmaps for `side`, made again only when the size changed.
    fn bitmaps(&mut self, side: u32) -> Result<&SizedBitmaps, PlatformError> {
        if self.bitmaps.as_ref().is_none_or(|kept| kept.side != side) {
            self.bitmaps = None;
            self.bitmaps = Some(SizedBitmaps::new(&self.context, side)?);
        }
        Ok(self.bitmaps.as_ref().expect("made above"))
    }

    fn draw(&mut self, svg: &str, side: u32) -> Result<Icon, PlatformError> {
        let context = self.context.clone();
        let bitmaps = self.bitmaps(side)?;
        // SAFETY: the context, bitmaps, and stream below are live for every
        // call that uses them, and the mapped rows are read only while the
        // bitmap is mapped.
        unsafe {
            let stream = SHCreateMemStream(Some(svg.as_bytes()))
                .ok_or_else(|| PlatformError::Win32(Error::from_hresult(E_FAIL)))?;
            let document = context.CreateSvgDocument(
                &stream,
                D2D_SIZE_F {
                    width: side as f32,
                    height: side as f32,
                },
            )?;

            context.SetTarget(Some(&bitmaps.target.cast::<ID2D1Image>()?));
            context.BeginDraw();
            context.Clear(Some(&color_f(Color::TRANSPARENT)));
            context.DrawSvgDocument(&document);
            let drawn = context.EndDraw(None, None);
            context.SetTarget(None::<&ID2D1Image>);
            drawn?;

            // The source cannot stay a target while it is copied from, and
            // the destination must be mapped after the copy.
            bitmaps
                .readback
                .CopyFromBitmap(None, &bitmaps.target, None)?;
            let map = bitmaps.readback.Map(D2D1_MAP_OPTIONS_READ)?;
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
            bitmaps.readback.Unmap()?;
            Ok(Icon::new(side, side, pixels))
        }
    }
}

impl SizedBitmaps {
    fn new(context: &ID2D1DeviceContext5, side: u32) -> Result<Self, PlatformError> {
        let size = D2D_SIZE_U {
            width: side,
            height: side,
        };
        let properties = |options| D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: PREMULTIPLIED_BGRA,
            dpiX: PIXEL_DPI,
            dpiY: PIXEL_DPI,
            bitmapOptions: options,
            ..Default::default()
        };
        // SAFETY: `context` is live; the bitmaps it makes are owned here.
        unsafe {
            Ok(Self {
                side,
                target: context.CreateBitmap(
                    size,
                    None,
                    0,
                    &properties(D2D1_BITMAP_OPTIONS_TARGET),
                )?,
                readback: context.CreateBitmap(
                    size,
                    None,
                    0,
                    &properties(D2D1_BITMAP_OPTIONS(
                        D2D1_BITMAP_OPTIONS_CPU_READ.0 | D2D1_BITMAP_OPTIONS_CANNOT_DRAW.0,
                    )),
                )?,
            })
        }
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

    /// The kept context's COM pointer, if this thread has one.
    fn kept_context() -> Option<*mut core::ffi::c_void> {
        RASTERIZER.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(|rasterizer| rasterizer.context.as_raw())
        })
    }

    #[test]
    fn one_context_draws_every_glyph_on_a_thread() {
        // SAFETY: as above.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        rasterize(&square(), 20, Color::rgb(255, 0, 0)).expect("a glyph");
        let first = kept_context().expect("a kept context");
        rasterize(&square(), 20, Color::rgb(0, 0, 255)).expect("a glyph");
        assert_eq!(kept_context(), Some(first));
        release();
        assert_eq!(kept_context(), None);
    }

    #[test]
    fn a_new_size_draws_whole_and_the_old_size_still_does() {
        // SAFETY: as above.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        for side in [20, 40, 20] {
            let icon = rasterize(&square(), side, Color::rgb(255, 0, 0)).expect("a glyph");
            let side = side as u32;
            assert_eq!((icon.width(), icon.height()), (side, side));
            assert_eq!(pixel(&icon, side / 2, side / 2), [0, 0, 255, 255]);
            assert_eq!(pixel(&icon, 0, 0)[3], 0, "the last draw left nothing");
        }
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
