//! The DeskSpace icon, drawn in code so the tray and settings window have a
//! real icon without a resource compiler: an accent-blue rounded square holding
//! a 2x2 grid of space tiles.
// TODO(deskspace): replace with an .ico resource once release packaging (M8)
// adds a resource step.

use std::path::Path;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

use crate::error::PlatformError;

/// Windows 11 default accent blue.
const ACCENT: (u8, u8, u8) = (0x00, 0x67, 0xC0);

/// Straight-alpha ARGB pixels, row-major, top row first.
fn pixels(size: i32) -> Vec<u32> {
    let s = size as f32;
    let outer_radius = s * 0.22;
    let margin = s * 0.19;
    let gap = s * 0.08;
    let tile = (s - 2.0 * margin - gap) / 2.0;
    let tile_radius = s * 0.06;

    let mut out = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let body = coverage(rounded_rect_distance(px, py, 0.0, 0.0, s, s, outer_radius));
            let mut tiles = 0.0f32;
            for row in 0..2 {
                for column in 0..2 {
                    let left = margin + column as f32 * (tile + gap);
                    let top = margin + row as f32 * (tile + gap);
                    let distance =
                        rounded_rect_distance(px, py, left, top, tile, tile, tile_radius);
                    tiles = tiles.max(coverage(distance));
                }
            }
            let mix = |accent: u8| (accent as f32 * (1.0 - tiles) + 255.0 * tiles).round() as u32;
            let alpha = (body * 255.0).round() as u32;
            out.push((alpha << 24) | (mix(ACCENT.0) << 16) | (mix(ACCENT.1) << 8) | mix(ACCENT.2));
        }
    }
    out
}

/// Signed distance from a point to a rounded rectangle (negative inside).
fn rounded_rect_distance(
    px: f32,
    py: f32,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> f32 {
    let half_w = width / 2.0;
    let half_h = height / 2.0;
    let dx = (px - (left + half_w)).abs() - (half_w - radius);
    let dy = (py - (top + half_h)).abs() - (half_h - radius);
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    outside + dx.max(dy).min(0.0) - radius
}

/// One pixel of anti-aliasing across the edge.
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Creates the icon at `size` pixels. The caller owns the HICON.
pub(crate) fn create(size: i32) -> Result<HICON, PlatformError> {
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        // Negative height makes a top-down DIB, matching `pixels`' row order.
        biHeight: -size,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    let source = pixels(size);
    // SAFETY: the DIB section is size*size 32-bit pixels, exactly
    // `source.len()` u32s; both bitmaps are deleted after CreateIconIndirect
    // copies them.
    unsafe {
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(PlatformError::InvalidFormat("icon bitmap"));
        }
        std::ptr::copy_nonoverlapping(source.as_ptr(), bits as *mut u32, source.len());
        // With a 32-bit color bitmap the alpha channel defines transparency;
        // the monochrome mask must exist but is ignored.
        // https://learn.microsoft.com/en-us/windows/win32/menurc/using-icons
        let mask = CreateBitmap(size, size, 1, 1, None);
        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&icon_info);
        let _ = DeleteObject(HGDIOBJ(mask.0));
        let _ = DeleteObject(HGDIOBJ(color.0));
        Ok(icon?)
    }
}

/// The icon sizes written to the `.ico` file, covering Explorer's small,
/// medium, and large menu/thumbnail slots.
const FILE_SIZES: [i32; 3] = [16, 32, 48];

