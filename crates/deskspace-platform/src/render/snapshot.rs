//! A space's pixels at the start of an animation, as text and icons drawn
//! apart from the window (Smoke). Pure: the platform renders it
//! (`d2d::contents`).

/// A space's pixels, `width` by `height`, top-down premultiplied BGRA
/// (`0xAARRGGBB`), at the space's pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    width: u32,
    height: u32,
    pixels: Box<[u32]>,
}

impl Snapshot {
    pub fn new(width: u32, height: u32, pixels: Box<[u32]>) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize);
        Self {
            width,
            height,
            pixels,
        }
    }

    /// The pixel at (`x`, `y`), or transparent outside the snapshot.
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        self.pixels[(y as u32 * self.width + x as u32) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_is_transparent_outside_the_snapshot() {
        let snapshot = Snapshot::new(2, 1, Box::new([0xFF20_4060, 0x8040_6080]));
        assert_eq!(snapshot.pixel(0, 0), 0xFF20_4060);
        assert_eq!(snapshot.pixel(1, 0), 0x8040_6080);
        assert_eq!(snapshot.pixel(-1, 0), 0);
        assert_eq!(snapshot.pixel(2, 0), 0);
        assert_eq!(snapshot.pixel(0, 1), 0);
    }
}
