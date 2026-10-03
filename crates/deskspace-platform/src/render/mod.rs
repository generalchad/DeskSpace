pub mod d2d;
pub mod scene;

pub use d2d::{Painter, PreparedScene};
pub use scene::{spaceScene, spaceStyle, ItemState, ItemVisual};

/// An opaque Win32 `HICON`, owned by `IconCache`.
pub type IconHandle = isize;