/// Writes the app icon to `path` as a multi-size `.ico`, so the shell can show
/// it (for example beside the desktop "Show spaces" verb) without a resource
/// compiler. Overwrites an existing file.
pub fn write_icon(path: &Path) -> Result<(), PlatformError> {
    let images: Vec<Vec<u8>> = FILE_SIZES.iter().map(|&size| icon_image(size)).collect();
    let mut bytes = Vec::new();
    // ICONDIR: reserved 0, type 1 (icon), image count.
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(FILE_SIZES.len() as u16).to_le_bytes());
    // ICONDIRENTRY per image, then the image data after all entries.
    let mut offset = 6 + 16 * FILE_SIZES.len();
    for (size, image) in FILE_SIZES.iter().zip(&images) {
        let dimension = if *size >= 256 { 0u8 } else { *size as u8 };
        bytes.push(dimension);
        bytes.push(dimension);
        bytes.push(0); // color count
        bytes.push(0); // reserved
        bytes.extend_from_slice(&1u16.to_le_bytes()); // planes
        bytes.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        bytes.extend_from_slice(&(image.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image.len();
    }
    for image in &images {
        bytes.extend_from_slice(image);
    }
    std::fs::write(path, &bytes)?;
    Ok(())
}

/// One ICO image: a `BITMAPINFOHEADER`, bottom-up BGRA pixels, and a zeroed
/// AND mask (the alpha channel carries transparency).
fn icon_image(size: i32) -> Vec<u8> {
    let pixels = pixels(size);
    let xor_len = (size * size) as usize * 4;
    // Each AND-mask row is 1 bpp, padded to a 4-byte boundary.
    let and_len = (((size + 31) / 32) * 4) as usize * size as usize;
    let mut out = Vec::with_capacity(40 + xor_len + and_len);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&size.to_le_bytes()); // biWidth
    // ICO bitmaps hold the color and AND masks, hence twice the height.
    out.extend_from_slice(&(size * 2).to_le_bytes()); // biHeight
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    out.extend_from_slice(&((xor_len + and_len) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // biXPelsPerMeter/Y/ClrUsed/ClrImportant
    for y in (0..size).rev() {
        for x in 0..size {
            let pixel = pixels[(y * size + x) as usize];
            out.push((pixel & 0xFF) as u8); // blue
            out.push(((pixel >> 8) & 0xFF) as u8); // green
            out.push(((pixel >> 16) & 0xFF) as u8); // red
            out.push(((pixel >> 24) & 0xFF) as u8); // alpha (straight)
        }
    }
    out.resize(out.len() + and_len, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_transparent_and_center_is_opaque() {
        let size = 32;
        let image = pixels(size);
        assert_eq!(image.len(), (size * size) as usize);
        assert_eq!(image[0] >> 24, 0);
        let center = image[(size / 2 * size + size / 2) as usize];
        assert_eq!(center >> 24, 255);
    }

    #[test]
    fn ico_file_has_a_valid_directory_and_image_data() {
        let path = std::env::temp_dir().join("deskspace-icon-writer-test.ico");
        write_icon(&path).expect("write icon");
        let bytes = std::fs::read(&path).expect("read icon");
        let _ = std::fs::remove_file(&path);
        // ICONDIR: reserved 0, type 1, three images.
        assert_eq!(&bytes[0..4], [0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 3);
        // First entry is 16x16, 32 bpp, and its bytes land inside the file.
        let entry = &bytes[6..22];
        assert_eq!((entry[0], entry[1]), (16, 16));
        assert_eq!(u16::from_le_bytes([entry[4], entry[5]]), 1); // planes
        assert_eq!(u16::from_le_bytes([entry[6], entry[7]]), 32); // bits per pixel
        let bytes_in_res = u32::from_le_bytes(entry[8..12].try_into().unwrap());
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        assert!(offset + bytes_in_res as usize <= bytes.len());
        // The image starts with a 40-byte BITMAPINFOHEADER whose height is
        // twice the width (color plus AND mask).
        assert_eq!(
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            40
        );
        assert_eq!(
            i32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
            32
        );
    }

    #[test]
    fn tiles_are_lighter_than_the_body() {
        let size = 32;
        let image = pixels(size);
        let blue = |pixel: u32| pixel & 0xFF;
        let red = |pixel: u32| (pixel >> 16) & 0xFF;
        // Inside the top-left tile.
        let tile = image[(9 * size + 9) as usize];
        // On the body, between the tiles and the outer edge.
        let body = image[(16 * size + 3) as usize];
        assert!(red(tile) > red(body));
        assert_eq!(blue(body), ACCENT.2 as u32);
    }
}
