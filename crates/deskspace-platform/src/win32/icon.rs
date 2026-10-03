//! The DeskSpace icon, drawn in code so the tray and settings window have a
//! real icon without a resource compiler: an accent-blue rounded square holding
//! a 2x2 grid of space tiles.
// TODO(deskspace): replace with an .ico resource once release packaging (M8)
// adds a resource step.

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
