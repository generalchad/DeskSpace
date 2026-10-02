pub mod gdi;
pub mod scene;

pub use gdi::Painter;
pub use scene::{spaceVisual, ItemVisual, Scene};

pub type IconHandle = isize;
