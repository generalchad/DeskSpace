#![forbid(unsafe_code)]

pub mod error;
pub mod icons;
pub mod schema;
pub mod store;

pub use error::ConfigError;
pub use icons::{ICON_SETS, IconSetSpec, IconStore, default_set, icon_set};
pub use schema::{CURRENT_VERSION, Config, NEW_SPACE_SIZE_RANGE, NewLens, WindowSettings};
pub use store::ConfigStore;
