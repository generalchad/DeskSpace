//! The glyph cache: icon-set glyphs drawn at a pixel size and color (ROADMAP
//! D16), kept like the shell icon cache. Evicting is safe because a scene
//! holds its own reference-counted `Icon`.

use std::collections::{BTreeMap, HashMap, HashSet};

use deskspace_core::Glyph;
use deskspace_core::theme::Color;

use crate::render::Icon;
use crate::render::d2d::rasterize;

/// One glyph: its `<prefix>:<name>` key, the pixel size, and the color.
type Key = (String, i32, [u8; 4]);

/// Glyphs drawn per key, size, and color, bounded by a least-recently-used
/// limit. The color is part of the key so a theme change does not hand out
/// pixels in the old one.
pub struct GlyphCache {
    capacity: usize,
    clock: u64,
    entries: HashMap<Key, CachedGlyph>,
    /// The entries' keys by last use, so the oldest is the first and eviction
    /// needs no scan of `entries`.
    by_age: BTreeMap<u64, Key>,
    /// Glyphs whose draw failed, so each is logged once rather than on every
    /// repaint.
    failed: HashSet<String>,
}

struct CachedGlyph {
    icon: Icon,
    last_used: u64,
}

impl GlyphCache {
    /// Enough for every title bar in a session, with room for both themes.
    pub const DEFAULT_CAPACITY: usize = 256;

    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            clock: 0,
            entries: HashMap::new(),
            by_age: BTreeMap::new(),
            failed: HashSet::new(),
        }
    }

    /// `key`'s glyph at `px` pixels square in `color`, drawn only on a miss.
    /// `glyph` looks the glyph up in its set, so it is not called when the
    /// pixels are already cached. `None` when the glyph is unknown, or when
    /// this Windows cannot draw SVG documents.
    pub fn icon(
        &mut self,
        key: &str,
        px: i32,
        color: Color,
        glyph: impl FnOnce() -> Option<Glyph>,
    ) -> Option<Icon> {
        let key = (key.to_owned(), px, [color.r, color.g, color.b, color.a]);
        if let Some(icon) = self.touch(&key) {
            return Some(icon);
        }
        let glyph = glyph()?;
        match rasterize(&glyph, px, color) {
            Ok(icon) => {
                self.insert(key, icon.clone());
                Some(icon)
            }
            Err(error) => {
                if self.failed.insert(key.0.clone()) {
                    tracing::debug!(%error, glyph = key.0, "could not draw a glyph");
                }
                None
            }
        }
    }

    /// Forgets every cached glyph, so each is drawn again on next use (for
    /// example when an icon set is replaced).
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
            CachedGlyph {
                icon,
                last_used: self.clock,
            },
        );
    }
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    fn square() -> Glyph {
        Glyph {
            key: "test:square".to_owned(),
            body: "<path d='M0 0h8v8H0z'/>".to_owned(),
            width: 8,
            height: 8,
        }
    }

    #[test]
    fn it_evicts_the_least_recently_used_entry() {
        // SAFETY: drawing needs COM on this thread; already initialized is fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let green = Color::rgb(0, 255, 0);
        let drawn = Cell::new(0);
        fn draw(drawn: &Cell<u32>) -> Option<Glyph> {
            drawn.set(drawn.get() + 1);
            Some(square())
        }
        let mut cache = GlyphCache::with_capacity(2);

        assert!(cache.icon("test:square", 8, red, || draw(&drawn)).is_some());
        assert!(
            cache
                .icon("test:square", 8, blue, || draw(&drawn))
                .is_some()
        );
        assert_eq!(drawn.get(), 2);
        // A hit does not draw again.
        assert!(cache.icon("test:square", 8, red, || draw(&drawn)).is_some());
        assert_eq!(drawn.get(), 2);
        assert_eq!(cache.len(), 2);

        // Green is new, so red (used most recently) stays and blue goes.
        assert!(
            cache
                .icon("test:square", 8, green, || draw(&drawn))
                .is_some()
        );
        assert_eq!(drawn.get(), 3);
        assert_eq!(cache.len(), 2);
        assert!(cache.icon("test:square", 8, red, || draw(&drawn)).is_some());
        assert_eq!(drawn.get(), 3, "red is still cached");
        assert!(
            cache
                .icon("test:square", 8, blue, || draw(&drawn))
                .is_some()
        );
        assert_eq!(drawn.get(), 4, "blue was evicted");
    }

    #[test]
    fn a_missing_glyph_is_not_cached() {
        let mut cache = GlyphCache::new();
        assert!(
            cache
                .icon("test:gone", 8, Color::rgb(0, 0, 0), || None)
                .is_none()
        );
        assert!(cache.is_empty());
    }
}
