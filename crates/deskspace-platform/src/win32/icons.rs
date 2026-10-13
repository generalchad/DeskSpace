//! The shell icon cache: base icons from the image factory, the classic
//! fallback, and the overlays composited as Explorer draws them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SHFILEINFOW,
    SHGFI_ADDOVERLAYS, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_LINKOVERLAY, SHGFI_OVERLAYINDEX,
    SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO, SHIL_LARGE, SHIL_SMALL,
    SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::{Interface, PCWSTR};

use crate::render::Icon;
use crate::render::d2d::decode_bitmap;
use crate::win32::system::wide;

/// Icons decoded per path, pixel size, and overlay size, bounded by a
/// least-recently-used limit. Evicting is always safe: scenes hold their own
/// reference-counted `Icon`s. The size is part of the key, so a per-monitor DPI
/// or icon-size change does not hand out stale pixels, and the same holds for a
/// changed overlay size.
pub struct IconCache {
    capacity: usize,
    clock: u64,
    entries: HashMap<(PathBuf, i32, u8), CachedIcon>,
}

struct CachedIcon {
    icon: Icon,
    last_used: u64,
}

impl IconCache {
    /// Enough for every visible item on a crowded desktop.
    pub const DEFAULT_CAPACITY: usize = 1024;

    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            clock: 0,
            entries: HashMap::new(),
        }
    }

    /// The icon for `path` rendered at `size` pixels square, with the shell's
    /// overlay drawn at `overlay_size` percent of that.
    pub fn icon_for(&mut self, path: &Path, size: i32, overlay_size: u8) -> Option<Icon> {
        self.clock += 1;
        let key = (path.to_path_buf(), size, overlay_size);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.clock;
            return Some(entry.icon.clone());
        }
        let icon = load_icon(path, size, overlay_size)?;
        self.insert(path.to_path_buf(), size, overlay_size, icon.clone());
        Some(icon)
    }

    /// Forgets every size of the icon for `path`, so the next lookup reloads
    /// it (for example after the file changed).
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.retain(|(entry, _, _), _| entry != path);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert(&mut self, path: PathBuf, size: i32, overlay_size: u8, icon: Icon) {
        if self.entries.len() >= self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            (path, size, overlay_size),
            CachedIcon {
                icon,
                last_used: self.clock,
            },
        );
    }
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders `path` as an icon `size` pixels square, with the shell's overlay
/// (shortcut arrow, sync badge) drawn over it as Explorer does. The base image
/// comes from the shell's image factory, which draws crisp icons at any size
/// (`HICON` only goes up to the small/large system sizes); paths the factory
/// cannot render fall back to the classic shell icon.
fn load_icon(path: &Path, size: i32, overlay_size: u8) -> Option<Icon> {
    let size = size.clamp(1, 1024) as u32;
    let base = load_base_icon(path, size).or_else(|| load_classic_icon(path))?;
    let base = resample(&base, size);
    match overlay_index(path).and_then(|overlay| load_overlay_image(overlay, size, overlay_size)) {
        Some(overlay) => Some(composite_overlay(&base, &overlay)),
        None => Some(base),
    }
}

/// Whether `path` is a shortcut, which Windows marks with the link arrow.
fn is_shortcut(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("url")
        })
}

/// The classic 32 px `SHGetFileInfoW` icon, without overlays, for paths the
/// image factory rejects (some virtual and shortcut targets).
fn load_classic_icon(path: &Path) -> Option<Icon> {
    let wide = wide(&path.to_string_lossy());
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON;
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure. The
    // returned HICON is ours and destroyed after its pixels are copied out.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = decode_icon_masked(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
    }
}

fn load_base_icon(path: &Path, size: u32) -> Option<Icon> {
    let wide = wide(&path.to_string_lossy());
    // SAFETY: `wide` is NUL-terminated and outlives the parse; the image
    // factory and its bitmap are released below, after the pixels are copied.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let bitmap: HBITMAP = factory
            .GetImage(
                SIZE {
                    cx: size as i32,
                    cy: size as i32,
                },
                SIIGBF_ICONONLY,
            )
            .ok()?;
        let icon = decode_bitmap(bitmap);
        let _ = DeleteObject(bitmap.into());
        icon
    }
}

