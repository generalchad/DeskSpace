#![forbid(unsafe_code)]

pub mod animation;
pub mod command;
pub mod desktop;
pub mod desktop_layer;
pub mod geometry;
pub mod icon;
pub mod icon_match;
pub mod icon_set;
pub mod id;
pub mod item;
pub mod layout;
pub mod placement;
pub mod space;
pub mod theme;

pub use command::{Command, CommandOutcome, SortKey, Touched};
pub use desktop::Desktop;
pub use desktop_layer::DesktopLayer;
pub use geometry::{Point, Rect, Size};
pub use icon::{IconRef, SpaceIcon};
pub use icon_set::{Glyph, IconSet};
pub use id::{ItemId, SpaceId};
pub use item::{Item, ItemFacts, ItemKind};
pub use layout::GridSpec;
pub use space::{ResizeEdges, Space, SpaceLook};
pub use theme::{Backdrop, Color, LabelShadow, Palette, Preset, Theme, ThemeMode};
