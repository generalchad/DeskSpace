//! The shell icon cache: base icons from the image factory, the classic
//! fallback, and the overlays composited as Explorer draws them.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SHFILEINFOW,
    SHGFI_ADDOVERLAYS, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_LINKOVERLAY, SHGFI_OVERLAYINDEX,
    SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO,
    SHIL_LARGE, SHIL_SMALL, SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
use windows::core::{Interface, PCWSTR};

use crate::render::Icon;
use crate::render::d2d::{decode_bitmap, decode_icon};
use crate::win32::shell_items::shell_path;

/// What a cached icon was rendered for, so two uses of one path never share
/// the wrong pixels: an item with the shell's overlays, or a lens title's
/// folder icon (whose availability decides the fallback).
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum IconKind {
    Item {
        overlay_size: u8,
    },
    /// A folder's icon for a lens title bar, with whether the folder could be
    /// listed.
    Folder {
        available: bool,
    },
}

/// One icon: a path, the pixel size it was rendered at, and what it is for.
type Key = (PathBuf, i32, IconKind);

/// Icons decoded per path, pixel size, and overlay size, bounded by a
/// least-recently-used limit. Evicting is always safe: scenes hold their own
/// reference-counted `Icon`s. The size is part of the key, so a per-monitor DPI
/// or icon-size change does not hand out stale pixels, and the same holds for a
/// changed overlay size.
pub struct IconCache {
    capacity: usize,
    clock: u64,
    entries: HashMap<Key, CachedIcon>,
    /// The entries' keys by last use, so the oldest is the first and eviction
    /// needs no scan of `entries`.
    by_age: BTreeMap<u64, Key>,
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
            by_age: BTreeMap::new(),
        }
    }

    /// The icon for `path` rendered at `size` pixels square, with the shell's
    /// overlay drawn at `overlay_size` percent of that.
    pub fn icon_for(&mut self, path: &Path, size: i32, overlay_size: u8) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Item { overlay_size });
        if let Some(icon) = self.touch(&key) {
            return Some(icon);
        }
        let icon = load_icon(path, size, overlay_size)?;
        self.insert(key, icon.clone());
        Some(icon)
    }

    /// The icon for a lens's folder, drawn `size` pixels square in a title bar.
    /// An available folder uses its own shell icon; an unavailable one gets
    /// the shell's generic folder icon without touching the disk.
    pub fn folder_icon(&mut self, path: &Path, size: i32, available: bool) -> Option<Icon> {
        let key = (path.to_path_buf(), size, IconKind::Folder { available });
        if let Some(icon) = self.touch(&key) {
            return Some(icon);
        }
        let icon = load_folder_icon(path, size.clamp(1, 1024) as u32, available)?;
        self.insert(key, icon.clone());
        Some(icon)
    }

    /// Forgets every size of the icon for `path`, so the next lookup reloads
    /// it (for example after the file changed).
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.retain(|key, _| key.0 != path);
        self.by_age.retain(|_, key| key.0 != path);
    }

    /// Forgets every cached icon, so each is rendered again on next use.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_age.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Marks `key` as used now and returns its icon, when it is cached.
    fn touch(&mut self, key: &Key) -> Option<Icon> {
        let entry = self.entries.get_mut(key)?;
        self.clock += 1;
        self.by_age.remove(&entry.last_used);
        entry.last_used = self.clock;
        self.by_age.insert(self.clock, key.clone());
        Some(entry.icon.clone())
    }

    fn insert(&mut self, key: Key, icon: Icon) {
        self.clock += 1;
        if self.entries.len() >= self.capacity
            && let Some((_, oldest)) = self.by_age.pop_first()
        {
            self.entries.remove(&oldest);
        }
        self.by_age.insert(self.clock, key.clone());
        self.entries.insert(
            key,
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

/// A lens title's folder icon: the folder's own shell icon when it can be
/// listed, else the generic one. Sized by `resample`, so the title bar gets a
/// crisp icon at any DPI.
fn load_folder_icon(path: &Path, size: u32, available: bool) -> Option<Icon> {
    let base = available
        .then(|| load_base_icon(path, size))
        .flatten()
        .or_else(load_generic_folder_icon)?;
    Some(resample(&base, size))
}

/// The shell's generic folder icon, read from the file attributes rather than
/// from the disk: `SHGFI_USEFILEATTRIBUTES` makes `SHGetFileInfoW` trust
/// `FILE_ATTRIBUTE_DIRECTORY`, so a folder that is gone is never touched.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
fn load_generic_folder_icon() -> Option<Icon> {
    let wide = shell_path(Path::new("folder"));
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES;
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure. The
    // returned HICON is ours and destroyed after its pixels are copied out.
    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_DIRECTORY,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = decode_icon(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
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
    let wide = shell_path(path);
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
        let icon = decode_icon(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        icon
    }
}

fn load_base_icon(path: &Path, size: u32) -> Option<Icon> {
    let wide = shell_path(path);
    // SAFETY: `wide` is NUL-terminated and outlives the parse.
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }.ok()?;
    shell_item_icon(&item, size)
}

/// Renders a shell item's icon `size` pixels square with the shell's image
/// factory, without overlays. Works for virtual items with no path, such as
/// apps in the Applications folder.
pub(crate) fn shell_item_icon(item: &IShellItem, size: u32) -> Option<Icon> {
    // SAFETY: `item` is live; the image factory and its bitmap are released
    // below, after the pixels are copied.
    unsafe {
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

/// The 1-based overlay index Explorer would draw on `path`, or `None` when it
/// draws none. `ADDOVERLAYS` asks the registered overlay handlers (sync badges
/// and the like); `LINKOVERLAY` adds the link arrow, and is passed only for
/// shortcuts because it arrows any file it is given.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
fn overlay_index(path: &Path) -> Option<i32> {
    let wide = shell_path(path);
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
        let icon = decode_icon(hicon);
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
pub(crate) fn resample(icon: &Icon, size: u32) -> Icon {
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

    fn key(path: &str) -> Key {
        (PathBuf::from(path), 32, IconKind::Item { overlay_size: 80 })
    }

    #[test]
    fn evicts_the_least_recently_used_icon() {
        let mut cache = IconCache::with_capacity(2);
        cache.insert(key("a"), icon());
        cache.insert(key("b"), icon());
        // Touch "a" so "b" becomes the oldest.
        assert!(cache.touch(&key("a")).is_some());
        cache.insert(key("c"), icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&key("a")));
        assert!(!cache.entries.contains_key(&key("b")));
    }

    #[test]
    fn a_hit_protects_the_icon_from_the_next_eviction() {
        let mut cache = IconCache::with_capacity(2);
        cache.insert(key("a"), icon());
        cache.insert(key("b"), icon());
        // A hit on the newest still moves its age, so the oldest is evicted.
        assert!(cache.touch(&key("b")).is_some());
        cache.insert(key("c"), icon());
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&key("b")));
        assert!(cache.entries.contains_key(&key("c")));
        assert!(!cache.entries.contains_key(&key("a")));
        assert_eq!(cache.by_age.len(), cache.entries.len(), "ages stay in step");
    }

    #[test]
    fn invalidate_forgets_every_variant_of_a_path() {
        let mut cache = IconCache::new();
        cache.insert(key("a"), icon());
        cache.insert((PathBuf::from("a"), 48, key("a").2), icon());
        cache.insert(
            (PathBuf::from("a"), 32, IconKind::Folder { available: true }),
            icon(),
        );
        cache.insert(key("b"), icon());
        cache.invalidate(Path::new("a"));
        assert_eq!(cache.len(), 1);
        assert!(cache.entries.contains_key(&key("b")));
    }

    #[test]
    fn evicted_icons_stay_usable_by_their_holders() {
        let mut cache = IconCache::with_capacity(1);
        let held = icon();
        cache.insert(key("a"), held.clone());
        cache.insert(key("b"), icon());
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
    fn a_folder_title_icon_loads_available_and_unavailable() {
        // SAFETY: the shell APIs below need COM; this thread lives for the
        // test, so the apartment is released with the test process.
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
        };
        let missing = Path::new("C:/nowhere/DeskSpace title icon test");
        assert!(load_generic_folder_icon().is_some());
        let mut cache = IconCache::new();
        // An available folder uses its own icon; an unavailable one the
        // generic icon; the two never share a cache entry.
        assert!(cache.folder_icon(&std::env::temp_dir(), 16, true).is_some());
        assert!(cache.folder_icon(missing, 16, true).is_some());
        assert!(cache.folder_icon(missing, 16, false).is_some());
        assert_eq!(cache.len(), 3);
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