/// Converts an `HICON` to premultiplied BGRA from its color and mask bitmaps:
/// a 32 bpp icon carries its alpha in the color bitmap, a legacy icon carries
/// transparency in the 1 bpp AND mask.
fn decode_icon_masked(hicon: HICON) -> Option<Icon> {
    // SAFETY: `hicon` is live; `GetIconInfo` creates two bitmaps that are
    // deleted before returning, and the GDI calls only fill our buffers.
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(hicon, &mut info).ok()?;
        let mut bitmap = BITMAP::default();
        GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut bitmap as *mut BITMAP).cast()),
        );
        let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
        if width <= 0 || height <= 0 {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
            return None;
        }
        let dc = GetDC(None);
        let header = |bits: u16| BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: bits,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut color = vec![0u8; (width * height * 4) as usize];
        let mut color_info = header(32);
        let color_ok = GetDIBits(
            dc,
            info.hbmColor,
            0,
            height as u32,
            Some(color.as_mut_ptr().cast()),
            &mut color_info,
            DIB_RGB_COLORS,
        ) != 0;
        let stride = (((width + 31) / 32) * 4) as usize;
        let mut mask = vec![0u8; stride * height as usize];
        let mut mask_info = header(1);
        let mask_ok = GetDIBits(
            dc,
            info.hbmMask,
            0,
            height as u32,
            Some(mask.as_mut_ptr().cast()),
            &mut mask_info,
            DIB_RGB_COLORS,
        ) != 0;
        ReleaseDC(None, dc);
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        if !color_ok {
            return None;
        }
        let has_alpha = color.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
        let mut pixels = vec![0u8; color.len()];
        for y in 0..height as usize {
            for x in 0..width as usize {
                let index = (y * width as usize + x) * 4;
                let transparent = if mask_ok {
                    let byte = mask[y * stride + x / 8];
                    (byte >> (7 - (x % 8))) & 1 == 1
                } else {
                    false
                };
                let alpha = if has_alpha {
                    if transparent {
                        0
                    } else {
                        color[index + 3] as u16
                    }
                } else if transparent {
                    0
                } else {
                    255
                };
                for channel in 0..3 {
                    pixels[index + channel] =
                        ((color[index + channel] as u32 * alpha as u32 + 127) / 255) as u8;
                }
                pixels[index + 3] = alpha as u8;
            }
        }
        Some(Icon::new(
            width as u32,
            height as u32,
            pixels.into_boxed_slice(),
        ))
    }
}

/// The 1-based overlay index Explorer would draw on `path`, or `None` when it
/// draws none. `ADDOVERLAYS` asks the registered overlay handlers (sync badges
/// and the like); `LINKOVERLAY` adds the link arrow, and is passed only for
/// shortcuts because it arrows any file it is given.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
fn overlay_index(path: &Path) -> Option<i32> {
    let wide = wide(&path.to_string_lossy());
    let mut flags = SHGFI_ICON | SHGFI_OVERLAYINDEX | SHGFI_ADDOVERLAYS;
    if is_shortcut(path) {
        flags |= SHGFI_LINKOVERLAY;
    }
    let mut info = SHFILEINFOW::default();
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure of
    // the size passed. OVERLAYINDEX requires ICON, so an HICON comes back too;
    // it is ours and destroyed at once.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if !info.hIcon.is_invalid() {
            let _ = DestroyIcon(info.hIcon);
        }
        if result == 0 {
            return None;
        }
    }
    // OVERLAYINDEX returns the overlay index in the upper eight bits of iIcon.
    let overlay = (info.iIcon as u32 >> 24) as i32;
    (overlay != 0).then_some(overlay)
}

/// The overlay image `overlay` from the system image list nearest `size`, sized
/// to `overlay_size` percent of `size` and anchored where Explorer draws it: the
/// glyph sits in the bottom-left corner of the full icon, so the scaled image is
/// placed flush to that corner of a `size`-square transparent canvas. The
/// returned image is full `size` square, ready to composite over the base.
/// https://learn.microsoft.com/en-us/windows/win32/api/commoncontrols/nf-commoncontrols-iimagelist-getoverlayimage
fn load_overlay_image(overlay: i32, size: u32, overlay_size: u8) -> Option<Icon> {
    let list = system_image_list(size)?;
    // SAFETY: `list` is a live image list; the HICON `GetIcon` creates is ours
    // and destroyed after its pixels are copied out.
    let icon = unsafe {
        let index = list.GetOverlayImage(overlay).ok()?;
        let hicon = list.GetIcon(index, ILD_TRANSPARENT.0).ok()?;
        let icon = decode_icon_masked(hicon);
        let _ = DestroyIcon(hicon);
        icon
    }?;
    Some(scale_overlay_to_corner(&icon, size, overlay_size))
}

