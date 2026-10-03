pub mod d2d;
pub mod scene;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) use d2d::PaintFrame;
pub use d2d::{Painter, PreparedScene};
pub use scene::{ItemState, ItemVisual, SpaceScene, SpaceStyle};

/// A decoded icon: premultiplied BGRA pixels, cheap to clone. The pixels are
/// owned here rather than borrowed from an `HICON`, so a scene stays valid
/// after `IconCache` evicts the icon it came from. Painters cache GPU bitmaps
/// by `id`, which is never reused within a process.
#[derive(Clone)]
pub struct Icon(Arc<IconImage>);

struct IconImage {
    id: u64,
    width: u32,
    height: u32,
    pixels: Box<[u8]>,
}

impl Icon {
    /// `pixels` holds `width * height` premultiplied BGRA pixels, top-down.
    pub(crate) fn new(width: u32, height: u32, pixels: Box<[u8]>) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self(Arc::new(IconImage {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels,
        }))
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
}

impl PartialEq for Icon {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Icon {}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icon")
            .field("id", &self.id())
            .field("width", &self.width())
            .field("height", &self.height())
            .finish()
    }
}
