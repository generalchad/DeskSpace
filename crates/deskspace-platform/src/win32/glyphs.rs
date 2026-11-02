//! The glyph cache: icon-set glyphs drawn at a pixel size and color (ROADMAP
//! D16), kept like the shell icon cache. Evicting is safe because a scene
//! holds its own reference-counted `Icon`.

use std::collections::{BTreeMap, HashMap, HashSet};

use deskspace_core::Glyph;
use deskspace_core::theme::Color;

use crate::PlatformError;
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
        let pixels = (key.to_owned(), px, [color.r, color.g, color.b, color.a]);
        let drawn = self.get_or_draw(pixels, || {
            let Some(glyph) = glyph() else {
                return Ok(None);
            };
            rasterize(&glyph, px, color).map(Some)
        });
        match drawn {
            Ok(icon) => icon,
            Err(error) => {
                if self.failed.insert(key.to_owned()) {
                    tracing::debug!(%error, glyph = key, "could not draw a glyph");
                }
                None
            }
        }
    }

    /// `key`'s icon when cached, else `draw`'s, which is inserted when it
    /// returns one. `draw` is not called on a hit. An error is the caller's to
    /// report, and nothing is cached then.
    fn get_or_draw(
        &mut self,
        key: Key,
        draw: impl FnOnce() -> Result<Option<Icon>, PlatformError>,
    ) -> Result<Option<Icon>, PlatformError> {
        if let Some(icon) = self.touch(&key) {
            return Ok(Some(icon));
        }
        let drawn = draw()?;
        if let Some(icon) = &drawn {
            self.insert(key, icon.clone());
        }
        Ok(drawn)
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

    fn fake_icon(shade: u8) -> Icon {
        Icon::new(1, 1, Box::new([shade, shade, shade, 255]))
    }

    fn key(color: Color) -> Key {
        (
            "test:square".to_owned(),
            8,
            [color.r, color.g, color.b, color.a],
        )
    }

    fn fake_draw(drawn: &Cell<u32>) -> Result<Option<Icon>, PlatformError> {
        drawn.set(drawn.get() + 1);
        Ok(Some(fake_icon(drawn.get() as u8)))
    }

    fn get(cache: &mut GlyphCache, color: Color, drawn: &Cell<u32>) -> Option<Icon> {
        cache.get_or_draw(key(color), || fake_draw(drawn)).unwrap()
    }

    #[test]
    fn it_evicts_the_least_recently_used_entry() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let green = Color::rgb(0, 255, 0);
        let drawn = Cell::new(0);
        let mut cache = GlyphCache::with_capacity(2);

        assert!(get(&mut cache, red, &drawn).is_some());
        assert!(get(&mut cache, blue, &drawn).is_some());
        assert_eq!(drawn.get(), 2);
        // A hit does not draw again.
        assert!(get(&mut cache, red, &drawn).is_some());
        assert_eq!(drawn.get(), 2);
        assert_eq!(cache.len(), 2);

        // Green is new, so red (used most recently) stays and blue goes.
        assert!(get(&mut cache, green, &drawn).is_some());
        assert_eq!(drawn.get(), 3);
        assert_eq!(cache.len(), 2);
        assert!(get(&mut cache, red, &drawn).is_some());
        assert_eq!(drawn.get(), 3, "red is still cached");
        assert!(get(&mut cache, blue, &drawn).is_some());
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