/// `icon` (a full-icon-sized overlay image) scaled to `overlay_size` percent of
/// `size` and placed in the bottom-left corner of a `size`-square transparent
/// canvas, where Explorer draws overlays.
fn scale_overlay_to_corner(icon: &Icon, size: u32, overlay_size: u8) -> Icon {
    let scaled = resample(icon, (size * overlay_size as u32 / 100).max(1));
    let (width, height) = (scaled.width(), scaled.height());
    let mut canvas = vec![0u8; (size * size * 4) as usize];
    let top = size.saturating_sub(height);
    for y in 0..height.min(size) {
        for x in 0..width.min(size) {
            let from = ((y * width + x) * 4) as usize;
            let to = (((top + y) * size + x) * 4) as usize;
            canvas[to..to + 4].copy_from_slice(&scaled.pixels()[from..from + 4]);
        }
    }
    Icon::new(size, size, canvas.into_boxed_slice())
}

/// The smallest system image list whose icons are at least `size` pixels, or
/// the largest one when none is, so the overlay is only ever scaled down.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetimagelist
fn system_image_list(size: u32) -> Option<IImageList> {
    let mut largest: Option<(i32, IImageList)> = None;
    for kind in [SHIL_SMALL, SHIL_LARGE, SHIL_EXTRALARGE, SHIL_JUMBO] {
        // SAFETY: plain out-parameter calls on a live, shell-owned image list.
        let Some((width, list)) = (unsafe {
            SHGetImageList::<IImageList>(kind as i32)
                .ok()
                .and_then(|list| {
                    let (mut width, mut height) = (0, 0);
                    list.GetIconSize(&mut width, &mut height).ok()?;
                    Some((width, list))
                })
        }) else {
            continue;
        };
        if width >= size as i32 {
            return Some(list);
        }
        if largest.as_ref().is_none_or(|(best, _)| width > *best) {
            largest = Some((width, list));
        }
    }
    largest.map(|(_, list)| list)
}

/// `icon` scaled to `size` square by area averaging, which keeps a downscaled
/// glyph's thin outline instead of dropping rows as nearest-neighbor does.
/// Returns `icon` itself when it is already that size.
fn resample(icon: &Icon, size: u32) -> Icon {
    let (width, height) = (icon.width(), icon.height());
    if (width, height) == (size, size) || width == 0 || height == 0 {
        return icon.clone();
    }
    let source = icon.pixels();
    let (scale_x, scale_y) = (width as f32 / size as f32, height as f32 / size as f32);
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        let (top, bottom) = (y as f32 * scale_y, (y + 1) as f32 * scale_y);
        for x in 0..size {
            let (left, right) = (x as f32 * scale_x, (x + 1) as f32 * scale_x);
            let mut sum = [0f32; 4];
            let mut area = 0f32;
            for sy in top.floor() as u32..(bottom.ceil() as u32).min(height) {
                let cover_y = (bottom.min(sy as f32 + 1.0) - top.max(sy as f32)).max(0.0);
                for sx in left.floor() as u32..(right.ceil() as u32).min(width) {
                    let cover_x = (right.min(sx as f32 + 1.0) - left.max(sx as f32)).max(0.0);
                    let weight = cover_x * cover_y;
                    let from = ((sy * width + sx) * 4) as usize;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += source[from + channel] as f32 * weight;
                    }
                    area += weight;
                }
            }
            if area > 0.0 {
                let to = ((y * size + x) * 4) as usize;
                for (channel, total) in sum.iter().enumerate() {
                    out[to + channel] = (total / area).round().min(255.0) as u8;
                }
            }
        }
    }
    Icon::new(size, size, out.into_boxed_slice())
}

