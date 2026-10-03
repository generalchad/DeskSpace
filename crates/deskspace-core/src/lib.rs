#![forbid(unsafe_code)]

pub mod command;
pub mod desktop;
pub mod error;
pub mod space;
pub mod geometry;
pub mod id;
pub mod item;
pub mod layout;
pub mod theme;

pub use command::{Command, CommandOutcome};
pub use desktop::Desktop;
pub use error::CoreError;
pub use space::{space, ResizeEdges};
pub use geometry::{Point, Rect, Size};
pub use id::{spaceId, ItemId};
pub use item::{Item, ItemKind};
pub use layout::GridSpec;
pub use theme::{Backdrop, Color, CornerStyle, Palette, Theme, ThemeMode};

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
