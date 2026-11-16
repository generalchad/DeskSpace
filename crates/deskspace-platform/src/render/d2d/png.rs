//! Writing a test's pixels to a PNG, for the looks nobody can check without
//! a screen: the color picker's preview and `render::effect_sheet`'s
//! contact sheet share it. Test-only, so a shipped build carries no encoder.
//!
//! COM must be initialized on the calling thread; the tests that use this
//! initialize it themselves.

use std::path::Path;

use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA,
    IWICBitmapSource, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::win32::system::wide;

/// Writes `pixels` (`width` by `height`, premultiplied BGRA as
/// `0xAARRGGBB`) to `path` as a PNG.
pub(crate) fn write_pixels(
    pixels: &[u32],
    width: u32,
    height: u32,
    path: &Path,
) -> Result<(), PlatformError> {
    // `0xAARRGGBB` is BGRA in little-endian memory, the format below.
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|pixel| pixel.to_le_bytes())
        .collect();
    let factory = factory()?;
    // SAFETY: COM is initialized by the caller; `bytes` holds exactly
    // `width * height` pixels of the format named, so it covers every row.
    let bitmap = unsafe {
        factory.CreateBitmapFromMemory(
            width,
            height,
            &GUID_WICPixelFormat32bppPBGRA,
            width * 4,
            &bytes,
        )?
    };
    write_png(&bitmap, path)
}

/// Writes `source` to `path` as a PNG.
pub(crate) fn write_png(source: &IWICBitmapSource, path: &Path) -> Result<(), PlatformError> {
    let factory = factory()?;
    // SAFETY: COM is initialized on this thread by the caller; every object
    // below is local and outlives its use, and `wide` stays alive until the
    // stream has opened the file.
    unsafe {
        let stream = factory.CreateStream()?;
        let wide = wide(&path.to_string_lossy());
        stream.InitializeFromFilename(PCWSTR(wide.as_ptr()), GENERIC_WRITE.0)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        encoder.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
        let frame = frame.ok_or(PlatformError::InvalidFormat("PNG frame"))?;
        frame.Initialize(None)?;
        frame.WriteSource(source, std::ptr::null())?;
        frame.Commit()?;
        encoder.Commit()?;
    }
    Ok(())
}

fn factory() -> Result<IWICImagingFactory, PlatformError> {
    // SAFETY: plain factory creation; COM is initialized by the caller.
    let factory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    Ok(factory)
}