/// Draws `overlay` over `base`, source-over. Both are premultiplied BGRA and
/// the same size.
fn composite_overlay(base: &Icon, overlay: &Icon) -> Icon {
    let mut out = base.pixels().to_vec();
    for (destination, source) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(overlay.pixels().as_chunks::<4>().0.iter())
    {
        let alpha = source[3] as u16;
        if alpha == 0 {
            continue;
        }
        for channel in 0..4 {
            let under = destination[channel] as u16;
            destination[channel] =
                (source[channel] as u16 + (under * (255 - alpha) + 127) / 255).min(255) as u8;
        }
    }
    Icon::new(base.width(), base.height(), out.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon() -> Icon {
        Icon::new(1, 1, vec![0; 4].into_boxed_slice())
    }

    fn key(path: &str) -> (PathBuf, i32, u8) {
        (PathBuf::from(path), 32, 80)
    }

    #[test]
    fn evicts_the_least_recently_used_icon() {
        let mut cache = IconCache::with_capacity(2);
        cache.insert("a".into(), 32, 80, icon());
        cache.clock += 1;
        cache.insert("b".into(), 32, 80, icon());
        cache.clock += 1;
        // Touch "a" so "b" becomes the oldest.
        cache.entries.get_mut(&key("a")).unwrap().last_used = cache.clock;
        cache.insert("c".into(), 32, 80, icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&key("a")));
        assert!(!cache.entries.contains_key(&key("b")));
    }

    #[test]
    fn invalidate_forgets_every_variant_of_a_path() {
        let mut cache = IconCache::new();
        cache.insert("a".into(), 32, 80, icon());
        cache.insert("a".into(), 48, 80, icon());
        cache.insert("a".into(), 32, 60, icon());
        cache.insert("b".into(), 32, 80, icon());
        cache.invalidate(Path::new("a"));
        assert_eq!(cache.len(), 1);
        assert!(cache.entries.contains_key(&key("b")));
    }

    #[test]
    fn evicted_icons_stay_usable_by_their_holders() {
        let mut cache = IconCache::with_capacity(1);
        let held = icon();
        cache.insert("a".into(), 32, 80, held.clone());
        cache.insert("b".into(), 32, 80, icon());
        assert_eq!(held.pixels().len(), 4);
    }

    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Icon {
        Icon::new(1, 1, vec![r, g, b, a].into_boxed_slice())
    }

    #[test]
    fn an_opaque_overlay_replaces_the_base() {
        let base = pixel(255, 0, 0, 255);
        let overlay = pixel(0, 0, 255, 255);
        let combined = composite_overlay(&base, &overlay);
        assert_eq!(combined.pixels(), [0, 0, 255, 255]);
    }

    #[test]
    fn a_transparent_overlay_leaves_the_base() {
        let base = pixel(255, 0, 0, 255);
        let overlay = pixel(0, 0, 255, 0);
        let combined = composite_overlay(&base, &overlay);
        assert_eq!(combined.pixels(), [255, 0, 0, 255]);
    }

    #[test]
    fn a_half_transparent_overlay_blends() {
        let base = pixel(200, 0, 0, 255);
        let overlay = pixel(0, 0, 0, 128);
        let combined = composite_overlay(&base, &overlay);
        // out = 0 + 200 * (255 - 128)/255 = 99.6 -> 100 (premultiplied red).
        assert_eq!(combined.pixels()[0], 100);
        assert_eq!(combined.pixels()[3], 255);
    }

    #[test]
    fn resampling_to_the_same_size_keeps_the_pixels() {
        let pixels: Vec<u8> = (0..16).collect();
        let original = Icon::new(2, 2, pixels.clone().into_boxed_slice());
        assert_eq!(resample(&original, 2).pixels(), pixels.as_slice());
    }

    #[test]
    fn downscaling_averages_the_covered_pixels() {
        let quad = Icon::new(
            2,
            2,
            [
                [0, 0, 0, 0],
                [200, 0, 0, 255],
                [0, 0, 0, 0],
                [200, 0, 0, 255],
            ]
            .concat()
            .into_boxed_slice(),
        );
        let scaled = resample(&quad, 1);
        assert_eq!(scaled.pixels(), [100, 0, 0, 128]);
    }

    #[test]
    fn a_corner_glyph_stays_in_its_corner_when_downscaled() {
        let mut pixels = vec![0u8; 8 * 8 * 4];
        for y in 4..8 {
            for x in 0..4 {
                pixels[(y * 8 + x) * 4 + 3] = 255;
            }
        }
        let scaled = resample(&Icon::new(8, 8, pixels.into_boxed_slice()), 4);
        let alpha = |x: usize, y: usize| scaled.pixels()[(y * 4 + x) * 4 + 3];
        assert_eq!((alpha(0, 3), alpha(1, 2)), (255, 255));
        assert_eq!((alpha(2, 2), alpha(1, 1), alpha(3, 0)), (0, 0, 0));
    }

    #[test]
    fn the_overlay_is_scaled_into_the_bottom_left_corner() {
        let mut pixels = vec![0u8; 8 * 8 * 4];
        for y in 4..8 {
            for x in 0..4 {
                pixels[(y * 8 + x) * 4 + 3] = 255;
            }
        }
        let overlay = Icon::new(8, 8, pixels.into_boxed_slice());
        let placed = scale_overlay_to_corner(&overlay, 8, 50);
        assert_eq!((placed.width(), placed.height()), (8, 8));
        let alpha = |x: usize, y: usize| placed.pixels()[(y * 8 + x) * 4 + 3];
        // The 4x4 glyph halves to 2x2, still flush to the bottom-left.
        let solid = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| alpha(x, y) > 0)
            .count();
        assert_eq!(solid, 4);
        assert!(alpha(0, 7) > 0 && alpha(1, 6) > 0);
        assert_eq!((alpha(0, 0), alpha(7, 0), alpha(7, 7)), (0, 0, 0));
    }
}
