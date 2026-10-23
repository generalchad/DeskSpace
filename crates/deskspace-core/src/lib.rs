#![forbid(unsafe_code)]

pub mod animation;
pub mod command;
pub mod desktop;
pub mod desktop_layer;
pub mod geometry;
pub mod id;
pub mod item;
pub mod layout;
pub mod placement;
pub mod source;
pub mod space;
pub mod theme;
pub mod view;

mod sort;

pub use command::{Command, CommandOutcome, Touched};
pub use desktop::Desktop;
pub use desktop_layer::DesktopLayer;
pub use geometry::{Point, Rect, Size};
pub use id::{ItemId, SpaceId};
pub use item::{Item, ItemFacts, ItemKind, SortKey};
pub use layout::GridSpec;
pub use sort::SortRule;
pub use source::{FolderEntry, FolderView, LensTitle, SpaceSource, lens_title};
pub use space::{ResizeEdges, Space, SpaceLook};
pub use theme::{Backdrop, BorderSource, Color, LabelShadow, Palette, Preset, Theme, ThemeMode};
pub use view::{SpaceView, ViewMode};
