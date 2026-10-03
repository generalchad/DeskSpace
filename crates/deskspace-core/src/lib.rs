#![forbid(unsafe_code)]

pub mod command;
pub mod desktop;
pub mod error;
pub mod geometry;
pub mod id;
pub mod item;
pub mod layout;
pub mod space;
pub mod theme;

pub use command::{Command, CommandOutcome, SortKey};
pub use desktop::Desktop;
pub use error::CoreError;
pub use geometry::{Point, Rect, Size};
pub use id::{ItemId, SpaceId};
pub use item::{Item, ItemKind};
pub use layout::GridSpec;
pub use space::{ResizeEdges, Space};
pub use theme::{Backdrop, Color, Palette, Theme, ThemeMode};

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
