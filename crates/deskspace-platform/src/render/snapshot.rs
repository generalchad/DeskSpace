//! A space's pixels at the start of an animation. Effects that pull the
//! space's contents apart (Smoke) sample it, so what the space showed is what
//! drifts away. Pure: the platform renders the snapshot (`d2d::snapshot`).

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

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixel at (`x`, `y`), or transparent outside the snapshot.
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        self.pixels[(y as u32 * self.width + x as u32) as usize]
    }

    /// Bilinear sample at (`x`, `y`), in pixels, for float coordinates.
    /// Premultiplied pixels interpolate safely; outside the snapshot is
    /// transparent, so an edge fades rather than smearing.
    pub fn sample(&self, x: f32, y: f32) -> u32 {
        if !x.is_finite() || !y.is_finite() {
            return 0;
        }
        let (x0, y0) = (x.floor() as i32, y.floor() as i32);
        let (tx, ty) = (x - x0 as f32, y - y0 as f32);
        let corner = |dx, dy| self.pixel(x0 + dx, y0 + dy);
        let (top_left, top_right) = (corner(0, 0), corner(1, 0));
        let (bottom_left, bottom_right) = (corner(0, 1), corner(1, 1));
        let mut channels = [0u32; 4];
        for (channel, out) in channels.iter_mut().enumerate() {
            let shift = channel * 8;
            let at = |pixel: u32| f32::from((pixel >> shift) as u8);
            let top = at(top_left) + (at(top_right) - at(top_left)) * tx;
            let bottom = at(bottom_left) + (at(bottom_right) - at(bottom_left)) * tx;
            *out = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u32;
        }
        // Rounding can put a color a step above its alpha; premultiplied
        // pixels never let that happen.
        let alpha = channels[3];
        for channel in &mut channels[..3] {
            *channel = (*channel).min(alpha);
        }
        channels
            .iter()
            .enumerate()
            .map(|(channel, value)| value << (channel * 8))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot::new(
            2,
            2,
            Box::new([
                0xFF20_4060, // top-left: opaque
                0x8040_6080, // top-right: half alpha
                0x0000_0000, // bottom-left: transparent
                0xFFE0_C0A0, // bottom-right: opaque
            ]),
        )
    }

    #[test]
    fn a_sample_at_a_pixel_center_is_that_pixel() {
        let snapshot = snapshot();
        assert_eq!(snapshot.sample(0.0, 0.0), 0xFF20_4060);
        assert_eq!(snapshot.sample(1.0, 1.0), 0xFFE0_C0A0);
    }

    #[test]
    fn samples_interpolate_and_fade_at_the_edge() {
        let snapshot = snapshot();
        let middle = snapshot.sample(0.5, 0.5);
        // Each channel is the mean of the four corners.
        for (shift, expected) in [
            (0, (0x60 + 0x80 + 0xA0) / 4),
            (8, (0x40 + 0x60 + 0xC0) / 4),
            (16, (0x20 + 0x40 + 0xE0) / 4),
            (24, (0xFF + 0x80 + 0xFF + 2) / 4),
        ] {
            assert!((middle >> shift) & 0xFF == expected, "channel {shift}");
        }
        let edge = snapshot.sample(1.5, 0.5);
        let inside = snapshot.sample(1.0, 0.5);
        assert!(edge >> 24 < inside >> 24, "the edge fades out");
        assert_eq!(snapshot.sample(-2.0, 0.5), 0);
        assert_eq!(snapshot.sample(5.0, 0.5), 0);
        assert_eq!(snapshot.sample(f32::NAN, 0.0), 0);
    }

    #[test]
    fn every_sample_stays_premultiplied() {
        let snapshot = snapshot();
        for x in 0..=20 {
            for y in 0..=20 {
                let pixel = snapshot.sample(x as f32 * 0.1 - 0.5, y as f32 * 0.1 - 0.5);
                let alpha = pixel >> 24;
                for shift in [0, 8, 16] {
                    assert!((pixel >> shift) & 0xFF <= alpha, "{pixel:#010X}");
                }
            }
        }
    }
}
